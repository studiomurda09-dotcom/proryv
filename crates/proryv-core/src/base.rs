//! Обновляемая база: добавки к пресетам и стратегии по провайдерам.
//!
//! Файл `data/base.json` лежит в репозитории на GitHub. Программа скачивает его
//! при запуске, поэтому списки доменов и стратегии обновляются без новой версии.
//! Встроенные пресеты остаются основой, база только дополняет их.

use std::{collections::HashMap, process::Command, sync::RwLock};

use serde::{Deserialize, Serialize};

use crate::{
    presets::{self, Preset},
    strategy::{Desync, Pos, Strategy},
};

pub const BASE_URL: &str = "https://raw.githubusercontent.com/studiomurda09-dotcom/proryv/main/data/base.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Base {
    pub version: u32,
    /// Дополнительные домены: id пресета → домены.
    pub domains: HashMap<String, Vec<String>>,
    /// Новые стратегии или замена встроенных с тем же id.
    pub strategies: Vec<Strategy>,
    /// Провайдеры: какая стратегия у них работает.
    pub isp: Vec<IspEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IspEntry {
    /// Номер автономной системы провайдера (AS12389 → 12389).
    pub asn: u32,
    pub name: String,
    /// id пресета → id стратегии.
    pub strategies: HashMap<String, String>,
}

/// Провайдер пользователя, как его определил ip-api.com.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Isp {
    pub asn: u32,
    pub name: String,
}

static BASE: RwLock<Option<Base>> = RwLock::new(None);
static ISP: RwLock<Option<Isp>> = RwLock::new(None);
/// Свои стратегии из режима разработчика: (все, id включённых).
static CUSTOM: RwLock<(Vec<Strategy>, Vec<String>)> = RwLock::new((Vec::new(), Vec::new()));

/// Подключить свои стратегии. Неверные (за пределами лимитов) молча не берутся.
pub fn set_custom(list: &[crate::config::CustomStrategy]) {
    let ok: Vec<&crate::config::CustomStrategy> = list.iter().filter(|c| check_strategy(&c.strategy).is_ok()).collect();
    *CUSTOM.write().unwrap() = (
        ok.iter().map(|c| c.strategy.clone()).collect(),
        ok.iter().filter(|c| c.enabled).map(|c| c.strategy.id.clone()).collect(),
    );
}

pub fn set_base(b: Base) {
    *BASE.write().unwrap() = Some(b);
}
pub fn base() -> Option<Base> {
    BASE.read().unwrap().clone()
}
pub fn set_isp(isp: Option<Isp>) {
    *ISP.write().unwrap() = isp;
}
pub fn isp() -> Option<Isp> {
    ISP.read().unwrap().clone()
}

/// Запись базы для текущего провайдера, если она есть.
pub fn isp_entry() -> Option<IspEntry> {
    find_isp(base().as_ref()?, isp()?.asn)
}
fn find_isp(b: &Base, asn: u32) -> Option<IspEntry> {
    b.isp.iter().find(|e| e.asn == asn).cloned()
}

/// Встроенные стратегии с заменами и добавками из базы, плюс свои.
pub fn strategies() -> Vec<Strategy> {
    let mut all = strategies_with(base().as_ref());
    all.extend(CUSTOM.read().unwrap().0.iter().cloned());
    all
}
fn strategies_with(b: Option<&Base>) -> Vec<Strategy> {
    let mut all = presets::builtin_strategies();
    if let Some(b) = b {
        for s in b.strategies.iter().cloned() {
            match all.iter_mut().find(|x| x.id == s.id) {
                Some(x) => *x = s,
                None => all.push(s),
            }
        }
    }
    all
}

