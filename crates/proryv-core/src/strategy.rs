//! Стратегии обхода и планирование: как превратить один исходный пакет
//! в последовательность сегментов (настоящих и фейковых), которую DPI не разберёт.
//!
//! Планирование — чистая функция без сети, поэтому её легко тестировать
//! и показывать в интерфейсе («предпросмотр стратегии»).

use serde::{Deserialize, Serialize};

use crate::{http, tls};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    Start,
    HostStart,
    HostMid,
    HostEnd,
    /// Середина домена второго уровня («you|tube» в www.youtube.com).
    Midsld,
    /// Начало расширения server_name в ClientHello (sniext в zapret).
    SniExt,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub anchor: Anchor,
    #[serde(default)]
    pub add: i32,
}

impl Pos {
    pub const fn at(anchor: Anchor) -> Self {
        Pos { anchor, add: 0 }
    }
    pub const fn abs(n: i32) -> Self {
        Pos { anchor: Anchor::Start, add: n }
    }
}

/// Способы сделать фейковый пакет «невидимым» для сервера, но видимым для DPI.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fooling {
    /// Малый TTL: пакет умирает по дороге после DPI.
    Ttl,
    /// Неверный номер последовательности TCP.
    BadSeq,
    /// Неверная контрольная сумма TCP.
    BadSum,
    /// TCP-опция MD5 signature — сервер такой пакет отбросит.
    Md5Sig,
    /// Уменьшить TCP timestamp: сервер отбросит пакет как старый (PAWS).
    /// Работает, только если в Windows включены TCP timestamps.
    Ts,
}

/// Какое имя сайта класть в фейковый ClientHello.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum FakeSni {
    Fixed(String),
    /// Случайное разрешённое имя из пула — каждый раз разное (полиморфизм).
    Random,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Desync {
    Split { at: Pos },
    MultiSplit { at: Vec<Pos> },
    Disorder { at: Pos },
    MultiDisorder { at: Vec<Pos> },
    /// seqovl: перед первым куском подложить `len` байт поддельного ClientHello
    /// с номером последовательности «из прошлого». Сервер их отбросит, DPI — прочитает.
    SeqOverlap { len: u16, sni: String },
    Fake { ttl: u8, fooling: Vec<Fooling>, sni: FakeSni, repeats: u8 },
    /// Разбить одну TLS-запись на две (меняет сами байты, сервер соберёт).
    TlsRecordSplit { at: Pos },
    /// HTTP: «Host:» → «hoSt:».
    HostCase,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<Desync>,
}

