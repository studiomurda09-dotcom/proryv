//! Синтаксис zapret (winws/nfqws): строка параметров или целый .bat → стратегии Proryv.
//! И обратно: любую стратегию можно показать строкой zapret.
//!
//! Переводится то, что умеет движок Proryv. Остальное не выкидывается молча:
//! каждый пропущенный параметр попадает в `warnings`.

use serde::Serialize;

use crate::strategy::{Anchor, Desync, FakeSni, Fooling, Pos, Strategy};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Parsed {
    /// Откуда блок: «TCP 443 · list-google».
    pub label: String,
    pub strategy: Strategy,
    /// Что понято — для зелёных меток в интерфейсе.
    pub recognized: Vec<String>,
    pub warnings: Vec<String>,
    /// Исходные параметры блока без фильтров и списков.
    pub zapret: String,
}

/// Разобрать строку или .bat. Блоки `--new` дают отдельные стратегии;
/// блоки только для UDP пропускаются (голос и QUIC Proryv обрабатывает сам).
pub fn parse(text: &str) -> Result<Vec<Parsed>, String> {
    let tokens = tokenize(text);
    let mut blocks: Vec<Vec<(String, String)>> = vec![vec![]];
    for t in tokens {
        let Some(arg) = t.strip_prefix("--") else { continue };
        if arg == "new" {
            blocks.push(vec![]);
            continue;
        }
        let (k, v) = arg.split_once('=').unwrap_or((arg, ""));
        blocks.last_mut().unwrap().push((k.to_string(), v.to_string()));
    }
    let out: Vec<Parsed> = blocks.iter().filter_map(|b| parse_block(b)).collect();
    if out.is_empty() {
        return Err("Не нашёл ни одного TCP-блока с --dpi-desync".into());
    }
    Ok(out)
}

/// Слова командной строки: кавычки, `^` в конце строки .bat, `%VAR%` убираются.
fn tokenize(text: &str) -> Vec<String> {
    let text = text.replace("^\r\n", " ").replace("^\n", " ");
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn file_stem(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let name = name.rsplit('%').next().unwrap_or(name);
    name.trim_end_matches(".bin").trim_end_matches(".txt").to_string()
}

/// «tls_clienthello_www_google_com.bin» → «www.google.com».
fn sni_from_file(path: &str) -> Option<String> {
    let stem = file_stem(path);
    let host = stem.strip_prefix("tls_clienthello_")?;
    (!host.is_empty()).then(|| host.replace('_', "."))
}

fn parse_pos(s: &str) -> Option<Pos> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i32>() {
        return Some(Pos::abs(n));
    }
    let split = s.find(['+', '-']).unwrap_or(s.len());
    let (name, add) = s.split_at(split);
    let add = if add.is_empty() { 0 } else { add.parse().ok()? };
    let anchor = match name {
        "host" => Anchor::HostStart,
        "endhost" => Anchor::HostEnd,
        "midsld" => Anchor::Midsld,
        "midhost" => Anchor::HostMid,
        "sniext" => Anchor::SniExt,
        "method" => Anchor::Start,
        _ => return None,
    };
    Some(Pos { anchor, add })
}

