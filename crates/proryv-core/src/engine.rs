//! Движок: поток, который читает перехваченные пакеты, находит первые
//! пакеты соединений к нужным сайтам и заменяет их на план стратегии.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Settings,
    http,
    packet::{self, TcpInfo},
    platform::{Filter, Net, PacketIo, PlatformError},
    presets,
    strategy::{self, Desync, FakeSni, SegKind, Segment, Strategy},
    tls,
};

/// Максимальный размер данных в одном отправляемом сегменте (запас до MTU 1500).
const MAX_SEG: usize = 1360;
/// Сколько держим первый кусок ClientHello, ожидая второй.
const HOLD_TIMEOUT: Duration = Duration::from_millis(400);

/// Правило: какие домены обрабатывать и какой стратегией.
#[derive(Clone, Debug)]
pub struct Rule {
    pub preset_id: String,
    /// Пусто — все сайты.
    pub domains: Vec<String>,
    pub strategy: Strategy,
    pub ip_id_zero: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RunConfig {
    pub rules: Vec<Rule>,
    pub excluded: Vec<String>,
    pub tcp_ports: Vec<u16>,
    pub random_fakes: bool,
    pub block_quic: bool,
    /// Голос Discord: UDP-порты, где перед discord/stun шлём фейки.
    pub voice_ports: Vec<(u16, u16)>,
    pub game_tcp: bool,
    pub game_udp: bool,
    pub game_nets: Vec<Net>,
}

pub const DISCORD_VOICE_PORTS: [(u16, u16); 2] = [(19294, 19344), (50000, 50100)];
/// Сколько первых пакетов UDP-потока (или TCP для Game Filter) обрабатывать (cutoff в zapret).
const UDP_CUTOFF: u32 = 2;
const VOICE_REPEATS: usize = 6;
const GAME_UDP_REPEATS: usize = 12;
const GAME_TCP_REPEATS: usize = 6;

fn norm_domain(d: &str) -> String {
    d.trim().trim_start_matches("*.").trim_start_matches('.').to_ascii_lowercase()
}

pub fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || (host.len() > domain.len() && host.ends_with(domain) && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
}

impl RunConfig {
    pub fn from_settings(s: &Settings) -> RunConfig {
        let strategies = crate::base::strategies();
        let mut rules = Vec::new();
        let mut ports = Vec::new();
        for p in crate::base::presets() {
            if !s.enabled_presets.contains(&p.id) {
                continue;
            }
            let sid = s
                .strategy_override
                .iter()
                .find(|(pid, _)| *pid == p.id)
                .map(|(_, sid)| sid.clone())
                .unwrap_or_else(|| p.strategies[0].clone());
            let Some(strategy) = strategies.iter().find(|x| x.id == sid).cloned() else { continue };
            let mut domains: Vec<String> = p.domains.iter().map(|d| norm_domain(d)).collect();
            if p.id == "universal" {
                domains = s.custom_domains.iter().map(|d| norm_domain(d)).filter(|d| !d.is_empty()).collect();
                // Пустой список — не «все сайты подряд» (это ломало VPN и прочее), а ничего.
                if domains.is_empty() {
                    continue;
                }
            } else {
                // Свои домены добавляем и к обычным пресетам — так их проще включить.
                domains.extend(s.custom_domains.iter().map(|d| norm_domain(d)).filter(|d| !d.is_empty()));
            }
            ports.extend(p.tcp_ports.iter().copied());
            rules.push(Rule { preset_id: p.id, domains, strategy, ip_id_zero: p.ip_id_zero });
        }
        // Универсальный пресет (все сайты) — в конец, чтобы конкретные правила имели приоритет.
        rules.sort_by_key(|r| r.domains.is_empty());
        ports.sort_unstable();
        ports.dedup();
        let mut excluded: Vec<String> = presets::BUILTIN_EXCLUDES.iter().map(|d| d.to_string()).collect();
        excluded.extend(s.excluded_domains.iter().map(|d| norm_domain(d)).filter(|d| !d.is_empty()));
        // Голос — часть пресета Discord: включён пресет — работает и голос.
        let voice_ports = if s.enabled_presets.iter().any(|p| p == "discord") {
            DISCORD_VOICE_PORTS.to_vec()
        } else {
            vec![]
        };
        let game_nets: Vec<Net> = s.game_ips.iter().filter_map(|c| crate::platform::parse_cidr(c)).collect();
        let gm = s.game_filter.as_str();
        RunConfig {
            rules,
            excluded,
            tcp_ports: ports,
            random_fakes: s.random_fakes,
            block_quic: s.block_quic,
            voice_ports,
            game_tcp: matches!(gm, "tcp" | "all") && !game_nets.is_empty(),
            game_udp: matches!(gm, "udp" | "all") && !game_nets.is_empty(),
            game_nets,
        }
    }

    pub fn filter(&self) -> Filter {
        Filter {
            tcp_ports: self.tcp_ports.clone(),
            block_quic: self.block_quic,
            exclude_ifs: vec![],
            udp_ranges: self.voice_ports.clone(),
            game_tcp: self.game_tcp,
            game_udp: self.game_udp,
            game_nets: self.game_nets.clone(),
            syn_ack: self.rules.iter().any(|r| r.strategy.steps.iter().any(|s| matches!(s, Desync::AutoTtl { .. }))),
        }
    }

