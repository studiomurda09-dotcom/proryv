//! Проверка режима разработчика вживую: devtest.exe <IP прямого адаптера>
//! Строка zapret → своя стратегия → проверка на YouTube → движок с журналом.
//! Нужны права администратора и PRORYV_WINDIVERT_DIR.

use std::{process::Command, sync::{Arc, Mutex}, time::Duration};

use proryv_core::{base, config::{CustomStrategy, Settings}, engine::{Engine, RunConfig, TraceBuf}, platform, zapret};

fn main() {
    let iface = std::env::args().nth(1).expect("IP адаптера");
    let line = "--filter-tcp=443 --hostlist=list-google.txt --dpi-desync=fake,multisplit --dpi-desync-split-seqovl=681 --dpi-desync-split-pos=1 --dpi-desync-fooling=ts --dpi-desync-repeats=8 --dpi-desync-split-seqovl-pattern=tls_clienthello_www_google_com.bin --dpi-desync-fake-tls=tls_clienthello_www_google_com.bin";
    let parsed = zapret::parse(line).expect("разбор");
    let mut s = parsed[0].strategy.clone();
    s.name = "Моя ALT11".into();
    println!("разобрано: {} → {:?}", parsed[0].label, parsed[0].recognized);
    let custom = vec![CustomStrategy { strategy: s.clone(), zapret: parsed[0].zapret.clone(), enabled: true, result: None }];
    base::set_custom(&custom);
    let yt = base::presets().into_iter().find(|p| p.id == "youtube").unwrap();
    println!("в пресете YouTube последняя стратегия: {}", yt.strategies.last().unwrap());

    let settings = Settings { custom_strategies: custom, ..Default::default() };
    match proryv_core::autotune::check_one("youtube", &s.id, &settings) {
        Ok((ok, of)) => println!("проверка «Моя ALT11» на YouTube: {ok}/{of}"),
        Err(e) => println!("проверка: ошибка {e}"),
    }

    let run = Settings {
        enabled_presets: vec!["youtube".into()],
        strategy_override: vec![("youtube".into(), s.id.clone())],
        ..settings
    };
    let cfg = RunConfig::from_settings(&run);
    let io = platform::open_default(&cfg.filter()).expect("WinDivert");
    let trace = Arc::new(Mutex::new(TraceBuf::default()));
    let mut engine = Engine::default();
    engine.trace = Some(trace.clone());
    engine.start(cfg, io);
    std::thread::sleep(Duration::from_millis(300));
    for url in ["https://www.youtube.com/", "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"] {
        let out = Command::new("curl.exe").args(["-s", "-o", "NUL", "-m", "7", "--interface", &iface, "-w", "%{http_code}", url]).output().unwrap();
        println!("{url} → {}", String::from_utf8_lossy(&out.stdout));
    }
    engine.stop();
    let (next, lines) = trace.lock().unwrap().since(0);
    println!("журнал ({next} строк):");
    for l in lines {
        println!("  {l}");
    }
}
