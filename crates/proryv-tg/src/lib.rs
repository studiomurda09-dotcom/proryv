//! Telegram через Proryv: локальный MTProto-прокси, который пересылает трафик
//! Telegram Desktop через WebSocket на kwsN.web.telegram.org (вход веб-версии).
//! Идея и протокол — tg-ws-proxy (Flowseal, MIT). Сторонние CF-прокси не используются.

pub mod mtproto;
mod ws;

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        Arc, Mutex,
    },
    time::Duration,
};

use ctr::cipher::StreamCipher;
use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use mtproto::{crypto_ctx, parse_client_init, relay_init, MsgSplitter, HANDSHAKE_LEN};

/// IP по умолчанию для прямого TCP (запасной путь), как в tg-ws-proxy.
fn dc_ip(dc: u16) -> Option<&'static str> {
    Some(match dc {
        1 => "149.154.175.50",
        2 => "149.154.167.51",
        3 => "149.154.175.100",
        4 => "149.154.167.91",
        5 => "149.154.171.5",
        203 => "91.105.192.100",
        _ => return None,
    })
}

/// Куда подключаться по WebSocket: (адрес для TCP, домен для TLS/Host).
fn ws_targets(dc: u16, media: bool) -> Vec<(String, String)> {
    let dc = if dc == 203 { 2 } else { dc };
    let mut domains = vec![];
    if media {
        domains.push(format!("kws{dc}-1.web.telegram.org"));
    }
    domains.push(format!("kws{dc}.web.telegram.org"));
    let mut out = Vec::new();
    for d in domains {
        // DC2/DC4: фронтенд 149.154.167.220 доступен даже там, где обычные адреса
        // web.telegram.org заблокированы по IP (проверено на сотовой сети).
        if dc == 2 || dc == 4 {
            out.push(("149.154.167.220".to_string(), d.clone()));
        }
        out.push((d.clone(), d));
    }
    out
}

#[derive(Default, Serialize, Clone, Debug)]
pub struct TgStatsView {
    pub running: bool,
    pub port: u16,
    pub active: u64,
    pub total: u64,
    pub via_ws: u64,
    pub via_tcp: u64,
    pub failed: u64,
    pub bad_secret: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub last_error: Option<String>,
}

#[derive(Default)]
struct Stats {
    active: AtomicU64,
    total: AtomicU64,
    via_ws: AtomicU64,
    via_tcp: AtomicU64,
    failed: AtomicU64,
    bad_secret: AtomicU64,
    up: AtomicU64,
    down: AtomicU64,
    last_error: Mutex<Option<String>>,
}

pub struct TgProxy {
    port: u16,
    stats: Arc<Stats>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Новый случайный секрет (32 hex-символа).
pub fn gen_secret() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("random");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn parse_secret(hex: &str) -> Result<[u8; 16], String> {
    if hex.len() != 32 {
        return Err("секрет должен быть 32 hex-символа".into());
    }
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| "секрет не hex")?;
    }
    Ok(out)
}

/// Ссылка для Telegram: открыть — и прокси добавится сам.
pub fn link(port: u16, secret_hex: &str) -> String {
    format!("tg://proxy?server=127.0.0.1&port={port}&secret=dd{secret_hex}")
}