    /// QUIC режем только к Google/YouTube по IPv4: UDP 443 нужен VPN-протоколам.
    fn is_google(&self, ip: Option<u128>) -> bool {
        ip.is_some_and(|ip| crate::platform::google_nets().iter().any(|n| n.contains(ip)))
    }

    fn in_game_nets(&self, ip: Option<u128>) -> bool {
        ip.is_some_and(|ip| self.game_nets.iter().any(|n| n.contains(ip)))
    }

    fn rule_for(&self, host: &str) -> Option<&Rule> {
        let host = host.to_ascii_lowercase();
        if self.excluded.iter().any(|d| host_matches(&host, d)) {
            return None;
        }
        self.rules
            .iter()
            .find(|r| r.domains.is_empty() || r.domains.iter().any(|d| host_matches(&host, d)))
    }
}

#[derive(Default)]
pub struct Counters {
    pub connections: AtomicU64,
    pub desynced: AtomicU64,
    pub fakes_sent: AtomicU64,
    pub quic_blocked: AtomicU64,
    pub udp_desynced: AtomicU64,
    pub last_host: Mutex<Option<String>>,
    /// По пресетам: (обработано ClientHello, из них повторных — признак, что не прошло).
    pub health: Mutex<HashMap<String, (u64, u64)>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Stats {
    pub connections: u64,
    pub desynced: u64,
    pub fakes_sent: u64,
    pub uptime_secs: u64,
    pub last_host: Option<String>,
}

#[derive(Debug)]
pub struct Outgoing {
    pub data: Vec<u8>,
    pub addr: Vec<u8>,
    /// Пакет изменён нами — контрольные суммы посчитаны, драйвер не должен их трогать.
    pub modified: bool,
}

/// Первый кусок длинного ClientHello, который ждёт продолжения.
pub struct Held {
    packet: Vec<u8>,
    addr: Vec<u8>,
    info: TcpInfo,
    at: Instant,
}

/// Общая с потоком-сторожем таблица удержанных кусков: (v6, sport, dport).
pub type HeldMap = Arc<Mutex<HashMap<(bool, u16, u16), Held>>>;

/// Отпустить куски, продолжение которых так и не пришло. Зовётся сторожем по таймеру,
/// чтобы соединение не висело до следующего перехваченного пакета.
pub fn release_expired(held: &HeldMap, io: &dyn PacketIo) {
    let now = Instant::now();
    let expired: Vec<Held> = {
        let mut map = held.lock().unwrap();
        let keys: Vec<_> = map.iter().filter(|(_, h)| now - h.at > HOLD_TIMEOUT).map(|(k, _)| *k).collect();
        keys.into_iter().filter_map(|k| map.remove(&k)).collect()
    };
    for h in expired {
        let _ = io.send(&h.packet, &h.addr);
    }
}

pub struct Processor {
    cfg: RunConfig,
    stats: Arc<Counters>,
    held: HeldMap,
    /// Счётчик пакетов по потокам (UDP и Game Filter TCP): (proto, v6, sport, dport, dst_ip).
    flows: HashMap<(u8, bool, u16, u16, u128), (u32, Instant)>,
    /// Недавние ClientHello: (v6, sport, dport, seq) — чтобы ловить повторы (самолечение).
    recent: HashMap<(bool, u16, u16, u32), Instant>,
    /// autottl: TTL из SYN-ACK по адресу сервера.
    server_ttl: HashMap<u128, (u8, Instant)>,
    packets: u64,
    seed: u64,
    pub trace: Option<Trace>,
}

/// Журнал движка: последние строки и номер первой из них, чтобы читать «с места».
#[derive(Default)]
pub struct TraceBuf {
    pub first: u64,
    pub lines: std::collections::VecDeque<String>,
}

impl TraceBuf {
    const CAP: usize = 1000;

    pub fn push(&mut self, line: String) {
        self.lines.push_back(line);
        if self.lines.len() > Self::CAP {
            self.lines.pop_front();
            self.first += 1;
        }
    }

