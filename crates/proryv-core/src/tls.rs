//! Разбор TLS ClientHello: ищем SNI (имя сайта), который и смотрит DPI.

/// Где в полезной нагрузке TCP лежит имя сайта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SniInfo {
    pub host: String,
    /// Смещение первого байта имени от начала payload.
    pub offset: usize,
    pub len: usize,
    /// Длина первой TLS-записи (заголовок 5 байт + тело).
    pub record_len: usize,
}

fn u16_at(b: &[u8], i: usize) -> Option<usize> {
    Some(((*b.get(i)? as usize) << 8) | *b.get(i + 1)? as usize)
}

/// Похоже ли это на начало TLS ClientHello.
pub fn is_client_hello(p: &[u8]) -> bool {
    p.len() >= 6 && p[0] == 0x16 && p[1] == 0x03 && p[5] == 0x01
}

/// Найти SNI в TLS ClientHello. Возвращает `None`, если это не ClientHello
/// или пакет обрезан раньше расширения server_name.
pub fn parse_sni(p: &[u8]) -> Option<SniInfo> {
    if !is_client_hello(p) {
        return None;
    }
    let record_len = 5 + u16_at(p, 3)?;
    // 5 (record) + 4 (handshake hdr) + 2 (version) + 32 (random)
    let mut i = 5 + 4 + 2 + 32;
    i += 1 + *p.get(i)? as usize; // session id
    i += 2 + u16_at(p, i)?; // cipher suites
    i += 1 + *p.get(i)? as usize; // compression
    let ext_end = (i + 2 + u16_at(p, i)?).min(p.len());
    i += 2;
    while i + 4 <= ext_end {
        let ty = u16_at(p, i)?;
        let len = u16_at(p, i + 2)?;
        let body = i + 4;
        if ty == 0x0000 {
            // server_name: list_len(2) type(1) name_len(2) name
            let name_len = u16_at(p, body + 3)?;
            let off = body + 5;
            let host = std::str::from_utf8(p.get(off..off + name_len)?).ok()?.to_string();
            return Some(SniInfo { host, offset: off, len: name_len, record_len });
        }
        i = body + len;
    }
    None
}

/// Собрать минимальный ClientHello с указанным SNI (для тестов, предпросмотра и фейков).
pub fn build_client_hello(sni: &str) -> Vec<u8> {
    let name = sni.as_bytes();
    let mut ext = Vec::new();
    // server_name
    ext.extend_from_slice(&[0x00, 0x00]);
    ext.extend_from_slice(&((name.len() + 5) as u16).to_be_bytes());
    ext.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
    ext.push(0);
    ext.extend_from_slice(&(name.len() as u16).to_be_bytes());
    ext.extend_from_slice(name);
    // supported_versions: TLS 1.3, 1.2
    ext.extend_from_slice(&[0x00, 0x2b, 0x00, 0x05, 0x04, 0x03, 0x04, 0x03, 0x03]);

    let mut hs = Vec::new();
    hs.extend_from_slice(&[0x03, 0x03]); // legacy version
    hs.extend_from_slice(&[0x5a; 32]); // random
    hs.push(32);
    hs.extend_from_slice(&[0xa5; 32]); // session id
    hs.extend_from_slice(&[0x00, 0x04, 0x13, 0x01, 0x13, 0x02]); // ciphers
    hs.extend_from_slice(&[0x01, 0x00]); // compression
    hs.extend_from_slice(&(ext.len() as u16).to_be_bytes());
    hs.extend_from_slice(&ext);

    let mut out = vec![0x16, 0x03, 0x01];
    out.extend_from_slice(&((hs.len() + 4) as u16).to_be_bytes());
    out.push(0x01);
    let l = hs.len() as u32;
    out.extend_from_slice(&l.to_be_bytes()[1..]);
    out.extend_from_slice(&hs);
    out
}

/// Достать session id из ClientHello (для фейка «dupsid» — как у настоящего).
pub fn session_id(p: &[u8]) -> Option<&[u8]> {
    if !is_client_hello(p) {
        return None;
    }
    let i = 5 + 4 + 2 + 32;
    let len = *p.get(i)? as usize;
    p.get(i + 1..i + 1 + len)
}

