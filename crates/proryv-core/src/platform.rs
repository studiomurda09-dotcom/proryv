//! Интерфейс перехвата пакетов. Ядро ничего не знает об ОС:
//! на Windows это WinDivert, на Linux/роутерах будет NFQUEUE.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
    #[error("Драйвер перехвата уже занят другой программой (обычно служба zapret или GoodbyeDPI). Удалите её службу (в zapret: service.bat → Remove Services) и перезагрузите ПК.")]
    DriverInUse,
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
    /// Game Filter: TCP/UDP на порты 1024–65535 к этим диапазонам адресов.
    pub game_tcp: bool,
    pub game_udp: bool,
    pub game_nets: Vec<Net>,
    /// Перехватывать входящие SYN-ACK (для autottl): TTL ответа = число хопов.
    pub syn_ack: bool,
}

/// Диапазон адресов. IPv4 хранится как v4-mapped (`::ffff:a.b.c.d`),
/// поэтому IPv4 и IPv6 сравниваются одним числом.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Net {
    pub lo: u128,
    pub hi: u128,
}

const V4_MAPPED: u128 = 0xffff << 32;

impl Net {
    pub fn contains(&self, ip: u128) -> bool {
        ip >= self.lo && ip <= self.hi
    }

    pub fn is_v4(&self) -> bool {
        self.lo >> 32 == 0xffff
    }
}

pub fn ip_key(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(a) => V4_MAPPED | u32::from(a) as u128,
        IpAddr::V6(a) => u128::from(a),
    }
}

/// Сети Google/YouTube. QUIC блокируем только к ним: UDP 443 используют и VPN
/// (Hysteria2, TUIC…), и игры — их трогать нельзя.
pub const GOOGLE_NETS: &[&str] = &[
    "142.250.0.0/15", "172.217.0.0/16", "172.253.0.0/16", "173.194.0.0/16", "74.125.0.0/16",
    "216.58.192.0/19", "209.85.128.0/17", "64.233.160.0/19", "108.177.0.0/17", "192.178.0.0/15",
    "66.102.0.0/20", "66.249.64.0/19", "72.14.192.0/18", "216.239.32.0/19",
    "2001:4860::/32", "2404:6800::/32", "2607:f8b0::/32", "2800:3f0::/32", "2a00:1450::/32", "2c0f:fb50::/32",
];

pub fn google_nets() -> Vec<Net> {
    GOOGLE_NETS.iter().filter_map(|c| parse_cidr(c)).collect()
}

/// Условие WinDivert «адрес назначения в одной из сетей».
pub fn nets_condition(nets: &[Net]) -> String {
    nets.iter()
        .map(|n| {
            if n.is_v4() {
                let ip = |v: u128| Ipv4Addr::from(v as u32);
                format!("(ip.DstAddr >= {} and ip.DstAddr <= {})", ip(n.lo), ip(n.hi))
            } else {
                format!("(ipv6.DstAddr >= {} and ipv6.DstAddr <= {})", Ipv6Addr::from(n.lo), Ipv6Addr::from(n.hi))
            }
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// "128.116.0.0/17", "1.2.3.4", "2a00:1450::/32" → диапазон.
pub fn parse_cidr(s: &str) -> Option<Net> {
    let s = s.trim();
    let (ip, bits) = match s.split_once('/') {
        Some((ip, b)) => (ip.trim(), Some(b.trim().parse::<u32>().ok()?)),
        None => (s, None),
    };
    let (key, bits) = match ip.parse::<IpAddr>().ok()? {
        a @ IpAddr::V4(_) => (ip_key(a), 96 + bits.unwrap_or(32)),
        a @ IpAddr::V6(_) => (ip_key(a), bits.unwrap_or(128)),
    };
    if bits > 128 {
        return None;
    }
    let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
    Some(Net { lo: key & mask, hi: (key & mask) | !mask })
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

/// Службы других обходов: держат свой WinDivert даже без открытого окна.
pub const CONFLICT_SERVICES: &[(&str, &str)] = &[
    ("zapret", "zapret (служба)"),
    ("winws1", "zapret (служба)"),
    ("goodbyedpi", "GoodbyeDPI (служба)"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_v4_and_v6() {
        let roblox = parse_cidr("128.116.0.0/17").unwrap();
        assert!(roblox.is_v4());
        assert!(roblox.contains(ip_key("128.116.5.9".parse().unwrap())));
        assert!(!roblox.contains(ip_key("128.117.0.1".parse().unwrap())));
        let g6 = parse_cidr("2a00:1450::/32").unwrap();
        assert!(!g6.is_v4());
        assert!(g6.contains(ip_key("2a00:1450:4010:c0e::5d".parse().unwrap())));
        assert!(!g6.contains(ip_key("2a00:1451::1".parse().unwrap())));
        assert!(!g6.contains(ip_key("142.250.1.1".parse().unwrap())), "IPv4 не попадает в IPv6-сеть");
        assert_eq!(parse_cidr("1.2.3.4"), Some(Net { lo: ip_key("1.2.3.4".parse().unwrap()), hi: ip_key("1.2.3.4".parse().unwrap()) }));
        assert!(parse_cidr("1.2.3.4/33").is_none() && parse_cidr("::/129").is_none() && parse_cidr("мусор").is_none());
    }

    #[test]
    fn condition_uses_right_field_per_family() {
        let c = nets_condition(&[parse_cidr("128.116.0.0/17").unwrap(), parse_cidr("2a00:1450::/32").unwrap()]);
        assert!(c.contains("(ip.DstAddr >= 128.116.0.0 and ip.DstAddr <= 128.116.127.255)"), "{c}");
        assert!(c.contains("(ipv6.DstAddr >= 2a00:1450:: and ipv6.DstAddr <= 2a00:1450:ffff:ffff:ffff:ffff:ffff:ffff)"), "{c}");
    }
}
