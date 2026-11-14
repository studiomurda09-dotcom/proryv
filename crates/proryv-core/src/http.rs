//! Поиск заголовка Host в обычном HTTP-запросе.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub host: String,
    /// Смещение значения Host от начала payload.
    pub offset: usize,
    pub len: usize,
    /// Смещение начала слова "Host".
    pub header_offset: usize,
}

const METHODS: [&[u8]; 7] = [b"GET ", b"POST ", b"HEAD ", b"PUT ", b"OPTIONS ", b"DELETE ", b"PATCH "];

pub fn is_http_request(p: &[u8]) -> bool {
    METHODS.iter().any(|m| p.starts_with(m))
}

pub fn parse_host(p: &[u8]) -> Option<HostInfo> {
    if !is_http_request(p) {
        return None;
    }
    let mut i = 0;
    while let Some(nl) = p[i..].windows(2).position(|w| w == b"\r\n") {
        let line_start = i + nl + 2;
        let line_end = line_start + p[line_start..].windows(2).position(|w| w == b"\r\n")?;
        let line = &p[line_start..line_end];
        if line.len() > 5 && line[..5].eq_ignore_ascii_case(b"host:") {
            let mut v = line_start + 5;
            while p.get(v) == Some(&b' ') {
                v += 1;
            }
            let host = std::str::from_utf8(&p[v..line_end]).ok()?.trim_end().to_string();
            return Some(HostInfo { len: host.len(), host, offset: v, header_offset: line_start });
        }
        if line.is_empty() {
            return None;
        }
        i = line_end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_host() {
        let p = b"GET / HTTP/1.1\r\nUser-Agent: x\r\nHost: rutracker.org\r\n\r\n";
        let h = parse_host(p).unwrap();
        assert_eq!(h.host, "rutracker.org");
        assert_eq!(&p[h.offset..h.offset + h.len], b"rutracker.org");
    }
}