/// Реалистичный ClientHello, похожий на Chrome: GREASE, ALPN, key_share, padding.
/// DPI разбирает такой фейк как настоящий запрос к `sni`.
/// `rnd` — случайные байты для поля random и ключа; `sid` — session id (32 байта).
pub fn build_realistic_hello(sni: &str, rnd: &[u8; 64], sid: Option<&[u8]>, min_len: usize) -> Vec<u8> {
    fn ext(out: &mut Vec<u8>, ty: u16, body: &[u8]) {
        out.extend_from_slice(&ty.to_be_bytes());
        out.extend_from_slice(&(body.len() as u16).to_be_bytes());
        out.extend_from_slice(body);
    }
    let name = sni.as_bytes();
    let mut e = Vec::new();
    ext(&mut e, 0x0a0a, &[]); // GREASE
    let mut sn = Vec::new();
    sn.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
    sn.push(0);
    sn.extend_from_slice(&(name.len() as u16).to_be_bytes());
    sn.extend_from_slice(name);
    ext(&mut e, 0x0000, &sn); // server_name
    ext(&mut e, 0x0017, &[]); // extended_master_secret
    ext(&mut e, 0xff01, &[0]); // renegotiation_info
    ext(&mut e, 0x000a, &[0, 8, 0x1a, 0x1a, 0, 0x1d, 0, 0x17, 0, 0x18]); // supported_groups
    ext(&mut e, 0x000b, &[1, 0]); // ec_point_formats
    ext(&mut e, 0x0023, &[]); // session_ticket
    ext(&mut e, 0x0010, &[0, 12, 2, b'h', b'2', 8, b'h', b't', b't', b'p', b'/', b'1', b'.', b'1']); // ALPN
    ext(&mut e, 0x0005, &[1, 0, 0, 0, 0]); // status_request
    ext(&mut e, 0x000d, &[0, 16, 4, 3, 8, 4, 4, 1, 5, 3, 8, 5, 5, 1, 8, 6, 6, 1]); // signature_algorithms
    ext(&mut e, 0x0012, &[]); // signed_certificate_timestamp
    let mut ks = vec![0, 41, 0x1a, 0x1a, 0, 1, 0, 0, 0x1d, 0, 32];
    ks.extend_from_slice(&rnd[32..64]);
    ext(&mut e, 0x0033, &ks); // key_share (x25519)
    ext(&mut e, 0x002d, &[1, 1]); // psk_key_exchange_modes
    ext(&mut e, 0x002b, &[6, 0x2a, 0x2a, 3, 4, 3, 3]); // supported_versions
    ext(&mut e, 0x001b, &[2, 0, 2]); // compress_certificate (brotli)
    ext(&mut e, 0x4469, &[0, 3, 2, b'h', b'2']); // application_settings
    ext(&mut e, 0x1a1a, &[0]); // GREASE

    let mut hs = Vec::new();
    hs.extend_from_slice(&[0x03, 0x03]);
    hs.extend_from_slice(&rnd[..32]);
    let sid = sid.filter(|s| s.len() <= 32).map(|s| s.to_vec()).unwrap_or_else(|| rnd[16..48].to_vec());
    hs.push(sid.len() as u8);
    hs.extend_from_slice(&sid);
    let ciphers: [u16; 16] = [0x2a2a, 0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8, 0xc013, 0xc014, 0x009c, 0x009d, 0x002f, 0x0035];
    hs.extend_from_slice(&((ciphers.len() * 2) as u16).to_be_bytes());
    for c in ciphers {
        hs.extend_from_slice(&c.to_be_bytes());
    }
    hs.extend_from_slice(&[1, 0]);

    // padding (ext 21) — до нужного размера, как делает Chrome.
    let base = 5 + 4 + hs.len() + 2 + e.len();
    let target = min_len.max(517);
    if base + 4 < target {
        ext(&mut e, 0x0015, &vec![0u8; target - base - 4]);
    }
    hs.extend_from_slice(&(e.len() as u16).to_be_bytes());
    hs.extend_from_slice(&e);

    let mut out = vec![0x16, 0x03, 0x01];
    out.extend_from_slice(&((hs.len() + 4) as u16).to_be_bytes());
    out.push(0x01);
    out.extend_from_slice(&(hs.len() as u32).to_be_bytes()[1..]);
    out.extend_from_slice(&hs);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn realistic_hello_parses() {
        let p = build_realistic_hello("www.google.com", &[7; 64], Some(&[1; 32]), 700);
        assert_eq!(parse_sni(&p).unwrap().host, "www.google.com");
        assert_eq!(p.len(), 700);
        assert_eq!(session_id(&p).unwrap(), &[1; 32]);
    }

    use super::*;

    #[test]
    fn finds_sni() {
        let p = build_client_hello("www.youtube.com");
        let s = parse_sni(&p).unwrap();
        assert_eq!(s.host, "www.youtube.com");
        assert_eq!(&p[s.offset..s.offset + s.len], b"www.youtube.com");
        assert_eq!(s.record_len, p.len());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_sni(b"GET / HTTP/1.1\r\n").is_none());
        assert!(parse_sni(&[0x16, 0x03, 0x01]).is_none());
        let p = build_client_hello("discord.com");
        assert!(parse_sni(&p[..60]).is_none()); // обрезан
    }
}