fn parse_block(args: &[(String, String)]) -> Option<Parsed> {
    let get = |k: &str| args.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    let modes: Vec<&str> = get("dpi-desync")?.split(',').filter(|m| !m.is_empty()).collect();
    let tcp = get("filter-tcp");
    if tcp.is_none() && get("filter-udp").is_some() {
        return None;
    }

    let mut recognized = Vec::new();
    let mut warnings = Vec::new();
    let mut steps = Vec::new();

    let positions: Vec<Pos> = match get("dpi-desync-split-pos") {
        Some(list) => list
            .split(',')
            .filter_map(|p| {
                let r = parse_pos(p);
                if r.is_none() {
                    warnings.push(format!("позиция «{p}» не поддерживается — пропущена"));
                }
                r
            })
            .collect(),
        None => vec![Pos::abs(2)],
    };
    let pos_text = get("dpi-desync-split-pos").unwrap_or("2");

    let fake_modes = ["fake", "fakedsplit", "fakeddisorder", "hostfakesplit"];
    let mut fooling = Vec::new();
    let mut ttl = 0;
    let autottl = get("dpi-desync-autottl").and_then(|v| {
        let r = parse_autottl(v);
        if r.is_none() {
            warnings.push(format!("--dpi-desync-autottl={v} не понят — пропущен"));
        }
        r
    });
    if modes.iter().any(|m| fake_modes.contains(m)) {
        for f in get("dpi-desync-fooling").unwrap_or("").split(',').filter(|f| !f.is_empty()) {
            match fooling_from(f) {
                Some(x) => fooling.push(x),
                None => warnings.push(format!("fooling={f} не поддерживается — пропущен")),
            }
        }
        if let Some(t) = get("dpi-desync-ttl").and_then(|v| v.parse::<u8>().ok()) {
            ttl = t.min(64);
            fooling.push(Fooling::Ttl);
        } else if let Some((_, min, _)) = autottl {
            // Пока хопы неизвестны, фейк всё равно не должен дойти до сервера.
            ttl = min;
            fooling.push(Fooling::Ttl);
        }
        if fooling.is_empty() {
            warnings.push("у фейка нет fooling и ttl — он может дойти до сервера и сломать соединение".into());
        } else {
            let names: Vec<&str> = fooling.iter().map(fooling_name).collect();
            recognized.push(format!("fooling={}", names.join(",")) + &if ttl > 0 { format!(" ttl={ttl}") } else { String::new() });
        }
    }

    if modes.contains(&"fake") {
        let repeats: u8 = get("dpi-desync-repeats").and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 32);
        let sni = args
            .iter()
            .filter(|(k, _)| k == "dpi-desync-fake-tls")
            .filter_map(|(_, v)| sni_from_file(v))
            .last()
            .map(FakeSni::Fixed)
            .unwrap_or(FakeSni::Random);
        recognized.push(format!("fake ×{repeats}"));
        match &sni {
            FakeSni::Fixed(s) => recognized.push(format!("fake-tls → {s}")),
            FakeSni::Random => recognized.push("фейк: случайный сайт".into()),
        }
        steps.push(Desync::Fake { ttl, fooling: fooling.clone(), sni, repeats });
    }

    for m in &modes {
        match *m {
            "fake" => {}
            "multisplit" => {
                steps.push(Desync::MultiSplit { at: positions.clone() });
                recognized.push(format!("multisplit @{pos_text}"));
            }
            "split" | "split2" => {
                steps.push(Desync::Split { at: positions.first().copied().unwrap_or(Pos::abs(2)) });
                recognized.push(format!("split @{pos_text}"));
            }
            "multidisorder" => {
                steps.push(Desync::MultiDisorder { at: positions.clone() });
                recognized.push(format!("multidisorder @{pos_text}"));
            }
            "disorder" | "disorder2" => {
                steps.push(Desync::Disorder { at: positions.first().copied().unwrap_or(Pos::abs(2)) });
                recognized.push(format!("disorder @{pos_text}"));
            }
            "fakedsplit" | "fakeddisorder" => {
                let disorder = *m == "fakeddisorder";
                steps.push(Desync::FakedSplit { at: positions.first().copied().unwrap_or(Pos::abs(2)), ttl, fooling: fooling.clone(), disorder });
                recognized.push(format!("{m} @{pos_text}"));
            }
            "hostfakesplit" => {
                steps.push(Desync::HostFakeSplit { ttl, fooling: fooling.clone() });
                recognized.push("hostfakesplit".into());
            }
            other => warnings.push(format!("режим {other} пока не поддерживается — пропущен")),
        }
    }

    if let Some(len) = get("dpi-desync-split-seqovl").and_then(|v| v.parse::<u16>().ok()).filter(|l| *l > 0) {
        let len = len.min(1500);
        let sni = get("dpi-desync-split-seqovl-pattern").and_then(sni_from_file).unwrap_or_else(|| "www.google.com".into());
        if !steps.iter().any(|s| matches!(s, Desync::Split { .. } | Desync::MultiSplit { .. } | Desync::Disorder { .. } | Desync::MultiDisorder { .. })) {
            steps.push(Desync::MultiSplit { at: vec![Pos::abs(1)] });
        }
        steps.push(Desync::SeqOverlap { len, sni: sni.clone() });
        recognized.push(format!("seqovl {len} ({sni})"));
    }

    if let Some((delta, min, max)) = autottl {
        if steps.iter().any(|s| matches!(s, Desync::Fake { .. } | Desync::FakedSplit { .. } | Desync::HostFakeSplit { .. })) {
            steps.push(Desync::AutoTtl { delta, min, max });
            recognized.push(format!("autottl {delta:+} ({min}–{max})"));
        }
    }

    for (k, _) in args {
        let note = match k.as_str() {
            "ip-id" => Some("--ip-id задаётся пресетом (у YouTube уже zero) — пропущен"),
            "dpi-desync-cutoff" => Some("--dpi-desync-cutoff пока не поддерживается — пропущен"),
            "dpi-desync-any-protocol" => Some("--dpi-desync-any-protocol пока не поддерживается — пропущен"),
            "dpi-desync-start" => Some("--dpi-desync-start пока не поддерживается — пропущен"),
            _ => None,
        };
        if let Some(n) = note {
            if !warnings.iter().any(|w| w == n) {
                warnings.push(n.to_string());
            }
        }
    }

    if steps.is_empty() {
        return None;
    }

    let mut label = match tcp {
        Some(p) => format!("TCP {p}"),
        None => "TCP".to_string(),
    };
    if let Some(list) = get("hostlist").or(get("ipset")) {
        label += &format!(" · {}", file_stem(list));
    } else if let Some(d) = get("hostlist-domains") {
        label += &format!(" · {d}");
    }
    let zapret = to_zapret_args(&steps);
    let id = format!("custom_{:08x}", fnv(&zapret));
    Some(Parsed { label: label.clone(), strategy: Strategy { id, name: label, description: zapret.clone(), steps }, recognized, warnings, zapret })
}

