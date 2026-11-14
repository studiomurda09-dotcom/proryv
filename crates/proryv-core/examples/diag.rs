//! Диагностика сайтов из консоли: diag.exe youtube.com discord.com ...
fn main() {
    #[cfg(windows)]
    println!("direct ip: {:?}", proryv_core::platform::windivert::direct_source_ip());
    for h in std::env::args().skip(1) {
        let d = proryv_core::autotune::diagnose(&h);
        println!("{:<22} {:<5} {}", d.host, d.verdict, d.message);
    }
}
