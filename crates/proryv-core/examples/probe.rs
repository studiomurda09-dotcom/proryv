//! Тестер стратегий: по очереди включает каждую стратегию пресета и проверяет
//! через curl, открываются ли сайты. Основа будущего автоподбора.
//!
//!   probe.exe <папка_с_WinDivert> <файл_отчёта> [пресет]
//! Нужны права администратора.

#[cfg(windows)]
fn main() {
    use std::{fmt::Write as _, process::Command, sync::Arc, time::Duration};

    use proryv_core::{
        config::Settings,
        engine::{Engine, RunConfig},
        platform::{windivert::WinDivert, PacketIo},
        presets,
    };

    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(args.get(1).expect("папка WinDivert"));
    let report = args.get(2).expect("файл отчёта").clone();
    let preset_id = args.get(3).cloned().unwrap_or_else(|| "youtube".into());
    // IP физического адаптера: запросы пойдут напрямую, мимо VPN-туннеля.
    let iface = args.get(4).cloned();

    let urls: Vec<&str> = match preset_id.as_str() {
        "discord" => vec!["https://discord.com/api/v9/experiments", "https://cdn.discordapp.com/embed/avatars/0.png", "https://gateway.discord.gg/"],
        _ => vec![
            "https://www.youtube.com/generate_204",
            "https://www.youtube.com/",
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            "https://redirector.googlevideo.com/report_mapping",
            "https://jnn-pa.googleapis.com/",
            "https://youtubei.googleapis.com/",
        ],
    };

    let mut log = String::new();
    let flush = |log: &str| {
        let _ = std::fs::write(&report, log);
    };
    let curl = |url: &str| -> String {
        let mut cmd = Command::new("curl.exe");
        cmd.args(["-s", "-o", "NUL", "-m", "7", "--connect-timeout", "5", "-w", "%{http_code} %{time_total}s"]);
        if let Some(ip) = &iface {
            cmd.args(["--interface", ip]);
        }
        let out = cmd.arg(url).output();
        match out {
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let code = o.status.code().unwrap_or(-1);
                if code == 0 { s } else { format!("FAIL(curl {code}) {s}") }
            }
            Err(e) => format!("ERR {e}"),
        }
    };
    let run_urls = |log: &mut String| -> usize {
        let mut ok = 0;
        for u in &urls {
            let r = curl(u);
            if !r.starts_with("FAIL") && !r.starts_with("000") && !r.starts_with("ERR") {
                ok += 1;
            }
            let _ = writeln!(log, "    {r:<28} {u}");
        }
        ok
    };

    let _ = writeln!(log, "=== Proryv probe, пресет {preset_id}\n\n--- без обхода");
    let base = run_urls(&mut log);
    let _ = writeln!(log, "  итого: {base}/{}", urls.len());
    flush(&log);

    let preset = presets::builtin_presets().into_iter().find(|p| p.id == preset_id).expect("пресет");
    let dll = dir.join("WinDivert.dll");
    let mut summary = Vec::new();
    for sid in &preset.strategies {
        let s = Settings {
            enabled_presets: vec![preset_id.clone()],
            strategy_override: vec![(preset_id.clone(), sid.clone())],
            ..Default::default()
        };
        let cfg = RunConfig::from_settings(&s);
        let io: Arc<dyn PacketIo> = match WinDivert::open_with(&dll, &cfg.filter()) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                let _ = writeln!(log, "\nОШИБКА открытия WinDivert: {e}");
                flush(&log);
                return;
            }
        };
        let mut engine = Engine::default();
        engine.start(cfg, io);
        std::thread::sleep(Duration::from_millis(300));
        let _ = writeln!(log, "\n--- {sid}");
        let ok = run_urls(&mut log);
        let st = engine.status();
        engine.stop();
        let _ = writeln!(
            log,
            "  итого: {ok}/{}  (соединений {}, обработано {}, фейков {}, QUIC заблокировано)",
            urls.len(), st.stats.connections, st.stats.desynced, st.stats.fakes_sent
        );
        summary.push((sid.clone(), ok));
        flush(&log);
        std::thread::sleep(Duration::from_millis(300));
    }
    let _ = writeln!(log, "\n=== СВОДКА (без обхода {base}/{})", urls.len());
    for (sid, ok) in summary {
        let _ = writeln!(log, "  {ok}/{}  {sid}", urls.len());
    }
    let _ = writeln!(log, "DONE");
    flush(&log);
}

#[cfg(not(windows))]
fn main() {
    eprintln!("только Windows");
}