fn fooling_from(s: &str) -> Option<Fooling> {
    Some(match s {
        "ts" => Fooling::Ts,
        "badseq" => Fooling::BadSeq,
        "badsum" => Fooling::BadSum,
        "md5sig" => Fooling::Md5Sig,
        "datanoack" => Fooling::DataNoAck,
        _ => return None,
    })
}

fn fooling_name(f: &Fooling) -> &'static str {
    match f {
        Fooling::Ts => "ts",
        Fooling::BadSeq => "badseq",
        Fooling::BadSum => "badsum",
        Fooling::Md5Sig => "md5sig",
        Fooling::DataNoAck => "datanoack",
        Fooling::Ttl => "ttl",
    }
}

/// `--dpi-desync-autottl=[delta[:min[-max]]]`. Число без знака — сколько хопов
/// отнять (как в старых zapret), поэтому «2» = −2. По умолчанию −1:3-20.
fn parse_autottl(v: &str) -> Option<(i8, u8, u8)> {
    let (d, range) = v.split_once(':').unwrap_or((v, ""));
    let delta = match d.trim() {
        "" => -1,
        d if d.starts_with(['+', '-']) => d.parse().ok()?,
        d => -d.parse::<i8>().ok()?,
    };
    let (min, max) = match range.split_once('-') {
        Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
        None if range.is_empty() => (3, 20),
        None => (range.parse().ok()?, 20),
    };
    (min >= 1 && min <= max && max <= 64 && (-20..=20).contains(&delta)).then_some((delta, min, max))
}

