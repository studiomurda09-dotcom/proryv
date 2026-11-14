//! Проверка сайтов и автоподбор стратегии.
//!
//! Проверка идёт через curl.exe (есть в Windows 10/11), запросы привязываются к
//! физическому адаптеру — так результат не искажает включённый VPN.

use std::{process::Command, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    config::Settings,
    engine::{Engine, RunConfig},
    platform::{self, Filter},
};

/// Адреса для проверки пресета: главная, CDN, медиа.
pub fn test_urls(preset_id: &str) -> Vec<&'static str> {
    match preset_id {
        "youtube" => vec![
            "https://www.youtube.com/generate_204",
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg",
            "https://redirector.googlevideo.com/report_mapping",
            "https://youtubei.googleapis.com/",
        ],
        "discord" => vec![
            "https://discord.com/api/v9/experiments",
            "https://cdn.discordapp.com/embed/avatars/0.png",
            "https://gateway.discord.gg/",
        ],
        "roblox" => vec!["https://www.roblox.com/", "https://setup.rbxcdn.com/version"],
        "general" => vec!["https://rutracker.org/forum/index.php", "https://soundcloud.com/", "https://medium.com/"],
        "social" => vec!["https://www.instagram.com/", "https://x.com/", "https://www.facebook.com/"],
        _ => vec![],
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Check {
    pub url: String,
    pub ok: bool,
    pub code: u16,
    pub ms: u64,
    /// Код ошибки curl (6 — DNS, 7 — нет соединения, 28 — таймаут, 35 — TLS…).
    pub curl_error: i32,
}

/// Один HTTPS-запрос. `iface` — IP адаптера, к которому привязать запрос.
pub fn curl_check(url: &str, iface: Option<&str>, timeout_s: u32) -> Check {
    let mut cmd = Command::new("curl.exe");
    cmd.args(["-s", "-o", "NUL", "-m", &timeout_s.to_string(), "--connect-timeout", "5", "-w", "%{http_code} %{time_total}"]);
    if let Some(ip) = iface {
        cmd.args(["--interface", ip]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.arg(url).output();
    let (code, ms, err) = match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            let mut it = s.split_whitespace();
            let code = it.next().and_then(|c| c.parse().ok()).unwrap_or(0);
            let secs: f64 = it.next().and_then(|c| c.parse().ok()).unwrap_or(0.0);
            (code, (secs * 1000.0) as u64, o.status.code().unwrap_or(-1))
        }
        Err(_) => (0, 0, -1),
    };
    Check { url: url.to_string(), ok: err == 0 && code > 0, code, ms, curl_error: err }
}

/// Удаётся ли открыть TCP-соединение на 443 порт. curl telnet:// после соединения
/// ждёт данных до таймаута, поэтому смотрим на time_connect, а не на код выхода.
pub fn tcp_connects(ip: &str, iface: Option<&str>) -> bool {
    let mut cmd = Command::new("curl.exe");
    cmd.args(["-s", "-o", "NUL", "-m", "4", "--connect-timeout", "4", "-w", "%{time_connect}"]);
    if let Some(i) = iface {
        cmd.args(["--interface", i]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.arg(format!("telnet://{ip}:443"))
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .is_some_and(|t| t > 0.0)
}

pub fn check_all(urls: &[&str], iface: Option<&str>) -> (usize, u64, Vec<Check>) {
    let checks: Vec<Check> = urls.iter().map(|u| curl_check(u, iface, 7)).collect();
    let ok = checks.iter().filter(|c| c.ok).count();
    let ms = checks.iter().map(|c| if c.ok { c.ms } else { 7000 }).sum();
    (ok, ms, checks)
}

/// Событие прогресса для интерфейса.
#[derive(Serialize, Clone, Debug)]
pub struct Progress {
    pub preset: String,
    /// "baseline" — без обхода, иначе id стратегии.
    pub strategy: String,
    pub strategy_name: String,
    pub index: usize,
    pub total: usize,
    pub ok: usize,
    pub of: usize,
    pub ms: u64,
    pub done: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct Outcome {
    pub preset: String,
    pub baseline_ok: usize,
    pub of: usize,
    /// Лучшая стратегия (None — ни одна не помогла или сайт и так открывается).
    pub best: Option<String>,
    pub best_name: Option<String>,
    pub results: Vec<Progress>,
    pub direct_ip: Option<String>,
}

/// Перебрать стратегии пресета и найти лучшую.
/// Основной движок на время подбора должен быть остановлен.
pub fn run(preset_id: &str, base: &Settings, mut on_progress: impl FnMut(&Progress)) -> Result<Outcome, String> {
    let preset = crate::base::presets().into_iter().find(|p| p.id == preset_id).ok_or("неизвестный пресет")?;
    let urls = test_urls(preset_id);
    if urls.is_empty() {
        return Err("для этого пресета нет адресов проверки".into());
    }
    let names: std::collections::HashMap<String, String> =
        crate::base::strategies().into_iter().map(|s| (s.id, s.name)).collect();

    #[cfg(windows)]
    let iface = platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    let iface: Option<String> = None;

    let total = preset.strategies.len() + 1;
    let mut results = Vec::new();

    let (base_ok, base_ms, _) = check_all(&urls, iface.as_deref());
    let p = Progress {
        preset: preset_id.into(), strategy: "baseline".into(), strategy_name: "Без обхода".into(),
        index: 0, total, ok: base_ok, of: urls.len(), ms: base_ms, done: false,
    };
    on_progress(&p);
    results.push(p);

    for (i, sid) in preset.strategies.iter().enumerate() {
        let s = Settings {
            enabled_presets: vec![preset_id.to_string()],
            strategy_override: vec![(preset_id.to_string(), sid.clone())],
            custom_domains: base.custom_domains.clone(),
            excluded_domains: base.excluded_domains.clone(),
            random_fakes: base.random_fakes,
            block_quic: base.block_quic,
            ..Default::default()
        };
        let cfg = RunConfig::from_settings(&s);
        let filter: Filter = cfg.filter();
        let io = platform::open_default(&filter).map_err(|e| e.to_string())?;
        let mut engine = Engine::default();
        engine.start(cfg, io);
        std::thread::sleep(Duration::from_millis(250));
        let (ok, ms, _) = check_all(&urls, iface.as_deref());
        engine.stop();
        let p = Progress {
            preset: preset_id.into(), strategy: sid.clone(), strategy_name: names.get(sid).cloned().unwrap_or_default(),
            index: i + 1, total, ok, of: urls.len(), ms, done: false,
        };
        on_progress(&p);
        results.push(p);
        // Всё открылось быстро — дальше можно не искать.
        if ok == urls.len() && ms < 3000 {
            break;
        }
    }

    let best = results
        .iter()
        .filter(|r| r.strategy != "baseline" && r.ok > base_ok)
        .max_by(|a, b| a.ok.cmp(&b.ok).then(b.ms.cmp(&a.ms)))
        .map(|r| (r.strategy.clone(), r.strategy_name.clone()));
    Ok(Outcome {
        preset: preset_id.into(),
        baseline_ok: base_ok,
        of: urls.len(),
        best_name: best.as_ref().map(|b| b.1.clone()),
        best: best.map(|b| b.0),
        results,
        direct_ip: iface,
    })
}

// ---------------------------------------------------------------- диагностика

#[derive(Serialize, Clone, Debug)]
pub struct Diagnosis {
    pub host: String,
    pub ips: Vec<String>,
    /// "ok" | "dns" | "ip" | "dpi" | "other"
    pub verdict: String,
    pub message: String,
    pub direct: Option<Check>,
    /// Проверка шла напрямую через физический адаптер (мимо VPN).
    pub direct_path: bool,
}

/// Почему сайт не открывается: DNS, блокировка по IP или DPI.
pub fn diagnose(host: &str) -> Diagnosis {
    use std::net::ToSocketAddrs;
    let host = host.trim().trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or("").to_string();
    #[cfg(windows)]
    let iface = platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    let iface: Option<String> = None;

    let ips: Vec<String> = (host.as_str(), 443)
        .to_socket_addrs()
        .map(|a| a.filter(|s| s.is_ipv4()).map(|s| s.ip().to_string()).collect())
        .unwrap_or_default();
    let mk = |verdict: &str, message: String, direct: Option<Check>| Diagnosis {
        host: host.clone(), ips: ips.clone(), verdict: verdict.into(), message, direct, direct_path: iface.is_some(),
    };
    if host.is_empty() {
        return mk("other", "Введите адрес сайта".into(), None);
    }
    if ips.is_empty() {
        return mk("dns", "Не удалось узнать адрес сайта (DNS). Проверьте имя сайта.".into(), None);
    }
    if ips.iter().any(|ip| ip.starts_with("198.18.") || ip.starts_with("198.19.")) {
        return mk("other", "Адрес выдан VPN-клиентом (fake-IP) — сайт идёт через VPN, проверить его напрямую нельзя.".into(), None);
    }
    let bogus = ips.iter().any(|ip| ip.starts_with("0.") || ip.starts_with("127.") || ip.starts_with("10.") || ip == "195.208.4.1");
    if bogus {
        return mk("dns", format!("DNS вернул подменённый адрес ({}). Это DNS-блокировка — включите «Безопасный DNS» в браузере или смените DNS в Windows на 1.1.1.1.", ips[0]), None);
    }
    // TCP: просто соединиться с 443 портом.
    if !tcp_connects(&ips[0], iface.as_deref()) {
        return mk("ip", format!("Сервер {} недоступен даже по TCP — похоже на блокировку по IP. Обход DPI тут не поможет, нужен VPN/прокси.", ips[0]), None);
    }
    let https = curl_check(&format!("https://{host}/"), iface.as_deref(), 8);
    if https.ok {
        return mk("ok", format!("Сайт открывается (HTTP {}, {} мс).", https.code, https.ms), Some(https));
    }
    mk(
        "dpi",
        "Соединение есть, но рукопожатие TLS обрывается — это блокировка DPI. Добавьте сайт в «Свои домены» и подберите стратегию.".into(),
        Some(https),
    )
}
