//! Показать VPN-туннели, которые Proryv не будет трогать.
fn main() {
    #[cfg(windows)]
    for (i, d) in proryv_core::platform::windivert::tunnel_interfaces() {
        println!("{i}\t{d}");
    }
}