    /// Строки начиная с номера `since` и номер, с которого читать в следующий раз.
    pub fn since(&self, since: u64) -> (u64, Vec<String>) {
        let skip = since.saturating_sub(self.first) as usize;
        (self.first + self.lines.len() as u64, self.lines.iter().skip(skip).cloned().collect())
    }
}

pub type Trace = Arc<Mutex<TraceBuf>>;

fn pass(data: &[u8], addr: &[u8]) -> Outgoing {
    Outgoing { data: data.to_vec(), addr: addr.to_vec(), modified: false }
}

impl Processor {
    pub fn new(mut cfg: RunConfig, stats: Arc<Counters>) -> Self {
        if !cfg.random_fakes {
            for r in &mut cfg.rules {
                for s in &mut r.strategy.steps {
                    if let Desync::Fake { sni, .. } = s {
                        *sni = FakeSni::Fixed("www.google.com".into());
                    }
                }
            }
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let seed = now.as_nanos() as u64 ^ 0x2545_f491_4f6c_dd1d ^ std::process::id() as u64;
        Processor { cfg, stats, held: HeldMap::default(), flows: HashMap::new(), recent: HashMap::new(), server_ttl: HashMap::new(), packets: 0, seed, trace: None }
    }

    fn flow_index(&mut self, key: (u8, bool, u16, u16, u128), now: Instant) -> u32 {
        let e = self.flows.entry(key).or_insert((0, now));
        if now - e.1 > Duration::from_secs(60) {
            *e = (0, now); // поток давно молчал — считаем новым
        }
        let i = e.0;
        e.0 = e.0.saturating_add(1);
        e.1 = now;
        i
    }

    fn housekeeping(&mut self, now: Instant) {
        self.packets += 1;
        if self.packets % 2048 == 0 {
            self.flows.retain(|_, v| now - v.1 < Duration::from_secs(120));
            self.recent.retain(|_, t| now - *t < Duration::from_secs(20));
            self.server_ttl.retain(|_, v| now - v.1 < Duration::from_secs(600));
        }
    }

    /// UDP: голос Discord и Game Filter — фейки перед первыми пакетами потока.
    fn handle_udp(&mut self, pkt: &[u8], addr: &[u8], info: packet::UdpInfo, now: Instant) -> Vec<Outgoing> {
        let payload = &pkt[info.payload_off()..];
        let dst = packet::dst_ip(pkt);
        let voice = self.cfg.voice_ports.iter().any(|(a, b)| info.dst_port >= *a && info.dst_port <= *b)
            && packet::is_discord_or_stun(payload);
        let game = self.cfg.game_udp && info.dst_port >= 1024 && self.cfg.in_game_nets(dst);
        let mut out = Vec::new();
        if voice || game {
            let key = (17, info.v6, info.src_port, info.dst_port, dst.unwrap_or(0));
            let idx = self.flow_index(key, now);
            if voice || idx < UDP_CUTOFF {
                let repeats = if voice { VOICE_REPEATS } else { GAME_UDP_REPEATS };
                // Фейк — настоящий QUIC Initial к google.com (как ACTIVE_DISCORD_UDP.bin в zapret).
                let mut rnd = [0u8; 64];
                for c in rnd.chunks_mut(8) {
                    self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    c.copy_from_slice(&self.seed.to_le_bytes());
                }
                let fake = crate::quic::build_initial("www.google.com", &rnd);
                for _ in 0..repeats {
                    out.push(Outgoing { data: packet::build_udp(pkt, &info, &fake), addr: addr.to_vec(), modified: true });
                }
                self.stats.fakes_sent.fetch_add(repeats as u64, Relaxed);
                self.stats.udp_desynced.fetch_add(1, Relaxed);
                self.log(|| format!("UDP {} port {} pkt#{idx}: {repeats} фейков", if voice { "voice" } else { "game" }, info.dst_port));
            }
        }
        out.push(pass(pkt, addr));
        out
    }

    /// Game Filter TCP: первые данные игрового соединения (не TLS/HTTP) — фейки с ts.
    fn game_tcp(&mut self, pkt: &[u8], addr: &[u8], info: &TcpInfo, now: Instant) -> Option<Vec<Outgoing>> {
        let dst = packet::dst_ip(pkt);
        if !(self.cfg.game_tcp && info.dst_port >= 1024 && self.cfg.in_game_nets(dst)) {
            return None;
        }
        let idx = self.flow_index((6, info.v6, info.src_port, info.dst_port, dst.unwrap_or(0)), now);
        if idx >= 1 {
            return None;
        }
        let fake = Segment {
            kind: SegKind::Fake,
            seq_offset: 0,
            data: vec![0u8; 64],
            ttl: None,
            fooling: vec![strategy::Fooling::Ts],
            label: String::new(),
        };
        let mut out: Vec<Outgoing> = (0..GAME_TCP_REPEATS)
            .map(|_| Outgoing { data: packet::build_segment(pkt, info, &fake), addr: addr.to_vec(), modified: true })
            .collect();
        self.stats.fakes_sent.fetch_add(GAME_TCP_REPEATS as u64, Relaxed);
        out.push(pass(pkt, addr));
        Some(out)
    }

    fn log(&self, msg: impl FnOnce() -> String) {
        if let Some(t) = &self.trace {
            t.lock().unwrap().push(msg());
        }
    }

    pub fn held(&self) -> HeldMap {
        self.held.clone()
    }

    pub fn handle(&mut self, pkt: &[u8], addr: &[u8]) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let now = Instant::now();

        // QUIC отбрасываем: браузер через ~секунду перейдёт на TCP, где работает обход.
        if self.cfg.block_quic && packet::is_quic(pkt) && self.cfg.is_google(packet::dst_ip(pkt)) {
            self.stats.quic_blocked.fetch_add(1, Relaxed);
            return out;
        }
        self.housekeeping(now);
        // Входящий SYN-ACK перехватываем только ради autottl: запомнить TTL и отпустить.
        if let Some((ip, ttl)) = packet::syn_ack_ttl(pkt) {
            self.server_ttl.insert(ip, (ttl, now));
            out.push(pass(pkt, addr));
            return out;
        }
        if let Some(u) = packet::parse_udp(pkt) {
            out.extend(self.handle_udp(pkt, addr, u, now));
            return out;
        }

        let Some(info) = packet::parse_tcp(pkt) else {
            out.push(pass(pkt, addr));
            return out;
        };
        let key = (info.v6, info.src_port, info.dst_port);
        let payload = &pkt[info.payload_off()..];

        let held = self.held.lock().unwrap().remove(&key);
        if let Some(h) = held {
            let first_len = h.packet.len() - h.info.payload_off();
            if info.seq == h.info.seq.wrapping_add(first_len as u32) {
                let mut combined = h.packet[h.info.payload_off()..].to_vec();
                combined.extend_from_slice(payload);
                self.log(|| format!("combined {}+{} = {} B, sni={:?}", first_len, payload.len(), combined.len(), tls::parse_sni(&combined).map(|s| s.host)));
                if let Some(res) = self.try_desync(&h.packet, &h.info, &combined, &h.addr) {
                    out.extend(res);
                } else {
                    out.push(pass(&h.packet, &h.addr));
                    out.push(pass(pkt, addr));
                }
                return out;
            }
            self.log(|| format!("held mismatch: seq {} != {}+{}", info.seq, h.info.seq, first_len));
            out.push(pass(&h.packet, &h.addr));
        }

        let is_tls = tls::is_client_hello(payload);
        if is_tls || http::is_http_request(payload) {
            self.stats.connections.fetch_add(1, Relaxed);
            if let Some(res) = self.try_desync(pkt, &info, payload, addr) {
                out.extend(res);
                return out;
            }
            // SNI не попал в первый пакет (длинный ClientHello Chrome) — ждём продолжение.
            if is_tls && tls::parse_sni(payload).is_none() {
                let record_len = 5 + ((payload[3] as usize) << 8 | payload[4] as usize);
                if record_len > payload.len() {
                    self.log(|| format!("hold: hello {} of {} B, no SNI yet (port {})", payload.len(), record_len, info.src_port));
                    self.held.lock().unwrap().insert(key, Held { packet: pkt.to_vec(), addr: addr.to_vec(), info, at: now });
                    return out;
                }
            }
            let host = tls::parse_sni(payload).map(|s| s.host).or_else(|| http::parse_host(payload).map(|h| h.host));
            self.log(|| format!("pass {:?} ({} B)", host, payload.len()));
        } else if let Some(res) = self.game_tcp(pkt, addr, &info, now) {
            out.extend(res);
            return out;
        }
        out.push(pass(pkt, addr));
        out
    }