/// Встроенные пресеты с доменами из базы. Стратегия, которая сработала у этого
/// провайдера, ставится первой — её берёт «Авто» и с неё начинает автоподбор.
pub fn presets() -> Vec<Preset> {
    let b = base();
    let entry = b.as_ref().zip(isp()).and_then(|(b, i)| find_isp(b, i.asn));
    let mut all = presets_with(b.as_ref(), entry.as_ref());
    let enabled = CUSTOM.read().unwrap().1.clone();
    for p in &mut all {
        p.strategies.extend(enabled.iter().cloned());
    }
    all
}
fn presets_with(b: Option<&Base>, entry: Option<&IspEntry>) -> Vec<Preset> {
    let mut all = presets::builtin_presets();
    let known: Vec<String> = strategies_with(b).into_iter().map(|s| s.id).collect();
    for p in &mut all {
        if let Some(extra) = b.and_then(|b| b.domains.get(&p.id)) {
            for d in extra {
                if !p.domains.contains(d) {
                    p.domains.push(d.clone());
                }
            }
        }
        if let Some(sid) = entry.and_then(|e| e.strategies.get(&p.id)) {
            if known.contains(sid) {
                p.strategies.retain(|x| x != sid);
                p.strategies.insert(0, sid.clone());
            }
        }
    }
    all
}

fn curl(args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("curl.exe");
    cmd.args(["-s", "-f", "-L", "--connect-timeout", "6", "-m", "15"]).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Скачать свежую базу с GitHub.
pub fn fetch() -> Option<Base> {
    parse(&curl(&[BASE_URL])?).ok()
}

/// Разобрать и проверить базу. База приходит из сети, поэтому любое число в ней
/// ограничено: иначе одна опечатка (или подмена) остановит обработку трафика.
pub fn parse(text: &str) -> Result<Base, String> {
    let b: Base = serde_json::from_str(text).map_err(|e| e.to_string())?;
    validate(&b)?;
    Ok(b)
}

const MAX_REPEATS: u8 = 32;
const MAX_SEQOVL: u16 = 1500;
const MAX_STEPS: usize = 16;
const MAX_CUTS: usize = 16;
const MAX_OFFSET: i32 = 4096;
const MAX_DOMAINS: usize = 5000;

fn validate(b: &Base) -> Result<(), String> {
    let mut ids: Vec<String> = presets::builtin_strategies().into_iter().map(|s| s.id).collect();
    for s in &b.strategies {
        check_strategy(s)?;
        ids.push(s.id.clone());
    }
    let pids: Vec<String> = presets::builtin_presets().into_iter().map(|p| p.id).collect();
    for (p, list) in &b.domains {
        if !pids.contains(p) {
            return Err(format!("domains: нет пресета {p}"));
        }
        if list.len() > MAX_DOMAINS || list.iter().any(|d| d.is_empty() || d.len() > 253 || d.contains(char::is_whitespace)) {
            return Err(format!("domains.{p}: неверный список"));
        }
    }
    for e in &b.isp {
        for (p, s) in &e.strategies {
            if !pids.contains(p) || !ids.contains(s) {
                return Err(format!("AS{}: неизвестный пресет или стратегия {p} → {s}", e.asn));
            }
        }
    }
    Ok(())
}

/// Лимиты на одну стратегию: из сети или от пользователя — всё равно проверяем.
pub fn check_strategy(s: &Strategy) -> Result<(), String> {
    let pos_ok = |p: &Pos| p.add.abs() <= MAX_OFFSET;
    if s.id.is_empty() || s.id.len() > 40 || !s.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
        return Err(format!("плохой id стратегии: {:?}", s.id));
    }
    if s.steps.is_empty() || s.steps.len() > MAX_STEPS {
        return Err(format!("{}: шагов должно быть 1..={MAX_STEPS}", s.id));
    }
    for step in &s.steps {
        let ok = match step {
            Desync::Split { at } | Desync::Disorder { at } | Desync::TlsRecordSplit { at } => pos_ok(at),
            Desync::MultiSplit { at } | Desync::MultiDisorder { at } => !at.is_empty() && at.len() <= MAX_CUTS && at.iter().all(pos_ok),
            Desync::SeqOverlap { len, sni } => *len <= MAX_SEQOVL && sni.len() <= 253,
            Desync::Fake { ttl, repeats, .. } => *ttl <= 64 && (1..=MAX_REPEATS).contains(repeats),
            Desync::HostCase => true,
            // Без fooling фейк-нули дойдут до сервера раньше настоящих данных и испортят их.
            Desync::FakedSplit { at, ttl, fooling, .. } => pos_ok(at) && *ttl <= 64 && !fooling.is_empty(),
            Desync::HostFakeSplit { ttl, fooling } => *ttl <= 64 && !fooling.is_empty(),
            Desync::AutoTtl { delta, min, max } => (-20..=20).contains(delta) && (1..=*max).contains(min) && *max <= 64,
        };
        if !ok {
            return Err(format!("{}: недопустимый шаг {step:?}", s.id));
        }
    }
    Ok(())
}

