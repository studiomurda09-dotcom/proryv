//! Диагностика: запускает движок на N секунд и пишет журнал событий в файл.
//!   trace_run.exe <папка_с_WinDivert> <файл_журнала> <секунды>
//! Нужны права администратора. Ждёт, пока закроется proryv.exe, чтобы не было двух перехватчиков.

#[cfg(windows)]
fn main() {
    use std::{fmt::Write as _, process::Command, sync::{Arc, Mutex}, time::{Duration, Instant}};

    use proryv_core::{
        config::Settings,
        engine::{Engine, RunConfig},
        platform::{windivert::WinDivert, PacketIo},
    };

    let a: Vec<String> = std::env::args().collect();
    let dll = std::path::PathBuf::from(&a[1]).join("WinDivert.dll");
    let path = a[2].clone();
    let secs: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(120);
    let write = |s: &str| {
        let _ = std::fs::write(&path, s);
    };

    let wait_start = Instant::now();
    loop {
        let out = Command::new("tasklist").args(["/FI", "IMAGENAME eq proryv.exe", "/NH"]).output();
        let busy = out.map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("proryv.exe")).unwrap_or(false);
        if !busy {
            break;
        }
        write(&format!("WAITING: закройте Proryv ({}s)\n", wait_start.elapsed().as_secs()));
        if wait_start.elapsed() > Duration::from_secs(300) {
            write("ABORT: Proryv не закрыт\n");
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    let s = Settings {
        enabled_presets: vec!["youtube".into(), "discord".into(), "general".into()],
        ..Default::default()
    };
    let cfg = RunConfig::from_settings(&s);
    let io: Arc<dyn PacketIo> = match WinDivert::open_with(&dll, &cfg.filter()) {
        Ok(w) => Arc::new(w),
        Err(e) => {
            write(&format!("ERROR {e}\n"));
            return;
        }
    };
    let trace = Arc::new(Mutex::new(proryv_core::engine::TraceBuf::default()));
    let mut engine = Engine::default();
    engine.trace = Some(trace.clone());
    engine.start(cfg, io);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_secs(1));
        let st = engine.status();
        let mut s = format!(
            "RUNNING {}s state={:?} conn={} desync={} fakes={} err={:?}\n",
            t0.elapsed().as_secs(), st.state, st.stats.connections, st.stats.desynced, st.stats.fakes_sent, st.message
        );
        for l in trace.lock().unwrap().lines.iter() {
            let _ = writeln!(s, "{l}");
        }
        write(&s);
    }
    engine.stop();
    let mut s = std::fs::read_to_string(&path).unwrap_or_default();
    s.push_str("DONE\n");
    write(&s);
}

#[cfg(not(windows))]
fn main() {}