    /// Применить стратегию, если сайт из наших списков. `None` — не трогаем.
    fn try_desync(&mut self, pkt: &[u8], info: &TcpInfo, payload: &[u8], addr: &[u8]) -> Option<Vec<Outgoing>> {
        let host = tls::parse_sni(payload).map(|s| s.host).or_else(|| http::parse_host(payload).map(|h| h.host))?;
        let rule = self.cfg.rule_for(&host)?;
        let ip_id_zero = rule.ip_id_zero;
        // Самолечение: тот же ClientHello (тот же seq) пришёл снова — значит, прошлый не прошёл.
        let retrans = {
            let now = Instant::now();
            let rkey = (info.v6, info.src_port, info.dst_port, info.seq);
            let retrans = self.recent.get(&rkey).is_some_and(|t| now - *t < Duration::from_secs(15));
            self.recent.insert(rkey, now);
            let mut h = self.stats.health.lock().unwrap();
            let e = h.entry(rule.preset_id.clone()).or_insert((0, 0));
            e.0 += 1;
            if retrans {
                e.1 += 1;
            }
            retrans
        };
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(info.seq as u64 | 1);
        let mut plan = strategy::plan(payload, &rule.strategy, self.seed);
        let auto = rule.strategy.steps.iter().find_map(|s| match s {
            Desync::AutoTtl { delta, min, max } => Some((*delta, *min, *max)),
            _ => None,
        });
        if let Some((delta, min, max)) = auto {
            let seen = packet::dst_ip(pkt).and_then(|ip| self.server_ttl.get(&ip)).map(|v| v.0);
            if let Some(observed) = seen {
                let ttl = strategy::auto_ttl(observed, delta, min, max);
                for s in plan.segments.iter_mut().filter(|s| s.kind == SegKind::Fake) {
                    s.ttl = Some(ttl);
                }
                self.log(|| format!("autottl: ответ с TTL {observed} → фейкам TTL {ttl}"));
            } else {
                self.log(|| "autottl: SYN-ACK не видели — фейки без поправки TTL".into());
            }
        }

        let segs = split_oversized(plan.segments);
        self.log(|| {
            let fakes = segs.iter().filter(|s| s.kind == SegKind::Fake).count();
            let fail = if retrans { " · ПОВТОР — прошлый не прошёл" } else { "" };
            let parts = segs.len() - fakes;
            format!(
                "{host} → {} / {} · {fakes} {} + {parts} {}{fail}",
                rule.preset_id,
                rule.strategy.name,
                ru_plural(fakes, ["фейк", "фейка", "фейков"]),
                ru_plural(parts, ["часть", "части", "частей"]),
            )
        });
        let mut out = Vec::new();
        for seg in segs {
            if seg.kind == SegKind::Fake {
                self.stats.fakes_sent.fetch_add(1, Relaxed);
            }
            out.push(Outgoing {
                data: packet::build_segment_ex(pkt, info, &seg, ip_id_zero),
                addr: addr.to_vec(),
                modified: true,
            });
        }
        self.stats.desynced.fetch_add(1, Relaxed);
        *self.stats.last_host.lock().unwrap() = Some(host);
        Some(out)
    }
}

fn ru_plural(n: usize, forms: [&'static str; 3]) -> &'static str {
    match (n % 10, n % 100) {
        (1, r) if r != 11 => forms[0],
        (2..=4, r) if !(12..=14).contains(&r) => forms[1],
        _ => forms[2],
    }
}

/// Сегменты больше MTU режем дальше; фейки просто укорачиваем.
fn split_oversized(segs: Vec<Segment>) -> Vec<Segment> {
    let mut out = Vec::new();
    for mut s in segs {
        if s.data.len() <= MAX_SEG {
            out.push(s);
        } else if s.kind == SegKind::Fake {
            s.data.truncate(MAX_SEG);
            out.push(s);
        } else {
            for (i, chunk) in s.data.chunks(MAX_SEG).enumerate() {
                out.push(Segment {
                    seq_offset: s.seq_offset.wrapping_add((i * MAX_SEG) as u32),
                    data: chunk.to_vec(),
                    ..s.clone()
                });
            }
        }
    }
    out
}

/// Главный цикл: читать → обработать → отправить. Возвращается после `shutdown`.
pub fn run_loop(io: &dyn PacketIo, proc: &mut Processor) -> Result<(), PlatformError> {
    let mut buf = vec![0u8; 0xffff];
    let mut addr = vec![0u8; io.addr_len()];
    loop {
        let n = match io.recv(&mut buf, &mut addr) {
            Ok(n) => n,
            Err(PlatformError::Shutdown) => return Ok(()),
            Err(e) => return Err(e),
        };
        for mut o in proc.handle(&buf[..n], &addr) {
            if o.modified {
                io.mark_checksums_valid(&mut o.addr);
            }
            let _ = io.send(&o.data, &o.addr);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    Stopped,
    Running,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub state: EngineState,
    pub presets: Vec<String>,
    pub message: Option<String>,
    pub stats: Stats,
    pub demo: bool,
}

#[derive(Default)]
pub struct Engine {
    io: Option<Arc<dyn PacketIo>>,
    thread: Option<JoinHandle<()>>,
    watchdog: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
    presets: Vec<String>,
    started: Option<Instant>,
    stats: Arc<Counters>,
    error: Arc<Mutex<Option<String>>>,
    pub trace: Option<Trace>,
}

impl Engine {
    pub fn start(&mut self, cfg: RunConfig, io: Arc<dyn PacketIo>) {
        // Перезапуск с новыми настройками — продолжение той же сессии: время и счётчики не сбрасываем.
        let resumed = self.started.filter(|_| self.error.lock().unwrap().is_none());
        self.stop();
        *self.error.lock().unwrap() = None;
        if resumed.is_none() {
            self.stats = Arc::new(Counters::default());
        }
        self.presets = cfg.rules.iter().map(|r| r.preset_id.clone()).collect();
        let mut proc = Processor::new(cfg, self.stats.clone());
        proc.trace = self.trace.clone();
        let (held, io3, stop) = (proc.held(), io.clone(), Arc::new(AtomicBool::new(false)));
        let stop2 = stop.clone();
        let watchdog = std::thread::Builder::new()
            .name("proryv-held".into())
            .spawn(move || {
                while !stop2.load(Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                    release_expired(&held, io3.as_ref());
                }
            })
            .expect("spawn watchdog");
        self.watchdog = Some((stop, watchdog));
        let (io2, err) = (io.clone(), self.error.clone());
        self.thread = Some(
            std::thread::Builder::new()
                .name("proryv-engine".into())
                .spawn(move || {
                    if let Err(e) = run_loop(io2.as_ref(), &mut proc) {
                        *err.lock().unwrap() = Some(e.to_string());
                    }
                })
                .expect("spawn engine"),
        );
        self.io = Some(io);
        self.started = Some(resumed.unwrap_or_else(Instant::now));
    }

    /// Остановить на время (автоподбор): следующий `start` продолжит ту же сессию.
    pub fn pause(&mut self) {
        let started = self.started;
        self.stop();
        self.started = started;
    }

    pub fn fail(&mut self, message: String) {
        self.stop();
        *self.error.lock().unwrap() = Some(message);
    }

    /// По пресетам: (обработано ClientHello, повторов). Растущая доля повторов — стратегия перестала работать.
    pub fn health(&self) -> HashMap<String, (u64, u64)> {
        self.stats.health.lock().unwrap().clone()
    }

    pub fn stop(&mut self) {
        if let Some(io) = self.io.take() {
            io.shutdown();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some((stop, t)) = self.watchdog.take() {
            stop.store(true, Relaxed);
            let _ = t.join();
        }
        self.started = None;
    }

    pub fn status(&self) -> Status {
        let error = self.error.lock().unwrap().clone();
        let running = self.thread.as_ref().is_some_and(|t| !t.is_finished());
        let state = if running { EngineState::Running } else if error.is_some() { EngineState::Error } else { EngineState::Stopped };
        Status {
            state,
            presets: self.presets.clone(),
            message: error,
            stats: Stats {
                connections: self.stats.connections.load(Relaxed),
                desynced: self.stats.desynced.load(Relaxed),
                fakes_sent: self.stats.fakes_sent.load(Relaxed),
                uptime_secs: if running { self.started.map(|t| t.elapsed().as_secs()).unwrap_or(0) } else { 0 },
                last_host: self.stats.last_host.lock().unwrap().clone(),
            },
            demo: false,
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::tests::make_v4;

    fn processor() -> Processor {
        // Стратегия с фейком (TTL 4) и разрезом — удобно проверять сборку обратно.
        let s = Settings {
            enabled_presets: vec!["youtube".into()],
            strategy_override: vec![("youtube".into(), "fake_ttl_split".into())],
            ..Default::default()
        };
        Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()))
    }

    fn reassemble(out: &[Outgoing]) -> Vec<u8> {
        let mut real: Vec<(u32, Vec<u8>)> = out
            .iter()
            .map(|o| (packet::parse_tcp(&o.data).unwrap(), &o.data))
            .filter(|(i, d)| d[8] != 4 && i.seq < 0x8000_0000) // без фейков (TTL 4)
            .map(|(i, d)| (i.seq, d[i.payload_off()..].to_vec()))
            .collect();
        real.sort();
        real.into_iter().flat_map(|(_, d)| d).collect()
    }

    #[test]
    fn domain_matching() {
        assert!(host_matches("rr3.googlevideo.com", "googlevideo.com"));
        assert!(host_matches("youtube.com", "youtube.com"));
        assert!(!host_matches("notyoutube.com", "youtube.com"));
    }

    #[test]
    fn youtube_hello_is_desynced_and_reassembles() {
        let hello = tls::build_client_hello("www.youtube.com");
        let pkt = make_v4(&hello, 1000);
        let mut p = processor();
        let out = p.handle(&pkt, &[0; 80]);
        assert!(out.len() > 2, "ожидали фейки + куски, получили {}", out.len());
        assert!(out.iter().all(|o| o.modified));
        assert_eq!(reassemble(&out), hello);
    }

    #[test]
    fn youtube_over_ipv6_is_desynced_and_reassembles() {
        use crate::packet::tests::{make_v6, v6_tcp_checksum_valid};
        let hello = tls::build_client_hello("www.youtube.com");
        let mut p = processor();
        let out = p.handle(&make_v6(&hello, 1000), &[0; 80]);
        assert!(out.len() > 2 && out.iter().all(|o| o.modified));
        assert!(out.iter().all(|o| o.data[0] >> 4 == 6 && v6_tcp_checksum_valid(&o.data)));
        let mut real: Vec<(u32, Vec<u8>)> = out
            .iter()
            .filter(|o| o.data[7] != 4)
            .map(|o| {
                let i = packet::parse_tcp(&o.data).unwrap();
                (i.seq, o.data[i.payload_off()..].to_vec())
            })
            .collect();
        real.sort();
        assert_eq!(real.into_iter().flat_map(|(_, d)| d).collect::<Vec<u8>>(), hello);
    }

    #[test]
    fn long_hello_over_ipv6_is_held_then_desynced() {
        use crate::packet::tests::make_v6;
        let hello = tls::build_client_hello("www.youtube.com");
        let (a, b) = hello.split_at(60);
        let mut p = processor();
        assert!(p.handle(&make_v6(a, 1000), &[0; 80]).is_empty());
        let out = p.handle(&make_v6(b, 1060), &[0; 80]);
        assert!(out.len() > 2 && out.iter().all(|o| o.modified));
    }

    #[test]
    fn alt7_layout() {
        let s = Settings {
            enabled_presets: vec!["youtube".into()],
            strategy_override: vec![("youtube".into(), "alt7".into())],
            ..Default::default()
        };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        let hello = tls::build_client_hello("rr1.googlevideo.com");
        let sni = tls::parse_sni(&hello).unwrap();
        let out = p.handle(&make_v4(&hello, 5000), &[0; 80]);
        let seqs: Vec<u32> = out.iter().map(|o| packet::parse_tcp(&o.data).unwrap().seq).collect();
        let sniext = (sni.offset - 9 + 1) as u32;
        assert_eq!(seqs, vec![5000u32.wrapping_sub(679), 5002, 5000 + sniext]);
        // первый пакет = 679 байт фейка + 2 настоящих байта
        let i = packet::parse_tcp(&out[0].data).unwrap();
        assert_eq!(out[0].data.len() - i.payload_off(), 681);
    }

    #[test]
    fn default_youtube_strategy_is_alt11() {
        let cfg = RunConfig::from_settings(&Settings { enabled_presets: vec!["youtube".into()], ..Default::default() });
        assert_eq!(cfg.rules[0].strategy.id, "alt11");
    }

    #[test]
    fn quic_is_dropped_and_google_gets_zero_ip_id() {
        let s = Settings { enabled_presets: vec!["youtube".into()], ..Default::default() };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        // QUIC к YouTube (142.250.x.x) — отбрасываем.
        let mut udp = vec![0x45, 0, 0, 36, 0, 1, 0, 0, 64, 17, 0, 0, 10, 0, 0, 1, 142, 250, 1, 1];
        udp.extend_from_slice(&[0xc3, 0x50, 0x01, 0xbb, 0, 16, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(p.handle(&udp, &[0; 80]).is_empty(), "QUIC к Google должен отбрасываться");
        // UDP 443 к любому другому серверу (например, VPN на Hysteria2) — пропускаем.
        let mut vpn = udp.clone();
        vpn[16..20].copy_from_slice(&[45, 12, 34, 56]);
        assert_eq!(p.handle(&vpn, &[0; 80]).len(), 1, "UDP 443 к VPN нельзя трогать");
        let out = p.handle(&make_v4(&tls::build_client_hello("jnn-pa.googleapis.com"), 9), &[0; 80]);
        assert!(out.iter().all(|o| o.modified && o.data[4] == 0 && o.data[5] == 0));
    }

    #[test]
    fn quic_to_google_over_ipv6_is_dropped_vpn_is_not() {
        use crate::packet::tests::make_udp6;
        let s = Settings { enabled_presets: vec!["youtube".into()], ..Default::default() };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        let quic = make_udp6(&[0xc3; 40], "2a00:1450:4010:c0e::5d", 443);
        assert!(p.handle(&quic, &[0; 80]).is_empty(), "QUIC к Google по IPv6 должен отбрасываться");
        let vpn = make_udp6(&[0xc3; 40], "2a01:4f8::1", 443);
        assert_eq!(p.handle(&vpn, &[0; 80]).len(), 1, "UDP 443 к VPN по IPv6 нельзя трогать");
    }

    #[test]
    fn game_filter_accepts_ipv6_ranges() {
        use crate::packet::tests::make_udp6;
        let s = Settings { enabled_presets: vec![], game_filter: "udp".into(), game_ips: vec!["2620:2b:e000::/40".into()], ..Default::default() };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        let game = make_udp6(&[1; 40], "2620:2b:e0aa::7", 55000);
        assert_eq!(p.handle(&game, &[0; 80]).len(), GAME_UDP_REPEATS + 1);
        assert_eq!(p.handle(&make_udp6(&[1; 40], "2620:2c::1", 55000), &[0; 80]).len(), 1, "чужая сеть");
    }

    #[test]
    fn builtin_excludes_apply() {
        let s = Settings { enabled_presets: vec!["universal".into()], custom_domains: vec!["sberbank.ru".into()], ..Default::default() };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        let out = p.handle(&make_v4(&tls::build_client_hello("online.sberbank.ru"), 1), &[0; 80]);
        assert!(!out[0].modified);
    }

    #[test]
    fn other_sites_pass_untouched() {
        let pkt = make_v4(&tls::build_client_hello("example.com"), 1);
        let out = processor().handle(&pkt, &[0; 80]);
        assert_eq!(out.len(), 1);
        assert!(!out[0].modified);
        assert_eq!(out[0].data, pkt);
    }

    #[test]
    fn excluded_wins() {
        let s = Settings {
            enabled_presets: vec!["universal".into()],
            custom_domains: vec!["bank.ru".into(), "any.site".into()],
            excluded_domains: vec!["bank.ru".into()],
            ..Default::default()
        };
        let mut p = Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()));
        let out = p.handle(&make_v4(&tls::build_client_hello("online.bank.ru"), 1), &[0; 80]);
        assert!(!out[0].modified);
        let out = p.handle(&make_v4(&tls::build_client_hello("any.site"), 1), &[0; 80]);
        assert!(out[0].modified);
    }

    #[test]
    fn discord_voice_gets_udp_fakes() {
        use crate::packet::tests::make_udp4;
        let s = Settings { enabled_presets: vec!["discord".into()], ..Default::default() };
        let cfg = RunConfig::from_settings(&s);
        assert_eq!(cfg.voice_ports, DISCORD_VOICE_PORTS.to_vec());
        let mut p = Processor::new(cfg, Arc::new(Counters::default()));
        let mut disc = vec![0u8; 74];
        disc[..4].copy_from_slice(&[0, 1, 0, 0x46]);
        let out = p.handle(&make_udp4(&disc, [66, 22, 200, 1], 50010), &[0; 80]);
        assert_eq!(out.len(), VOICE_REPEATS + 1);
        assert!(!out.last().unwrap().modified, "оригинал уходит последним и без изменений");
        // Обычный голосовой трафик (не discovery/STUN) не трогаем.
        let out = p.handle(&make_udp4(&[0x90; 100], [66, 22, 200, 1], 50010), &[0; 80]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn game_filter_modes() {
        use crate::packet::tests::make_udp4;
        let mk = |mode: &str| {
            let s = Settings { enabled_presets: vec![], game_filter: mode.into(), ..Default::default() };
            Processor::new(RunConfig::from_settings(&s), Arc::new(Counters::default()))
        };
        // UDP к Roblox (128.116.x.x): фейки только на первых двух пакетах потока.
        let mut p = mk("udp");
        let pkt = make_udp4(&[1; 40], [128, 116, 5, 9], 55000);
        assert_eq!(p.handle(&pkt, &[0; 80]).len(), GAME_UDP_REPEATS + 1);
        assert_eq!(p.handle(&pkt, &[0; 80]).len(), GAME_UDP_REPEATS + 1);
        assert_eq!(p.handle(&pkt, &[0; 80]).len(), 1);
        assert_eq!(p.handle(&make_udp4(&[1; 40], [8, 8, 8, 8], 55000), &[0; 80]).len(), 1);
        assert_eq!(mk("off").handle(&pkt, &[0; 80]).len(), 1);
        // TCP-режим: первый пакет данных к игровому серверу получает ts-фейки.
        let mut p = mk("tcp");
        let mut t = make_v4(&[7; 30], 100);
        t[16..20].copy_from_slice(&[128, 116, 1, 1]);
        t[22..24].copy_from_slice(&40000u16.to_be_bytes());
        let info = packet::parse_tcp(&t).unwrap();
        packet::fix_checksums(&mut t, &info);
        assert_eq!(p.handle(&t, &[0; 80]).len(), GAME_TCP_REPEATS + 1);
        assert_eq!(p.handle(&t, &[0; 80]).len(), 1);
    }

    #[test]
    fn retransmitted_hello_counts_as_failure() {
        let s = Settings { enabled_presets: vec!["youtube".into()], ..Default::default() };
        let stats = Arc::new(Counters::default());
        let mut p = Processor::new(RunConfig::from_settings(&s), stats.clone());
        let pkt = make_v4(&tls::build_client_hello("www.youtube.com"), 777);
        p.handle(&pkt, &[0; 80]);
        p.handle(&pkt, &[0; 80]); // повтор того же seq
        assert_eq!(stats.health.lock().unwrap().get("youtube"), Some(&(2, 1)));
    }

    #[test]
    fn empty_universal_touches_nothing() {
        let s = Settings { enabled_presets: vec!["universal".into()], ..Default::default() };
        let cfg = RunConfig::from_settings(&s);
        assert!(cfg.rules.is_empty());
        let mut p = Processor::new(cfg, Arc::new(Counters::default()));
        let out = p.handle(&make_v4(&tls::build_client_hello("vpn-server.example"), 1), &[0; 80]);
        assert!(!out[0].modified);
    }

    #[test]
    fn split_client_hello_is_held_then_desynced() {
        // ClientHello, у которого SNI во втором TCP-пакете: подменяем запись так,
        // чтобы первые 60 байт ушли отдельно.
        let hello = tls::build_client_hello("www.youtube.com");
        let (a, b) = hello.split_at(60);
        let mut p = processor();
        let out1 = p.handle(&make_v4(a, 1000), &[0; 80]);
        assert!(out1.is_empty(), "первый кусок должен быть задержан");
        let out2 = p.handle(&make_v4(b, 1060), &[0; 80]);
        assert!(out2.iter().all(|o| o.modified));
        assert_eq!(reassemble(&out2), hello);
    }

    #[derive(Default)]
    struct RecordingIo(Mutex<Vec<Vec<u8>>>);
    impl PacketIo for RecordingIo {
        fn recv(&self, _: &mut [u8], _: &mut [u8]) -> Result<usize, crate::platform::PlatformError> {
            Err(crate::platform::PlatformError::Shutdown)
        }
        fn send(&self, packet: &[u8], _: &[u8]) -> Result<(), crate::platform::PlatformError> {
            self.0.lock().unwrap().push(packet.to_vec());
            Ok(())
        }
        fn shutdown(&self) {}
        fn addr_len(&self) -> usize {
            80
        }
    }

    #[test]
    fn held_piece_is_released_without_next_packet() {
        let hello = tls::build_client_hello("www.youtube.com");
        let first = make_v4(&hello[..60], 1000);
        let mut p = processor();
        assert!(p.handle(&first, &[0; 80]).is_empty());
        let io = RecordingIo::default();
        release_expired(&p.held(), &io);
        assert!(io.0.lock().unwrap().is_empty(), "до таймаута кусок держим");
        std::thread::sleep(HOLD_TIMEOUT + Duration::from_millis(30));
        release_expired(&p.held(), &io);
        assert_eq!(*io.0.lock().unwrap(), vec![first]);
        assert!(p.held().lock().unwrap().is_empty());
    }
}
