//! Минимальный WebSocket-клиент поверх TLS (rustls): только то, что нужно для /apiws.

use std::{io, sync::Arc, time::Duration};

use base64::Engine as _;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::Mutex,
};
use tokio_rustls::{client::TlsStream, rustls, TlsConnector};

type Tls = TlsStream<TcpStream>;

const OP_CONT: u8 = 0x0;
const OP_TEXT: u8 = 0x1;
const OP_BINARY: u8 = 0x2;
const OP_CLOSE: u8 = 0x8;
const OP_PING: u8 = 0x9;
const OP_PONG: u8 = 0xA;
const MAX_MSG: usize = 16 * 1024 * 1024;

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: std::sync::OnceLock<Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("tls versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

pub struct WsStream {
    tls: Tls,
}

impl WsStream {
    /// TCP к `host`:443, TLS с SNI `domain`, затем HTTP Upgrade на `path`.
    pub async fn connect(host: &str, domain: &str, path: &str) -> io::Result<WsStream> {
        let tcp = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((host, 443)))
            .await
            .map_err(|_| err("таймаут TCP"))??;
        let _ = tcp.set_nodelay(true);
        let name = rustls::pki_types::ServerName::try_from(domain.to_string()).map_err(|e| err(e.to_string()))?;
        let mut tls = tokio::time::timeout(Duration::from_secs(8), TlsConnector::from(tls_config()).connect(name, tcp))
            .await
            .map_err(|_| err("таймаут TLS"))??;

        let mut key = [0u8; 16];
        getrandom::getrandom(&mut key).map_err(|e| err(e.to_string()))?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {domain}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: binary\r\n\r\n",
            base64::engine::general_purpose::STANDARD.encode(key)
        );
        tls.write_all(req.as_bytes()).await?;

        // Читаем заголовки ответа побайтно до пустой строки.
        let mut head = Vec::with_capacity(512);
        let mut b = [0u8; 1];
        let read_head = async {
            while !head.ends_with(b"\r\n\r\n") {
                if tls.read(&mut b).await? == 0 || head.len() > 16 * 1024 {
                    return Err(err("обрыв при апгрейде"));
                }
                head.push(b[0]);
            }
            Ok::<(), io::Error>(())
        };
        tokio::time::timeout(Duration::from_secs(8), read_head).await.map_err(|_| err("таймаут апгрейда"))??;
        let status = String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string();
        if !status.contains(" 101") {
            return Err(err(format!("ответ «{status}»")));
        }
        Ok(WsStream { tls })
    }

    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.tls.write_all(&frame(OP_BINARY, data)).await
    }

    pub fn split(self) -> (WsReader, WsWriter) {
        let (r, w) = tokio::io::split(self.tls);
        let w = WsWriter { w: Arc::new(Mutex::new(w)) };
        (WsReader { r: BufReader::with_capacity(64 * 1024, r), w: w.clone(), frag: Vec::new() }, w)
    }
}

/// Кадр клиента (всегда с маской, как требует RFC 6455).
fn frame(op: u8, data: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(data.len() + 14);
    f.push(0x80 | op);
    let len = data.len();
    if len < 126 {
        f.push(0x80 | len as u8);
    } else if len < 65536 {
        f.push(0x80 | 126);
        f.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        f.push(0x80 | 127);
        f.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mut mask = [0u8; 4];
    let _ = getrandom::getrandom(&mut mask);
    f.extend_from_slice(&mask);
    f.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i & 3]));
    f
}

#[derive(Clone)]
pub struct WsWriter {
    w: Arc<Mutex<WriteHalf<Tls>>>,
}

impl WsWriter {
    pub async fn send(&self, data: &[u8]) -> io::Result<()> {
        self.w.lock().await.write_all(&frame(OP_BINARY, data)).await
    }
    async fn send_op(&self, op: u8, data: &[u8]) -> io::Result<()> {
        self.w.lock().await.write_all(&frame(op, data)).await
    }
    pub async fn close(&self) {
        let _ = self.send_op(OP_CLOSE, &[]).await;
        let _ = self.w.lock().await.shutdown().await;
    }
}

pub struct WsReader {
    r: BufReader<ReadHalf<Tls>>,
    w: WsWriter,
    frag: Vec<u8>,
}

impl WsReader {
    /// Следующее сообщение; None — сервер закрыл соединение.
    pub async fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let mut h = [0u8; 2];
            if let Err(e) = self.r.read_exact(&mut h).await {
                return if e.kind() == io::ErrorKind::UnexpectedEof { Ok(None) } else { Err(e) };
            }
            let fin = h[0] & 0x80 != 0;
            let op = h[0] & 0x0f;
            let mut len = (h[1] & 0x7f) as usize;
            if len == 126 {
                let mut b = [0u8; 2];
                self.r.read_exact(&mut b).await?;
                len = u16::from_be_bytes(b) as usize;
            } else if len == 127 {
                let mut b = [0u8; 8];
                self.r.read_exact(&mut b).await?;
                len = u64::from_be_bytes(b) as usize;
            }
            if len > MAX_MSG {
                return Err(err("слишком большой кадр"));
            }
            let mask = if h[1] & 0x80 != 0 {
                let mut m = [0u8; 4];
                self.r.read_exact(&mut m).await?;
                Some(m)
            } else {
                None
            };
            let mut payload = vec![0u8; len];
            self.r.read_exact(&mut payload).await?;
            if let Some(m) = mask {
                payload.iter_mut().enumerate().for_each(|(i, b)| *b ^= m[i & 3]);
            }
            match op {
                OP_CLOSE => {
                    let _ = self.w.send_op(OP_CLOSE, payload.get(..2).unwrap_or(&[])).await;
                    return Ok(None);
                }
                OP_PING => {
                    self.w.send_op(OP_PONG, &payload).await?;
                }
                OP_PONG => {}
                OP_CONT | OP_TEXT | OP_BINARY => {
                    if fin && self.frag.is_empty() {
                        return Ok(Some(payload));
                    }
                    self.frag.extend_from_slice(&payload);
                    if self.frag.len() > MAX_MSG {
                        return Err(err("слишком большое сообщение"));
                    }
                    if fin {
                        return Ok(Some(std::mem::take(&mut self.frag)));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_frame_layout() {
        let f = frame(OP_BINARY, &[1, 2, 3]);
        assert_eq!(f[0], 0x82);
        assert_eq!(f[1], 0x80 | 3);
        let m = &f[2..6];
        assert_eq!([f[6] ^ m[0], f[7] ^ m[1], f[8] ^ m[2]], [1, 2, 3]);
        let big = frame(OP_BINARY, &vec![0u8; 70000]);
        assert_eq!(big[1], 0x80 | 127);
    }
}