/// Определить провайдера. Запрос идёт через физический адаптер, мимо VPN,
/// иначе вместо провайдера определится VPN-сервер. Без прямого адаптера не гадаем.
pub fn detect_isp() -> Option<Isp> {
    #[cfg(windows)]
    let iface = crate::platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    let iface: Option<String> = None;
    let url = "http://ip-api.com/json/?fields=status,isp,as";
    let body = match &iface {
        Some(ip) => curl(&["--interface", ip, url])?,
        None => curl(&[url])?,
    };
    #[derive(Deserialize)]
    struct Resp {
        status: String,
        isp: String,
        #[serde(rename = "as")]
        asn: String,
    }
    let r: Resp = serde_json::from_str(&body).ok()?;
    if r.status != "success" {
        return None;
    }
    parse_as(&r.asn).map(|asn| Isp { asn, name: r.isp })
}

/// "AS12389 PJSC Rostelecom" → 12389.
fn parse_as(s: &str) -> Option<u32> {
    s.strip_prefix("AS")?.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_number() {
        assert_eq!(parse_as("AS12389 PJSC Rostelecom"), Some(12389));
        assert_eq!(parse_as(""), None);
    }

    #[test]
    fn repo_base_is_valid() {
        parse(include_str!("../../../data/base.json")).expect("data/base.json");
    }

    #[test]
    fn dangerous_base_is_rejected() {
        let fake = |repeats: u32| format!(r#"{{"version":2,"strategies":[{{"id":"x","name":"x","steps":[{{"kind":"fake","ttl":0,"fooling":["ts"],"sni":{{"mode":"random"}},"repeats":{repeats}}}]}}]}}"#);
        assert!(parse(&fake(8)).is_ok());
        assert!(parse(&fake(200)).is_err(), "200 фейков на пакет");
        assert!(parse(r#"{"version":2,"strategies":[{"id":"x","name":"x","steps":[{"kind":"seq_overlap","len":60000,"sni":"a"}]}]}"#).is_err());
        assert!(parse(r#"{"version":2,"strategies":[{"id":"x","name":"x","steps":[{"kind":"delay","ms":4294967295}]}]}"#).is_err());
        assert!(parse(r#"{"version":2,"strategies":[{"id":"../x","name":"x","steps":[{"kind":"host_case"}]}]}"#).is_err());
        assert!(parse(r#"{"version":2,"isp":[{"asn":1,"name":"x","strategies":{"youtube":"nope"}}]}"#).is_err());
    }

    #[test]
    fn isp_strategy_goes_first() {
        let mut strategies = HashMap::new();
        strategies.insert("youtube".to_string(), "fake_ts".to_string());
        let b = Base { version: 1, isp: vec![IspEntry { asn: 65000, name: "Test".into(), strategies }], ..Default::default() };
        let entry = find_isp(&b, 65000);
        let yt = presets_with(Some(&b), entry.as_ref()).into_iter().find(|p| p.id == "youtube").unwrap();
        assert_eq!(yt.strategies[0], "fake_ts");
    }
}
