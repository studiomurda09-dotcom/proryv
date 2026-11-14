//! Поднять прокси и проверить ответ всех DC Telegram: cargo run -p proryv-tg --example tg_selftest
fn main() {
    let secret = proryv_tg::gen_secret();
    let mut p = proryv_tg::TgProxy::start(14443, &secret).expect("start");
    for dc in [1i16, 2, 3, 4, 5, -2, -4] {
        match proryv_tg::self_test(14443, &secret, dc) {
            Ok(ms) => println!("DC{dc:<3} OK  {ms} мс"),
            Err(e) => println!("DC{dc:<3} ERR {e}"),
        }
    }
    let s = p.stats();
    println!("ws={} tcp={} failed={} last_error={:?}", s.via_ws, s.via_tcp, s.failed, s.last_error);
    p.stop();
}
