//! Разбор и сборка сырых IPv4/IPv6 + TCP пакетов.
//! Не зависит от ОС: одинаково нужен и для WinDivert, и для NFQUEUE.

use crate::strategy::{Fooling, Segment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpInfo {
    pub v6: bool,
    pub ip_hdr_len: usize,
    pub tcp_hdr_len: usize,
    pub seq: u32,
    pub src_port: u16,
    pub dst_port: u16,
}

impl TcpInfo {
    pub fn payload_off(&self) -> usize {
        self.ip_hdr_len + self.tcp_hdr_len
    }
}

fn be16(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// Разобрать IP+TCP. Пакеты с IPv6 extension headers и фрагменты пропускаем.
pub fn parse_tcp(p: &[u8]) -> Option<TcpInfo> {
    let (v6, ip_hdr_len) = match p.first()? >> 4 {
        4 => {
            let ihl = ((p[0] & 0x0f) as usize) * 4;
            if p.len() < ihl + 20 || ihl < 20 || p[9] != 6 {
                return None;
            }
            let frag = be16(p, 6);
            if frag & 0x3fff != 0 {
                return None; // фрагмент
            }
            (false, ihl)
        }
        6 => {
            if p.len() < 60 || p[6] != 6 {
                return None;
            }
            (true, 40)
        }
        _ => return None,
    };
    let t = ip_hdr_len;
    let tcp_hdr_len = ((p[t + 12] >> 4) as usize) * 4;
    if tcp_hdr_len < 20 || p.len() < t + tcp_hdr_len {
        return None;
    }
    Some(TcpInfo {
        v6,
        ip_hdr_len,
        tcp_hdr_len,
        seq: u32::from_be_bytes([p[t + 4], p[t + 5], p[t + 6], p[t + 7]]),
        src_port: be16(p, t),
        dst_port: be16(p, t + 2),
    })
}

fn sum16(data: &[u8], mut acc: u32) -> u32 {
    let mut chunks = data.chunks_exact(2);
    for c in &mut chunks {
        acc += u16::from_be_bytes([c[0], c[1]]) as u32;
    }
    if let [last] = chunks.remainder() {
        acc += (*last as u32) << 8;
    }
    acc
}

fn fold(mut acc: u32) -> u16 {
    while acc >> 16 != 0 {
        acc = (acc & 0xffff) + (acc >> 16);
    }
    !(acc as u16)
}

pub fn fix_checksums(p: &mut [u8], info: &TcpInfo) {
    let t = info.ip_hdr_len;
    let tcp_len = p.len() - t;
    if !info.v6 {
        p[10] = 0;
        p[11] = 0;
        let c = fold(sum16(&p[..t], 0));
        p[10..12].copy_from_slice(&c.to_be_bytes());
    }
    p[t + 16] = 0;
    p[t + 17] = 0;
    let mut acc = if info.v6 {
        let a = sum16(&p[8..40], 0);
        a + (tcp_len as u32 >> 16) + (tcp_len as u32 & 0xffff) + 6
    } else {
        sum16(&p[12..20], 0) + 6 + tcp_len as u32
    };
    acc = sum16(&p[t..], acc);
    let c = fold(acc);
    p[t + 16..t + 18].copy_from_slice(&c.to_be_bytes());
}

/// Собрать новый пакет на основе исходного: те же заголовки, другой payload и параметры.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpInfo {
    pub v6: bool,
    pub ip_hdr_len: usize,
    pub src_port: u16,
    pub dst_port: u16,
}

impl UdpInfo {
    pub fn payload_off(&self) -> usize {
        self.ip_hdr_len + 8
    }
}

pub fn parse_udp(p: &[u8]) -> Option<UdpInfo> {
    let (v6, ihl) = match p.first()? >> 4 {
        4 => {
            let ihl = ((p[0] & 0x0f) as usize) * 4;
            if p.len() < ihl + 8 || p[9] != 17 || be16(p, 6) & 0x3fff != 0 {
                return None;
            }
            (false, ihl)
        }
        6 => {
            if p.len() < 48 || p[6] != 17 {
                return None;
            }
            (true, 40)
        }
        _ => return None,
    };
    Some(UdpInfo { v6, ip_hdr_len: ihl, src_port: be16(p, ihl), dst_port: be16(p, ihl + 2) })
}

/// Адрес назначения IPv4 или IPv6 как одно число (см. `platform::ip_key`).
pub fn dst_ip(p: &[u8]) -> Option<u128> {
    use std::net::IpAddr;
    let ip: IpAddr = match p.first()? >> 4 {
        4 if p.len() >= 20 => <[u8; 4]>::try_from(&p[16..20]).ok()?.into(),
        6 if p.len() >= 40 => <[u8; 16]>::try_from(&p[24..40]).ok()?.into(),
        _ => return None,
    };
    Some(crate::platform::ip_key(ip))
}

/// Входящий SYN-ACK: (адрес сервера, TTL / hop limit) — для autottl.
pub fn syn_ack_ttl(p: &[u8]) -> Option<(u128, u8)> {
    use std::net::IpAddr;
    let info = parse_tcp(p)?;
    if p[info.ip_hdr_len + 13] & 0x12 != 0x12 {
        return None;
    }
    let (ip, ttl): (IpAddr, u8) = if info.v6 {
        (<[u8; 16]>::try_from(&p[8..24]).ok()?.into(), p[7])
    } else {
        (<[u8; 4]>::try_from(&p[12..16]).ok()?.into(), p[8])
    };
    Some((crate::platform::ip_key(ip), ttl))
}

pub fn build_udp(orig: &[u8], info: &UdpInfo, payload: &[u8]) -> Vec<u8> {
    let t = info.ip_hdr_len;
    let mut p = Vec::with_capacity(t + 8 + payload.len());
    p.extend_from_slice(&orig[..t + 8]);
    p.extend_from_slice(payload);
    let total = p.len();
    let udp_len = (total - t) as u16;
    p[t + 4..t + 6].copy_from_slice(&udp_len.to_be_bytes());
    if info.v6 {
        p[4..6].copy_from_slice(&udp_len.to_be_bytes());
    } else {
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        p[10] = 0;
        p[11] = 0;
        let c = fold(sum16(&p[..t], 0));
        p[10..12].copy_from_slice(&c.to_be_bytes());
    }
    p[t + 6] = 0;
    p[t + 7] = 0;
    let acc = if info.v6 {
        sum16(&p[8..40], 0) + udp_len as u32 + 17
    } else {
        sum16(&p[12..20], 0) + 17 + udp_len as u32
    };
    let mut c = fold(sum16(&p[t..], acc));
    if c == 0 {
        c = 0xffff;
    }
    p[t + 6..t + 8].copy_from_slice(&c.to_be_bytes());
    p
}

/// Похоже ли на пакет обнаружения IP Discord (голос) или STUN.
pub fn is_discord_or_stun(payload: &[u8]) -> bool {
    let discord = payload.len() == 74 && payload[..4] == [0x00, 0x01, 0x00, 0x46];
    let stun = payload.len() >= 20 && payload[4..8] == [0x21, 0x12, 0xa4, 0x42] && payload[0] & 0xc0 == 0;
    discord || stun
}

pub fn is_quic(p: &[u8]) -> bool {
    match p.first().map(|b| b >> 4) {
        Some(4) => {
            let ihl = ((p[0] & 0x0f) as usize) * 4;
            p.len() >= ihl + 8 && p[9] == 17 && be16(p, ihl + 2) == 443
        }
        Some(6) => p.len() >= 48 && p[6] == 17 && be16(p, 42) == 443,
        _ => false,
    }
}

pub fn build_segment(orig: &[u8], info: &TcpInfo, seg: &Segment) -> Vec<u8> {
    build_segment_ex(orig, info, seg, false)
}

/// `ip_id_zero` — обнулить IPv4 ID (как --ip-id=zero в zapret).
pub fn build_segment_ex(orig: &[u8], info: &TcpInfo, seg: &Segment, ip_id_zero: bool) -> Vec<u8> {
    let md5 = seg.fooling.contains(&Fooling::Md5Sig) && info.tcp_hdr_len + 20 <= 60;
    let hdr_end = info.payload_off();
    let mut p = Vec::with_capacity(hdr_end + 20 + seg.data.len());
    p.extend_from_slice(&orig[..hdr_end]);
    let t = info.ip_hdr_len;
    let mut tcp_hdr_len = info.tcp_hdr_len;
    if md5 {
        // TCP option 19 (MD5 signature), 18 байт + 2 NOP для выравнивания.
        let mut opt = vec![1u8, 1, 19, 18];
        opt.extend_from_slice(&[0u8; 16]);
        p.extend_from_slice(&opt);
        tcp_hdr_len += 20;
        p[t + 12] = ((tcp_hdr_len / 4) as u8) << 4 | (p[t + 12] & 0x0f);
    }
    p.extend_from_slice(&seg.data);

    // ts: уменьшаем TSval в опции timestamp — сервер отбросит пакет (PAWS).
    // Если timestamps в соединении нет, фейк мог бы дойти до сервера — тогда страхуемся badseq.
    let mut ts_done = false;
    if seg.fooling.contains(&Fooling::Ts) {
        let mut o = t + 20;
        let end = t + info.tcp_hdr_len;
        while o < end {
            match p[o] {
                0 => break,
                1 => o += 1,
                8 if o + 10 <= end && p[o + 1] == 10 => {
                    let v = u32::from_be_bytes([p[o + 2], p[o + 3], p[o + 4], p[o + 5]]).wrapping_sub(600_000);
                    p[o + 2..o + 6].copy_from_slice(&v.to_be_bytes());
                    ts_done = true;
                    break;
                }
                _ => {
                    let l = p.get(o + 1).copied().unwrap_or(0) as usize;
                    if l < 2 {
                        break;
                    }
                    o += l;
                }
            }
        }
    }

    let mut seq = info.seq.wrapping_add(seg.seq_offset);
    if seg.fooling.contains(&Fooling::BadSeq) || (seg.fooling.contains(&Fooling::Ts) && !ts_done) {
        seq = seq.wrapping_sub(10_000);
    }
    p[t + 4..t + 8].copy_from_slice(&seq.to_be_bytes());
    if seg.fooling.contains(&Fooling::DataNoAck) {
        p[t + 13] &= !0x10;
    }

    let total = p.len();
    if info.v6 {
        p[4..6].copy_from_slice(&((total - 40) as u16).to_be_bytes());
        if let Some(ttl) = seg.ttl {
            p[7] = ttl;
        }
    } else {
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        if let Some(ttl) = seg.ttl {
            p[8] = ttl;
        }
        if ip_id_zero {
            p[4] = 0;
            p[5] = 0;
        }
    }
    let new_info = TcpInfo { tcp_hdr_len, ..*info };
    fix_checksums(&mut p, &new_info);
    if seg.fooling.contains(&Fooling::BadSum) {
        p[t + 16] ^= 0x5a;
    }
    p
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::strategy::SegKind;

    /// Собрать тестовый IPv4/TCP пакет с payload и корректными суммами.
    pub fn make_v4(payload: &[u8], seq: u32) -> Vec<u8> {
        let mut p = vec![
            0x45, 0, 0, 0, 0x12, 0x34, 0x40, 0, 64, 6, 0, 0, 192, 168, 1, 10, 142, 250, 74, 46,
        ];
        let tcp = [
            0xc3, 0x50, 0x01, 0xbb, 0, 0, 0, 0, 0, 0, 0, 1, 0x50, 0x18, 0xff, 0xff, 0, 0, 0, 0,
        ];
        p.extend_from_slice(&tcp);
        p[24..28].copy_from_slice(&seq.to_be_bytes());
        p.extend_from_slice(payload);
        let len = p.len() as u16;
        p[2..4].copy_from_slice(&len.to_be_bytes());
        let info = parse_tcp(&p).unwrap();
        fix_checksums(&mut p, &info);
        p
    }

    /// IPv6/TCP пакет: 2a00:1:2::10 → 2a00:1450:4010:c0e::5d (YouTube), порт 50000 → 443.
    pub fn make_v6(payload: &[u8], seq: u32) -> Vec<u8> {
        let mut p = vec![0x60, 0, 0, 0, 0, 0, 6, 64];
        p.extend_from_slice(&[0x2a, 0x00, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10]);
        p.extend_from_slice(&[0x2a, 0x00, 0x14, 0x50, 0x40, 0x10, 0x0c, 0x0e, 0, 0, 0, 0, 0, 0, 0, 0x5d]);
        p.extend_from_slice(&[0xc3, 0x50, 0x01, 0xbb, 0, 0, 0, 0, 0, 0, 0, 1, 0x50, 0x18, 0xff, 0xff, 0, 0, 0, 0]);
        p[44..48].copy_from_slice(&seq.to_be_bytes());
        p.extend_from_slice(payload);
        let len = (p.len() - 40) as u16;
        p[4..6].copy_from_slice(&len.to_be_bytes());
        let info = parse_tcp(&p).unwrap();
        fix_checksums(&mut p, &info);
        p
    }

    /// Сумма TCP по RFC 8200 §8.1, посчитанная заново, без fix_checksums.
    pub fn v6_tcp_checksum_valid(p: &[u8]) -> bool {
        let tcp = &p[40..];
        let mut words: Vec<u32> = p[8..40].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]]) as u32).collect();
        words.extend([(tcp.len() as u32) >> 16, tcp.len() as u32 & 0xffff, 6]);
        words.extend(tcp.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u32));
        let mut s: u32 = words.iter().sum();
        while s >> 16 != 0 {
            s = (s & 0xffff) + (s >> 16);
        }
        s == 0xffff
    }

    #[test]
    fn v6_packet_parses_and_segments_have_valid_checksums() {
        let p = make_v6(b"hello", 1000);
        let i = parse_tcp(&p).unwrap();
        assert!(i.v6);
        assert_eq!((i.dst_port, i.seq, i.payload_off()), (443, 1000, 60));
        assert!(v6_tcp_checksum_valid(&p));
        assert_eq!(dst_ip(&p), Some(crate::platform::ip_key("2a00:1450:4010:c0e::5d".parse().unwrap())));
        let seg = Segment { kind: SegKind::Fake, seq_offset: 3, data: b"fake!".to_vec(), ttl: Some(4), fooling: vec![Fooling::Ttl], label: String::new() };
        let out = build_segment(&p, &i, &seg);
        assert!(v6_tcp_checksum_valid(&out));
        assert_eq!(out[7], 4, "hop limit = TTL фейка");
        assert_eq!(u16::from_be_bytes([out[4], out[5]]) as usize, out.len() - 40);
        assert_eq!(parse_tcp(&out).unwrap().seq, 1003);
    }

    fn checksum_ok(p: &[u8]) -> bool {
        let info = parse_tcp(p).unwrap();
        let mut q = p.to_vec();
        fix_checksums(&mut q, &info);
        q == p
    }

    #[test]
    fn parses_v4() {
        let p = make_v4(b"hello", 1000);
        let i = parse_tcp(&p).unwrap();
        assert_eq!((i.dst_port, i.seq, i.payload_off()), (443, 1000, 40));
        assert_eq!(&p[i.payload_off()..], b"hello");
    }

    #[test]
    fn ipv4_checksum_known_value() {
        // Классический пример из RFC 1071-подобных разборов.
        let mut h = [0x45u8, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0, 0, 0xc0, 0xa8, 0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7];
        let c = fold(sum16(&h, 0));
        h[10..12].copy_from_slice(&c.to_be_bytes());
        assert_eq!(c, 0xb861);
    }

    #[test]
    fn builds_segment_with_offset_ttl_and_md5() {
        let orig = make_v4(b"0123456789", 5000);
        let info = parse_tcp(&orig).unwrap();
        let seg = Segment {
            kind: SegKind::Fake,
            seq_offset: 4,
            data: b"456".to_vec(),
            ttl: Some(3),
            fooling: vec![Fooling::Ttl, Fooling::Md5Sig],
            label: String::new(),
        };
        let p = build_segment(&orig, &info, &seg);
        let ni = parse_tcp(&p).unwrap();
        assert_eq!(ni.seq, 5004);
        assert_eq!(ni.tcp_hdr_len, 40);
        assert_eq!(p[8], 3);
        assert_eq!(u16::from_be_bytes([p[2], p[3]]) as usize, p.len());
        assert_eq!(&p[ni.payload_off()..], b"456");
        assert!(checksum_ok(&p));
    }

    #[test]
    fn ts_fooling_lowers_timestamp_or_falls_back_to_badseq() {
        // Пакет с опцией timestamp (NOP NOP TS) — заголовок TCP 32 байта.
        let mut p = make_v4(b"", 100);
        p.truncate(40);
        p.extend_from_slice(&[1, 1, 8, 10, 0, 0x10, 0, 0, 0, 0, 0, 1]);
        p[32] = 0x80; // data offset = 8 слов
        p.extend_from_slice(b"hi");
        let len = p.len() as u16;
        p[2..4].copy_from_slice(&len.to_be_bytes());
        let info = parse_tcp(&p).unwrap();
        let seg = Segment { kind: SegKind::Fake, seq_offset: 0, data: b"x".to_vec(), ttl: None,
            fooling: vec![Fooling::Ts], label: String::new() };
        let out = build_segment(&p, &info, &seg);
        let tsval = u32::from_be_bytes([out[44], out[45], out[46], out[47]]);
        assert_eq!(tsval, 0x0010_0000u32.wrapping_sub(600_000));
        assert_eq!(parse_tcp(&out).unwrap().seq, 100);
        assert!(checksum_ok(&out));
        // Без timestamp — страховка badseq.
        let plain = make_v4(b"hi", 100);
        let out = build_segment(&plain, &parse_tcp(&plain).unwrap(), &seg);
        assert_eq!(parse_tcp(&out).unwrap().seq, 100u32.wrapping_sub(10_000));
    }

    pub fn make_udp4(payload: &[u8], dst: [u8; 4], dport: u16) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 1, 0, 0, 64, 17, 0, 0, 10, 0, 0, 1, dst[0], dst[1], dst[2], dst[3]];
        p.extend_from_slice(&[0xc3, 0x50]);
        p.extend_from_slice(&dport.to_be_bytes());
        p.extend_from_slice(&[0, 0, 0, 0]);
        let info = UdpInfo { v6: false, ip_hdr_len: 20, src_port: 50000, dst_port: dport };
        build_udp(&p, &info, payload)
    }

    pub fn make_udp6(payload: &[u8], dst: &str, dport: u16) -> Vec<u8> {
        let dst: std::net::Ipv6Addr = dst.parse().unwrap();
        let mut p = vec![0x60, 0, 0, 0, 0, 0, 17, 64];
        p.extend_from_slice(&[0x2a, 0x00, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10]);
        p.extend_from_slice(&dst.octets());
        p.extend_from_slice(&[0xc3, 0x50]);
        p.extend_from_slice(&dport.to_be_bytes());
        p.extend_from_slice(&[0, 0, 0, 0]);
        let info = UdpInfo { v6: true, ip_hdr_len: 40, src_port: 50000, dst_port: dport };
        build_udp(&p, &info, payload)
    }

    #[test]
    fn udp_build_and_checksum() {
        let p = make_udp4(b"hello", [8, 8, 8, 8], 50010);
        let i = parse_udp(&p).unwrap();
        assert_eq!((i.dst_port, &p[i.payload_off()..]), (50010, &b"hello"[..]));
        assert_eq!(u16::from_be_bytes([p[24], p[25]]), 13);
        // Проверка суммы: пересчёт по пакету с полем суммы даёт 0.
        let acc = sum16(&p[12..20], 0) + 17 + 13;
        assert_eq!(fold(sum16(&p[20..], acc)), 0);
        assert_eq!(dst_ip(&p), Some(crate::platform::ip_key("8.8.8.8".parse().unwrap())));
    }

    #[test]
    fn detects_discord_ip_discovery_and_stun() {
        let mut d = vec![0u8; 74];
        d[..4].copy_from_slice(&[0, 1, 0, 0x46]);
        assert!(is_discord_or_stun(&d));
        let mut s = vec![0u8; 20];
        s[1] = 1;
        s[4..8].copy_from_slice(&[0x21, 0x12, 0xa4, 0x42]);
        assert!(is_discord_or_stun(&s));
        assert!(!is_discord_or_stun(&[1u8; 50]));
    }

    #[test]
    fn badsum_breaks_checksum() {
        let orig = make_v4(b"abc", 1);
        let info = parse_tcp(&orig).unwrap();
        let seg = Segment { kind: SegKind::Fake, seq_offset: 0, data: b"x".to_vec(), ttl: None,
            fooling: vec![Fooling::BadSum, Fooling::BadSeq], label: String::new() };
        let p = build_segment(&orig, &info, &seg);
        assert!(!checksum_ok(&p));
        assert_eq!(parse_tcp(&p).unwrap().seq, 1u32.wrapping_sub(10_000));
    }

    #[test]
    fn datanoack_clears_ack_flag() {
        let orig = make_v4(b"abc", 1);
        let info = parse_tcp(&orig).unwrap();
        let seg = Segment { kind: SegKind::Fake, seq_offset: 0, data: b"x".to_vec(), ttl: None,
            fooling: vec![Fooling::DataNoAck], label: String::new() };
        let p = build_segment(&orig, &info, &seg);
        assert_eq!(orig[33] & 0x10, 0x10);
        assert_eq!(p[33] & 0x10, 0);
        assert!(checksum_ok(&p));
    }

    #[test]
    fn syn_ack_ttl_reads_server_and_ttl() {
        let mut p = make_v4(b"", 0);
        p[33] = 0x12; // SYN+ACK
        p[8] = 52;
        assert_eq!(syn_ack_ttl(&p), Some((crate::platform::ip_key("192.168.1.10".parse().unwrap()), 52)));
        p[33] = 0x18; // PSH+ACK — не SYN-ACK
        assert_eq!(syn_ack_ttl(&p), None);
    }
}