/// Пул «безобидных» имён для случайных фейков.
pub const FAKE_SNI_POOL: &[&str] = &[
    "www.google.com", "yandex.ru", "vk.com", "www.microsoft.com", "www.apple.com",
    "ok.ru", "mail.ru", "www.wikipedia.org", "www.bing.com", "gosuslugi.ru",
];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Proto {
    Tls,
    Http,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegKind {
    Real,
    Fake,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Segment {
    pub kind: SegKind,
    /// Сдвиг номера последовательности TCP относительно исходного пакета.
    pub seq_offset: u32,
    pub data: Vec<u8>,
    pub ttl: Option<u8>,
    pub fooling: Vec<Fooling>,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Plan {
    pub proto: Proto,
    pub host: Option<String>,
    /// Payload после модификаций (может отличаться от исходного, напр. TLS record split).
    pub payload_len: usize,
    pub segments: Vec<Segment>,
}

struct Ctx {
    proto: Proto,
    host: Option<String>,
    host_off: usize,
    host_len: usize,
}

impl Ctx {
    fn sld(&self) -> Option<(usize, usize)> {
        let host = self.host.as_deref()?;
        let labels: Vec<&str> = host.split('.').collect();
        if labels.len() < 2 {
            return None;
        }
        let sld = labels[labels.len() - 2];
        let start: usize = labels[..labels.len() - 2].iter().map(|l| l.len() + 1).sum();
        Some((self.host_off + start, sld.len()))
    }
}

fn context(p: &[u8]) -> Ctx {
    if let Some(s) = tls::parse_sni(p) {
        return Ctx { proto: Proto::Tls, host: Some(s.host), host_off: s.offset, host_len: s.len };
    }
    if let Some(h) = http::parse_host(p) {
        return Ctx { proto: Proto::Http, host: Some(h.host), host_off: h.offset, host_len: h.len };
    }
    Ctx { proto: Proto::Unknown, host: None, host_off: 0, host_len: 0 }
}

fn resolve(pos: Pos, c: &Ctx, len: usize) -> Option<usize> {
    let base = match pos.anchor {
        Anchor::Start => 0,
        _ if c.host.is_none() => return None,
        Anchor::HostStart => c.host_off,
        Anchor::HostMid => c.host_off + c.host_len / 2,
        Anchor::HostEnd => c.host_off + c.host_len,
        Anchor::Midsld => match c.sld() {
            Some((off, len)) => off + len / 2,
            None => c.host_off + c.host_len / 2,
        },
        // type(2) len(2) list_len(2) name_type(1) name_len(2) — 9 байт до самого имени.
        Anchor::SniExt if c.proto == Proto::Tls => c.host_off - 9,
        Anchor::SniExt => c.host_off,
    } as i64;
    let v = base + pos.add as i64;
    (v > 0 && (v as usize) < len).then_some(v as usize)
}

/// Простой xorshift — чтобы не тянуть зависимость ради выбора из пула.
fn next_rand(seed: &mut u64) -> u64 {
    let mut x = *seed | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    x
}

fn tls_record_split(p: &[u8], at: usize) -> Option<Vec<u8>> {
    let body_len = ((p[3] as usize) << 8) | p[4] as usize;
    if at <= 5 || at >= 5 + body_len || 5 + body_len > p.len() {
        return None;
    }
    let first = at - 5;
    let second = body_len - first;
    let mut out = Vec::with_capacity(p.len() + 5);
    out.extend_from_slice(&p[..3]);
    out.extend_from_slice(&(first as u16).to_be_bytes());
    out.extend_from_slice(&p[5..at]);
    out.extend_from_slice(&p[..3]);
    out.extend_from_slice(&(second as u16).to_be_bytes());
    out.extend_from_slice(&p[at..]);
    Some(out)
}

pub fn plan(original: &[u8], strategy: &Strategy, mut seed: u64) -> Plan {
    let mut payload = original.to_vec();
    let mut ctx = context(&payload);

    // 1. Модификаторы, меняющие сами байты.
    for step in &strategy.steps {
        match step {
            Desync::TlsRecordSplit { at } if ctx.proto == Proto::Tls => {
                if let Some(k) = resolve(*at, &ctx, payload.len()) {
                    if let Some(np) = tls_record_split(&payload, k) {
                        payload = np;
                        if ctx.host_off >= k {
                            ctx.host_off += 5;
                        }
                    }
                }
            }
            Desync::HostCase if ctx.proto == Proto::Http => {
                if let Some(h) = http::parse_host(&payload) {
                    payload[h.header_offset..h.header_offset + 4].copy_from_slice(b"hoSt");
                }
            }
            _ => {}
        }
    }
    let len = payload.len();

    // 2. Точки разреза и порядок.
    let mut cuts = Vec::new();
    let mut disorder = false;
    for step in &strategy.steps {
        match step {
            Desync::Split { at } => cuts.extend(resolve(*at, &ctx, len)),
            Desync::MultiSplit { at } => cuts.extend(at.iter().filter_map(|p| resolve(*p, &ctx, len))),
            Desync::Disorder { at } => {
                cuts.extend(resolve(*at, &ctx, len));
                disorder = true;
            }
            Desync::MultiDisorder { at } => {
                cuts.extend(at.iter().filter_map(|p| resolve(*p, &ctx, len)));
                disorder = true;
            }
            _ => {}
        }
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut segments = Vec::new();
    let mut rnd = [0u8; 64];
    let fill_rnd = |seed: &mut u64, rnd: &mut [u8; 64]| {
        for c in rnd.chunks_mut(8) {
            c.copy_from_slice(&next_rand(seed).to_le_bytes());
        }
    };
    let real_sid = tls::session_id(original).map(|s| s.to_vec());

    // 3. Фейки идут первыми: реалистичный ClientHello к «разрешённому» сайту
    //    с тем же session id, что у настоящего (как dupsid в zapret).
    for step in &strategy.steps {
        if let Desync::Fake { ttl, fooling, sni, repeats } = step {
            let name = match sni {
                FakeSni::Fixed(s) => s.clone(),
                FakeSni::Random => {
                    FAKE_SNI_POOL[(next_rand(&mut seed) % FAKE_SNI_POOL.len() as u64) as usize].to_string()
                }
            };
            fill_rnd(&mut seed, &mut rnd);
            let data = if ctx.proto == Proto::Http {
                format!("GET / HTTP/1.1\r\nHost: {name}\r\nUser-Agent: Mozilla/5.0\r\nAccept: */*\r\n\r\n").into_bytes()
            } else {
                tls::build_realistic_hello(&name, &rnd, real_sid.as_deref(), len.min(1300))
            };
            for r in 0..(*repeats).max(1) {
                segments.push(Segment {
                    kind: SegKind::Fake,
                    seq_offset: 0,
                    data: data.clone(),
                    ttl: fooling.contains(&Fooling::Ttl).then_some(*ttl),
                    fooling: fooling.clone(),
                    label: format!("фейк #{} ({name})", r + 1),
                });
            }
        }
    }

    let mut bounds = vec![0];
    bounds.extend(&cuts);
    bounds.push(len);
    let mut real: Vec<Segment> = bounds
        .windows(2)
        .enumerate()
        .map(|(i, w)| Segment {
            kind: SegKind::Real,
            seq_offset: w[0] as u32,
            data: payload[w[0]..w[1]].to_vec(),
            ttl: None,
            fooling: vec![],
            label: format!("часть {} [{}..{})", i + 1, w[0], w[1]),
        })
        .collect();
    if disorder && real.len() > 1 {
        real.reverse();
    }

    // 5. seqovl: к куску с начала данных спереди приклеиваем поддельный ClientHello.
    for step in &strategy.steps {
        if let Desync::SeqOverlap { len: ovl, sni } = step {
            let ovl = *ovl as usize;
            if ovl == 0 {
                continue;
            }
            fill_rnd(&mut seed, &mut rnd);
            let mut pattern = tls::build_realistic_hello(sni, &rnd, real_sid.as_deref(), ovl);
            pattern.truncate(ovl);
            if let Some(first) = real.iter_mut().find(|s| s.seq_offset == 0) {
                pattern.extend_from_slice(&first.data);
                first.data = pattern;
                first.seq_offset = 0u32.wrapping_sub(ovl as u32);
                first.label = format!("{} + seqovl {ovl} Б ({sni})", first.label);
            }
        }
    }
    segments.extend(real);

    Plan { proto: ctx.proto, host: ctx.host, payload_len: len, segments }
}

impl Strategy {
    /// Короткая запись в стиле параметров zapret — для интерфейса и логов.
    pub fn summary(&self) -> String {
        fn pos(p: &Pos) -> String {
            let a = match p.anchor {
                Anchor::Start => return p.add.to_string(),
                Anchor::HostStart => "host",
                Anchor::HostMid => "midhost",
                Anchor::HostEnd => "endhost",
                Anchor::Midsld => "midsld",
                Anchor::SniExt => "sniext",
            };
            match p.add {
                0 => a.into(),
                n => format!("{a}{n:+}"),
            }
        }
        self.steps
            .iter()
            .map(|s| match s {
                Desync::Split { at } => format!("split={}", pos(at)),
                Desync::MultiSplit { at } => format!("multisplit={}", at.iter().map(pos).collect::<Vec<_>>().join(",")),
                Desync::Disorder { at } => format!("disorder={}", pos(at)),
                Desync::MultiDisorder { at } => format!("multidisorder={}", at.iter().map(pos).collect::<Vec<_>>().join(",")),
                Desync::SeqOverlap { len, sni } => format!("seqovl={len} ({sni})"),
                Desync::Fake { ttl, fooling, repeats, .. } => {
                    let f: Vec<_> = fooling.iter().map(|f| format!("{f:?}").to_lowercase()).collect();
                    format!("fake ttl={ttl} fool={} x{repeats}", f.join("+"))
                }
                Desync::TlsRecordSplit { at } => format!("tlsrec={}", pos(at)),
                Desync::HostCase => "hostcase".into(),
            })
            .collect::<Vec<_>>()
            .join("  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strat(steps: Vec<Desync>) -> Strategy {
        Strategy { id: "t".into(), name: "t".into(), description: String::new(), steps }
    }

    fn reassemble(plan: &Plan) -> Vec<u8> {
        let mut real: Vec<_> = plan.segments.iter().filter(|s| s.kind == SegKind::Real).collect();
        real.sort_by_key(|s| s.seq_offset);
        real.iter().flat_map(|s| s.data.clone()).collect()
    }

    #[test]
    fn split_mid_host_cuts_sni() {
        let p = tls::build_client_hello("www.youtube.com");
        let sni = tls::parse_sni(&p).unwrap();
        let plan = plan(&p, &strat(vec![Desync::Split { at: Pos::at(Anchor::HostMid) }]), 1);
        assert_eq!(plan.segments.len(), 2);
        assert_eq!(plan.segments[0].data.len(), sni.offset + sni.len / 2);
        assert_eq!(reassemble(&plan), p);
    }

    #[test]
    fn disorder_reverses_and_still_reassembles() {
        let p = tls::build_client_hello("discord.com");
        let plan = plan(&p, &strat(vec![Desync::Disorder { at: Pos::abs(2) }]), 1);
        assert_eq!(plan.segments[0].seq_offset, 2);
        assert_eq!(plan.segments[1].seq_offset, 0);
        assert_eq!(reassemble(&plan), p);
    }

    #[test]
    fn fakes_go_first_with_ttl() {
        let p = tls::build_client_hello("discord.com");
        let s = strat(vec![
            Desync::Fake { ttl: 4, fooling: vec![Fooling::Ttl], sni: FakeSni::Random, repeats: 2 },
            Desync::Split { at: Pos::abs(1) },
        ]);
        let plan = plan(&p, &s, 42);
        assert_eq!(plan.segments.len(), 4);
        assert!(plan.segments[..2].iter().all(|s| s.kind == SegKind::Fake && s.ttl == Some(4)));
        let fake_sni = tls::parse_sni(&plan.segments[0].data).unwrap().host;
        assert!(FAKE_SNI_POOL.contains(&fake_sni.as_str()));
    }

    #[test]
    fn tls_record_split_is_valid() {
        let p = tls::build_client_hello("www.youtube.com");
        let plan = plan(&p, &strat(vec![Desync::TlsRecordSplit { at: Pos::at(Anchor::HostMid) }]), 1);
        let out = reassemble(&plan);
        assert_eq!(out.len(), p.len() + 5);
        let first = ((out[3] as usize) << 8) | out[4] as usize;
        assert_eq!(out[5 + first], 0x16); // вторая запись начинается сразу после первой
    }

    #[test]
    fn seqovl_prepends_fake_before_seq() {
        let p = tls::build_client_hello("www.youtube.com");
        let s = strat(vec![
            Desync::MultiSplit { at: vec![Pos::abs(1)] },
            Desync::SeqOverlap { len: 681, sni: "www.google.com".into() },
        ]);
        let plan = plan(&p, &s, 3);
        let first = plan.segments.iter().find(|s| s.seq_offset == 0u32.wrapping_sub(681)).unwrap();
        assert_eq!(first.data.len(), 682);
        assert_eq!(first.data[681], p[0]);
        assert_eq!(tls::parse_sni(&first.data[..681]).unwrap().host, "www.google.com");
        let rest = plan.segments.iter().find(|s| s.seq_offset == 1).unwrap();
        assert_eq!(rest.data, p[1..]);
    }

    #[test]
    fn midsld_and_multidisorder() {
        let p = tls::build_client_hello("www.youtube.com");
        let sni = tls::parse_sni(&p).unwrap();
        let plan = plan(&p, &strat(vec![Desync::MultiDisorder { at: vec![Pos::abs(1), Pos::at(Anchor::Midsld)] }]), 1);
        let offs: Vec<u32> = plan.segments.iter().map(|s| s.seq_offset).collect();
        let mid = (sni.offset + 4 + 7 / 2) as u32; // "www." + половина "youtube"
        assert_eq!(offs, vec![mid, 1, 0]);
        assert_eq!(reassemble(&plan), p);
    }

    #[test]
    fn fake_is_realistic_and_reuses_session_id() {
        let p = tls::build_client_hello("www.youtube.com");
        let s = strat(vec![Desync::Fake { ttl: 0, fooling: vec![Fooling::BadSeq], sni: FakeSni::Fixed("www.google.com".into()), repeats: 1 }]);
        let plan = plan(&p, &s, 5);
        let fake = &plan.segments[0].data;
        assert_eq!(tls::parse_sni(fake).unwrap().host, "www.google.com");
        assert_eq!(tls::session_id(fake), tls::session_id(&p));
        assert!(fake.len() >= 517);
    }

    #[test]
    fn http_hostcase() {
        let p = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let plan = plan(p, &strat(vec![Desync::HostCase]), 1);
        assert!(reassemble(&plan).windows(5).any(|w| w == b"hoSt:"));
    }

    #[test]
    fn serde_roundtrip() {
        let s = strat(vec![Desync::Fake { ttl: 3, fooling: vec![Fooling::BadSeq], sni: FakeSni::Fixed("a.ru".into()), repeats: 1 }]);
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Strategy>(&j).unwrap(), s);
    }
}
