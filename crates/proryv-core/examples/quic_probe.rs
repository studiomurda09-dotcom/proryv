//! Отправить наш QUIC Initial на www.google.com:443 и проверить, что сервер отвечает.
use std::net::{ToSocketAddrs, UdpSocket};

fn main() {
    let mut rnd = [0u8; 64];
    for (i, b) in rnd.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(97).wrapping_add(std::process::id() as u8);
    }
    let pkt = proryv_core::quic::build_initial("www.google.com", &rnd);
    let addr = ("www.google.com", 443).to_socket_addrs().unwrap().find(|a| a.is_ipv4()).unwrap();
    let s = UdpSocket::bind("0.0.0.0:0").unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();
    s.send_to(&pkt, addr).unwrap();
    let mut buf = [0u8; 2048];
    match s.recv_from(&mut buf) {
        Ok((n, from)) => println!("ответ от {from}: {n} байт, первый байт 0x{:02x}, версия {:02x?}", buf[0], &buf[1..5]),
        Err(e) => println!("нет ответа: {e}"),
    }
}
