//! Интерфейс перехвата пакетов. Ядро ничего не знает об ОС:
//! на Windows это WinDivert, на Linux/роутерах будет NFQUEUE.

use thiserror::Error;

#[cfg(windows)]
pub mod windivert;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Нужны права администратора: перезапустите программу от имени администратора.")]
    NotElevated,
    #[error("Не найдены файлы WinDivert ({0}). Переустановите программу.")]
    DriverMissing(String),
    #[error("Windows заблокировала драйвер WinDivert ({0}). Проверьте антивирус и «Целостность памяти».")]
    DriverBlocked(String),
    #[error("Перехват остановлен")]
    Shutdown,
    #[error("Ошибка перехвата: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub tcp_ports: Vec<u16>,
    /// Перехватывать исходящий UDP 443 (QUIC), чтобы его отбрасывать.
    pub block_quic: bool,
    /// Индексы сетевых интерфейсов, трафик которых не трогаем (VPN-туннели).
    pub exclude_ifs: Vec<u32>,
    /// Диапазоны UDP-портов (голос Discord).
    pub udp_ranges: Vec<(u16, u16)>,
    /// Game Filter: TCP/UDP на порты 1024–65535 к этим диапазонам IPv4 (start, end).
    pub game_tcp: bool,
    pub game_udp: bool,
    pub game_nets: Vec<(u32, u32)>,
}

/// Сети Google/YouTube. QUIC блокируем только к ним: UDP 443 используют и VPN
/// (Hysteria2, TUIC…), и игры — их трогать нельзя.
pub const GOOGLE_NETS: &[&str] = &[
    "142.250.0.0/15", "172.217.0.0/16", "172.253.0.0/16", "173.194.0.0/16", "74.125.0.0/16",
    "216.58.192.0/19", "209.85.128.0/17", "64.233.160.0/19", "108.177.0.0/17", "192.178.0.0/15",
    "66.102.0.0/20", "66.249.64.0/19", "72.14.192.0/18", "216.239.32.0/19",
];

pub fn google_nets() -> Vec<(u32, u32)> {
    GOOGLE_NETS.iter().filter_map(|c| parse_cidr(c)).collect()
}

fn ip_str(v: u32) -> String {
    std::net::Ipv4Addr::from(v).to_string()
}

/// Условие WinDivert «адрес назначения в одной из сетей».
pub fn nets_condition(nets: &[(u32, u32)]) -> String {
    nets.iter().map(|(a, b)| format!("(ip.DstAddr >= {} and ip.DstAddr <= {})", ip_str(*a), ip_str(*b))).collect::<Vec<_>>().join(" or ")
}

/// "128.116.0.0/17" или "1.2.3.4" → (начало, конец) диапазона.
pub fn parse_cidr(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    let (ip, bits) = match s.split_once('/') {
        Some((ip, b)) => (ip, b.trim().parse::<u32>().ok()?),
        None => (s, 32),
    };
    if bits > 32 {
        return None;
    }
    let ip: std::net::Ipv4Addr = ip.trim().parse().ok()?;
    let ip = u32::from(ip);
    let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
    Some((ip & mask, (ip & mask) | !mask))
}

/// Канал перехвата. `recv` блокируется; `shutdown` (из другого потока) прерывает его.
pub trait PacketIo: Send + Sync {
    /// Получить пакет; в `addr` — непрозрачные данные драйвера для повторной отправки.
    fn recv(&self, buf: &mut [u8], addr: &mut [u8]) -> Result<usize, PlatformError>;
    fn send(&self, packet: &[u8], addr: &[u8]) -> Result<(), PlatformError>;
    fn shutdown(&self);
    fn addr_len(&self) -> usize;
    /// Пометить адрес: «контрольные суммы уже посчитаны, не трогать».
    fn mark_checksums_valid(&self, _addr: &mut [u8]) {}
}

pub fn open_default(filter: &Filter) -> Result<std::sync::Arc<dyn PacketIo>, PlatformError> {
    #[cfg(windows)]
    {
        let mut f = filter.clone();
        f.exclude_ifs.extend(windivert::tunnel_interfaces().into_iter().map(|(i, _)| i));
        Ok(std::sync::Arc::new(windivert::WinDivert::open(&f)?))
    }
    #[cfg(not(windows))]
    {
        let _ = filter;
        Err(PlatformError::Io(std::io::Error::new(std::io::ErrorKind::Unsupported, "пока поддерживается только Windows")))
    }
}

/// Программы, которые тоже перехватывают трафик и будут конфликтовать.
pub const KNOWN_CONFLICTS: &[(&str, &str)] = &[
    ("winws.exe", "zapret"),
    ("goodbyedpi.exe", "GoodbyeDPI"),
    ("ciadpi.exe", "ByeDPI"),
    ("spoofdpi.exe", "SpoofDPI"),
    // VPN/прокси-клиенты в режиме TUN: весь трафик идёт в туннель, и обход его ломает.
    ("happd.exe", "Happ (VPN)"),
    ("hiddify.exe", "Hiddify (VPN)"),
    ("sing-box.exe", "sing-box (VPN)"),
    ("v2rayn.exe", "v2rayN (VPN)"),
    ("nekobox.exe", "NekoBox (VPN)"),
    ("nekoray.exe", "NekoRay (VPN)"),
    ("clash-verge.exe", "Clash Verge (VPN)"),
    ("mihomo.exe", "Mihomo/Clash (VPN)"),
    ("amneziavpn.exe", "AmneziaVPN"),
];
