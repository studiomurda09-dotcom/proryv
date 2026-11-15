//! Настоящий автоподбор с замером времени: tune.exe [пресет]
//! Нужны права администратора и PRORYV_WINDIVERT_DIR (или WinDivert рядом с exe).

fn main() {
    let preset = std::env::args().nth(1).unwrap_or_else(|| "youtube".into());
    let start = std::time::Instant::now();
    let res = proryv_core::autotune::run(&preset, &proryv_core::config::Settings::default(), |p| {
        println!("{:>6.1} с  {:<22} {}/{}  {} мс", start.elapsed().as_secs_f32(), p.strategy, p.ok, p.of, p.ms);
    });
    match res {
        Ok(o) => println!("итог: лучшая {:?}, без обхода {}/{}, прямой адаптер {:?}, всего {:.1} с", o.best, o.baseline_ok, o.of, o.direct_ip, start.elapsed().as_secs_f32()),
        Err(e) => println!("ошибка: {e}"),
    }
}