fn fnv(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

fn pos_str(p: &Pos) -> String {
    let name = match p.anchor {
        Anchor::Start => return p.add.to_string(),
        Anchor::HostStart => "host",
        Anchor::HostEnd => "endhost",
        Anchor::Midsld => "midsld",
        Anchor::SniExt => "sniext",
        Anchor::HostMid => "midhost",
    };
    match p.add {
        0 => name.to_string(),
        a => format!("{name}{a:+}"),
    }
}

/// Стратегия → параметры zapret (для показа и экспорта).
pub fn to_zapret(s: &Strategy) -> String {
    to_zapret_args(&s.steps)
}

fn to_zapret_args(steps: &[Desync]) -> String {
    let mut modes = Vec::new();
    let mut extra: Vec<String> = Vec::new();
    fn add(extra: &mut Vec<String>, a: String) {
        if !extra.contains(&a) {
            extra.push(a);
        }
    }
    fn fool(extra: &mut Vec<String>, ttl: u8, fooling: &[Fooling]) {
        let f: Vec<&str> = fooling.iter().filter(|f| **f != Fooling::Ttl).map(fooling_name).collect();
        if !f.is_empty() {
            add(extra, format!("--dpi-desync-fooling={}", f.join(",")));
        }
        if fooling.contains(&Fooling::Ttl) {
            add(extra, format!("--dpi-desync-ttl={ttl}"));
        }
    }
    for st in steps {
        match st {
            Desync::Fake { ttl, fooling, sni, repeats } => {
                modes.push("fake");
                fool(&mut extra, *ttl, fooling);
                add(&mut extra, format!("--dpi-desync-repeats={repeats}"));
                if let FakeSni::Fixed(h) = sni {
                    add(&mut extra, format!("--dpi-desync-fake-tls=tls_clienthello_{}.bin", h.replace('.', "_")));
                }
            }
            Desync::Split { at } | Desync::Disorder { at } => {
                modes.push(if matches!(st, Desync::Split { .. }) { "split2" } else { "disorder2" });
                add(&mut extra, format!("--dpi-desync-split-pos={}", pos_str(at)));
            }
            Desync::MultiSplit { at } | Desync::MultiDisorder { at } => {
                modes.push(if matches!(st, Desync::MultiSplit { .. }) { "multisplit" } else { "multidisorder" });
                add(&mut extra, format!("--dpi-desync-split-pos={}", at.iter().map(pos_str).collect::<Vec<_>>().join(",")));
            }
            Desync::FakedSplit { at, ttl, fooling, disorder } => {
                modes.push(if *disorder { "fakeddisorder" } else { "fakedsplit" });
                fool(&mut extra, *ttl, fooling);
                add(&mut extra, format!("--dpi-desync-split-pos={}", pos_str(at)));
            }
            Desync::HostFakeSplit { ttl, fooling } => {
                modes.push("hostfakesplit");
                fool(&mut extra, *ttl, fooling);
            }
            Desync::AutoTtl { delta, min, max } => add(&mut extra, format!("--dpi-desync-autottl={delta:+}:{min}-{max}")),
            Desync::SeqOverlap { len, sni } => {
                add(&mut extra, format!("--dpi-desync-split-seqovl={len}"));
                add(&mut extra, format!("--dpi-desync-split-seqovl-pattern=tls_clienthello_{}.bin", sni.replace('.', "_")));
            }
            Desync::TlsRecordSplit { .. } => add(&mut extra, "# tlsrec (только Proryv)".into()),
            Desync::HostCase => add(&mut extra, "--hostcase".into()),
        }
    }
    let mut out = vec![format!("--dpi-desync={}", modes.join(","))];
    out.extend(extra);
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::builtin_strategies;

    fn builtin(id: &str) -> Strategy {
        builtin_strategies().into_iter().find(|s| s.id == id).unwrap()
    }

    /// Блок из general (ALT11).bat zapret-discord-youtube 1.10.3.
    const ALT11_BAT: &str = r#"start "zapret: %~n0" /min "%BIN%winws.exe" --wf-tcp=80,443 --wf-udp=443,19294-19344 ^
--filter-udp=19294-19344,50000-50100 --filter-l7=discord,stun --dpi-desync=fake --dpi-desync-repeats=6 --new ^
--filter-tcp=443 --hostlist="%LISTS%list-google.txt" --ip-id=zero --dpi-desync=fake,multisplit --dpi-desync-split-seqovl=681 --dpi-desync-split-pos=1 --dpi-desync-fooling=ts --dpi-desync-repeats=8 --dpi-desync-split-seqovl-pattern="%BIN%tls_clienthello_www_google_com.bin" --dpi-desync-fake-tls="%BIN%tls_clienthello_www_google_com.bin" --new ^
--filter-tcp=80,443 --hostlist="%LISTS%list-general.txt" --dpi-desync=fake,multisplit --dpi-desync-split-seqovl=664 --dpi-desync-split-pos=1 --dpi-desync-fooling=ts --dpi-desync-repeats=8 --dpi-desync-split-seqovl-pattern="%BIN%tls_clienthello_max_ru.bin" --dpi-desync-fake-tls="%BIN%stun2.bin" --dpi-desync-fake-tls="%BIN%tls_clienthello_max_ru.bin""#;

    #[test]
    fn alt11_bat_matches_builtin_alt11() {
        let blocks = parse(ALT11_BAT).unwrap();
        assert_eq!(blocks.len(), 2, "UDP-блок голоса пропускается");
        assert_eq!(blocks[0].label, "TCP 443 · list-google");
        assert_eq!(blocks[0].strategy.steps, builtin("alt11").steps);
        assert!(blocks[0].warnings.iter().any(|w| w.contains("ip-id")));
        match &blocks[1].strategy.steps[0] {
            Desync::Fake { sni: FakeSni::Fixed(s), .. } => assert_eq!(s, "max.ru"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(blocks[1].strategy.steps[2], Desync::SeqOverlap { len: 664, .. }));
    }

    #[test]
    fn alt7_line_matches_builtin() {
        let p = parse("--dpi-desync=multisplit --dpi-desync-split-pos=2,sniext+1 --dpi-desync-split-seqovl=679 --dpi-desync-split-seqovl-pattern=tls_clienthello_www_google_com.bin").unwrap();
        assert_eq!(p[0].strategy.steps, builtin("alt7").steps);
        assert!(p[0].warnings.is_empty());
    }

    #[test]
    fn unsupported_is_reported_not_dropped() {
        let p = parse("--dpi-desync=fake,fakeknot --dpi-desync-fooling=hopbyhop,ts --dpi-desync-split-pos=1,sld+1").unwrap();
        let w = p[0].warnings.join("\n");
        assert!(w.contains("fakeknot") && w.contains("hopbyhop") && w.contains("sld+1"), "{w}");
    }

    #[test]
    fn faked_modes_and_autottl() {
        let p = parse("--dpi-desync=fake,fakedsplit --dpi-desync-autottl=2 --dpi-desync-fooling=datanoack,ts --dpi-desync-split-pos=1").unwrap();
        let s = &p[0].strategy.steps;
        assert!(p[0].warnings.is_empty(), "{:?}", p[0].warnings);
        assert!(matches!(&s[1], Desync::FakedSplit { ttl: 3, disorder: false, fooling, .. } if fooling == &vec![Fooling::DataNoAck, Fooling::Ts, Fooling::Ttl]));
        assert_eq!(s[2], Desync::AutoTtl { delta: -2, min: 3, max: 20 });
        let back = parse(&to_zapret(&p[0].strategy)).unwrap();
        assert_eq!(&back[0].strategy.steps, s);
        let h = parse("--dpi-desync=hostfakesplit --dpi-desync-fooling=badseq").unwrap();
        assert_eq!(h[0].strategy.steps, vec![Desync::HostFakeSplit { ttl: 0, fooling: vec![Fooling::BadSeq] }]);
        assert_eq!(parse_autottl(""), Some((-1, 3, 20)));
        assert_eq!(parse_autottl("+1:4-12"), Some((1, 4, 12)));
        assert_eq!(parse_autottl("1:30-5"), None);
    }

    #[test]
    fn builtins_round_trip_through_zapret_syntax() {
        for s in builtin_strategies() {
            if s.steps.iter().any(|d| matches!(d, Desync::HostCase | Desync::TlsRecordSplit { .. })) {
                continue;
            }
            let back = parse(&to_zapret(&s)).unwrap_or_else(|e| panic!("{}: {e}", s.id));
            assert_eq!(back[0].strategy.steps, s.steps, "{}: {}", s.id, to_zapret(&s));
        }
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("привет").is_err());
        assert!(parse("--filter-udp=443 --dpi-desync=fake").is_err());
    }
}