impl TgProxy {
    pub fn start(port: u16, secret_hex: &str) -> Result<TgProxy, String> {
        let secret = parse_secret(secret_hex)?;
        let stats = Arc::new(Stats::default());
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let st = stats.clone();
        let thread = std::thread::Builder::new()
            .name("proryv-tg".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                rt.block_on(async move {
                    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
                        Ok(l) => l,
                        Err(e) => {
                            let _ = ready_tx.send(Err(format!("порт {port} занят: {e}")));
                            return;
                        }
                    };
                    let _ = ready_tx.send(Ok(()));
                    tokio::select! {
                        _ = stop_rx => {}
                        _ = accept_loop(listener, secret, st) => {}
                    }
                });
                rt.shutdown_timeout(Duration::from_millis(500));
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|e| e.to_string())??;
        Ok(TgProxy { port, stats, stop: Some(stop_tx), thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn stats(&self) -> TgStatsView {
        let s = &self.stats;
        TgStatsView {
            running: true,
            port: self.port,
            active: s.active.load(Relaxed),
            total: s.total.load(Relaxed),
            via_ws: s.via_ws.load(Relaxed),
            via_tcp: s.via_tcp.load(Relaxed),
            failed: s.failed.load(Relaxed),
            bad_secret: s.bad_secret.load(Relaxed),
            bytes_up: s.up.load(Relaxed),
            bytes_down: s.down.load(Relaxed),
            last_error: s.last_error.lock().unwrap().clone(),
        }
    }

    pub fn stop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for TgProxy {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Самопроверка: подключиться к своему прокси как Telegram-клиент, отправить
/// req_pq_multi в DC и дождаться resPQ от настоящего сервера Telegram.
/// Возвращает время ответа в мс.
pub fn self_test(port: u16, secret_hex: &str, dc: i16) -> Result<u128, String> {
    use ctr::cipher::KeyIvInit;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    let secret = parse_secret(secret_hex)?;
    let sha = |a: &[u8]| -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(a);
        h.update(secret);
        h.finalize().into()
    };
    // init-пакет клиента (как у Telegram Desktop с dd-секретом)
    let mut r = [0u8; 64];
    loop {
        getrandom::getrandom(&mut r).map_err(|e| e.to_string())?;
        if r[0] != 0xef && r[4..8] != [0; 4] {
            break;
        }
    }
    let mut enc = mtproto::Ctr::new((&sha(&r[8..40])).into(), (&r[40..56]).into());
    let mut ks = [0u8; 64];
    enc.apply_keystream(&mut ks);
    let d = dc.to_le_bytes();
    let tail = [0xef, 0xef, 0xef, 0xef, d[0], d[1], 0, 0];
    for i in 0..8 {
        r[56 + i] = tail[i] ^ ks[56 + i];
    }
    let mut rev = [0u8; 48];
    rev.copy_from_slice(&r[8..56]);
    rev.reverse();
    let mut dec = mtproto::Ctr::new((&sha(&rev[..32])).into(), (&rev[32..]).into());

    // req_pq_multi без шифрования: auth_key_id=0, msg_id, len, body.
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let msg_id: u64 = ((now.as_secs() as u64) << 32) | (((now.subsec_nanos() as u64) << 2) & 0xffff_fffc);
    let mut body = Vec::new();
    body.extend_from_slice(&0u64.to_le_bytes());
    body.extend_from_slice(&msg_id.to_le_bytes());
    body.extend_from_slice(&20u32.to_le_bytes());
    body.extend_from_slice(&0xbe7e_8ef1u32.to_le_bytes());
    body.extend_from_slice(&nonce);
    let mut pkt = vec![(body.len() / 4) as u8]; // abridged
    pkt.extend_from_slice(&body);
    enc.apply_keystream(&mut pkt);

    let t0 = std::time::Instant::now();
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("прокси не отвечает: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok();
    s.write_all(&r).and_then(|_| s.write_all(&pkt)).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 256];
    let mut got = Vec::new();
    while got.len() < 1 + 20 + 4 {
        let n = s.read(&mut buf).map_err(|e| format!("нет ответа от Telegram: {e}"))?;
        if n == 0 {
            return Err("соединение закрыто без ответа".into());
        }
        got.extend_from_slice(&buf[..n]);
    }
    dec.apply_keystream(&mut got);
    // [len][auth_key_id 8][msg_id 8][len 4][constructor 4]
    let ctor = u32::from_le_bytes([got[21], got[22], got[23], got[24]]);
    if ctor == 0x0516_2463 {
        Ok(t0.elapsed().as_millis())
    } else {
        Err(format!("неожиданный ответ 0x{ctor:08x}"))
    }
}

async fn accept_loop(listener: TcpListener, secret: [u8; 16], stats: Arc<Stats>) {
    loop {
        let Ok((sock, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let _ = sock.set_nodelay(true);
        let stats = stats.clone();
        tokio::spawn(async move {
            stats.total.fetch_add(1, Relaxed);
            stats.active.fetch_add(1, Relaxed);
            if let Err(e) = handle_client(sock, peer, secret, stats.clone()).await {
                stats.failed.fetch_add(1, Relaxed);
                *stats.last_error.lock().unwrap() = Some(e);
            }
            stats.active.fetch_sub(1, Relaxed);
        });
    }
}

async fn handle_client(mut client: TcpStream, _peer: SocketAddr, secret: [u8; 16], stats: Arc<Stats>) -> Result<(), String> {
    let mut init = [0u8; HANDSHAKE_LEN];
    tokio::time::timeout(Duration::from_secs(10), client.read_exact(&mut init))
        .await
        .map_err(|_| "таймаут рукопожатия".to_string())?
        .map_err(|e| e.to_string())?;
    let Some(ci) = parse_client_init(&init, &secret) else {
        stats.bad_secret.fetch_add(1, Relaxed);
        return Err("неверный секрет — заново добавьте прокси в Telegram".into());
    };
    let (dc, media) = (ci.dc() % 10000, ci.is_media());
    let test = ci.dc() >= 10000;
    let dc_idx = if media { -(dc as i16) } else { dc as i16 };
    let relay = relay_init(ci.tag, dc_idx);
    let ctx = crypto_ctx(&ci, &secret, &relay);

    // 1) WebSocket через web.telegram.org
    let path = if test { "/apiws_test" } else { "/apiws" };
    let mut last_err = String::new();
    for (host, domain) in ws_targets(dc, media) {
        match ws::WsStream::connect(&host, &domain, path).await {
            Ok(mut wsock) => {
                wsock.send(&relay).await.map_err(|e| e.to_string())?;
                stats.via_ws.fetch_add(1, Relaxed);
                return bridge_ws(client, wsock, ctx, MsgSplitter::new(ci.tag), stats).await;
            }
            Err(e) => last_err = format!("{domain}: {e}"),
        }
    }
    // 2) Запасной путь — напрямую по TCP к дата-центру.
    let ip = dc_ip(dc).ok_or_else(|| format!("неизвестный DC{dc}; WS: {last_err}"))?;
    let mut tg = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((ip, 443)))
        .await
        .map_err(|_| format!("DC{dc}: нет соединения (WS: {last_err})"))?
        .map_err(|e| format!("DC{dc}: {e} (WS: {last_err})"))?;
    let _ = tg.set_nodelay(true);
    tg.write_all(&relay).await.map_err(|e| e.to_string())?;
    stats.via_tcp.fetch_add(1, Relaxed);
    bridge_tcp(client, tg, ctx, stats).await
}

async fn bridge_ws(client: TcpStream, wsock: ws::WsStream, ctx: mtproto::CryptoCtx, mut splitter: MsgSplitter, stats: Arc<Stats>) -> Result<(), String> {
    let (mut cr, mut cw) = client.into_split();
    let (mut wr, ww) = wsock.split();
    let mtproto::CryptoCtx { mut clt_dec, mut clt_enc, mut tg_enc, mut tg_dec } = ctx;
    let st_up = stats.clone();
    let up = async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = cr.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                if let Some(tail) = splitter.flush() {
                    ww.send(&tail).await.map_err(|e| e.to_string())?;
                }
                ww.close().await;
                return Ok::<(), String>(());
            }
            st_up.up.fetch_add(n as u64, Relaxed);
            let mut plain = buf[..n].to_vec();
            clt_dec.apply_keystream(&mut plain);
            let mut out = plain.clone();
            tg_enc.apply_keystream(&mut out);
            for part in splitter.split(&out, &plain) {
                ww.send(&part).await.map_err(|e| e.to_string())?;
            }
        }
    };
    let st_down = stats.clone();
    let down = async move {
        while let Some(mut data) = wr.recv().await.map_err(|e| e.to_string())? {
            st_down.down.fetch_add(data.len() as u64, Relaxed);
            tg_dec.apply_keystream(&mut data);
            clt_enc.apply_keystream(&mut data);
            cw.write_all(&data).await.map_err(|e| e.to_string())?;
        }
        let _ = cw.shutdown().await;
        Ok::<(), String>(())
    };
    tokio::select! {
        r = up => r,
        r = down => r,
    }
}

async fn bridge_tcp(client: TcpStream, tg: TcpStream, ctx: mtproto::CryptoCtx, stats: Arc<Stats>) -> Result<(), String> {
    let (mut cr, mut cw) = client.into_split();
    let (mut tr, mut tw) = tg.into_split();
    let mtproto::CryptoCtx { mut clt_dec, mut clt_enc, mut tg_enc, mut tg_dec } = ctx;
    let s1 = stats.clone();
    let up = async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = cr.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                let _ = tw.shutdown().await;
                return Ok::<(), String>(());
            }
            s1.up.fetch_add(n as u64, Relaxed);
            clt_dec.apply_keystream(&mut buf[..n]);
            tg_enc.apply_keystream(&mut buf[..n]);
            tw.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        }
    };
    let down = async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = tr.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                let _ = cw.shutdown().await;
                return Ok::<(), String>(());
            }
            stats.down.fetch_add(n as u64, Relaxed);
            tg_dec.apply_keystream(&mut buf[..n]);
            clt_enc.apply_keystream(&mut buf[..n]);
            cw.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        }
    };
    tokio::select! {
        r = up => r,
        r = down => r,
    }
}
