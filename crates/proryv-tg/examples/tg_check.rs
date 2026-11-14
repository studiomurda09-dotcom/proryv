//! Проверить уже запущенный прокси: tg_check <порт> <секрет>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a[1].parse().unwrap();
    for dc in [2i16, 4, 1] {
        match proryv_tg::self_test(port, &a[2], dc) {
            Ok(ms) => println!("DC{dc} OK {ms} мс"),
            Err(e) => println!("DC{dc} ERR {e}"),
        }
    }
}
