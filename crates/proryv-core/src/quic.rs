//! Настоящий QUIC v1 Initial-пакет (RFC 9000/9001) с ClientHello к «безобидному» сайту.
//! Используется как UDP-фейк: DPI расшифровывает Initial (ключи выводятся из DCID),
//! видит обычный QUIC к google.com и пропускает поток. Так делает zapret
//! (ACTIVE_DISCORD_UDP.bin — тоже QUIC Initial), но пакет мы собираем сами.

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use aes_gcm::{aead::AeadInPlace, Aes128Gcm, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::tls;

const INITIAL_SALT: [u8; 20] = [
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a,
];
pub const QUIC_INITIAL_LEN: usize = 1200;

fn expand_label(secret: &[u8], label: &str, len: usize) -> Vec<u8> {
    let full = format!("tls13 {label}");
    let mut info = Vec::with_capacity(4 + full.len());
    info.extend_from_slice(&(len as u16).to_be_bytes());
    info.push(full.len() as u8);
    info.extend_from_slice(full.as_bytes());
    info.push(0);
    let hk = Hkdf::<Sha256>::from_prk(secret).expect("prk");
    let mut out = vec![0u8; len];
    hk.expand(&info, &mut out).expect("expand");
    out
}

/// Ключи клиента для Initial: (key, iv, hp).
pub fn client_initial_keys(dcid: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (prk, _) = Hkdf::<Sha256>::extract(Some(&INITIAL_SALT), dcid);
    let client = expand_label(&prk, "client in", 32);
    (expand_label(&client, "quic key", 16), expand_label(&client, "quic iv", 12), expand_label(&client, "quic hp", 16))
}

fn hp_mask(hp: &[u8], sample: &[u8]) -> [u8; 16] {
    let cipher = Aes128::new_from_slice(hp).expect("hp key");
    let mut block = aes::Block::clone_from_slice(&sample[..16]);
    cipher.encrypt_block(&mut block);
    block.into()
}

fn varint(v: u64, out: &mut Vec<u8>) {
    if v < 64 {
        out.push(v as u8);
    } else if v < 16384 {
        out.extend_from_slice(&((v as u16) | 0x4000).to_be_bytes());
    } else {
        out.extend_from_slice(&((v as u32) | 0x8000_0000).to_be_bytes());
    }
}

/// Собрать зашифрованный QUIC Initial ровно на 1200 байт.
/// `rnd` — 64 случайных байта (для DCID/SCID, random и ключа в ClientHello).
pub fn build_initial(sni: &str, rnd: &[u8; 64]) -> Vec<u8> {
    let dcid = &rnd[..8];
    let scid = &rnd[8..16];
    let (key, iv, hp) = client_initial_keys(dcid);

    // ClientHello без 5-байтового заголовка TLS-записи — в QUIC он идёт в CRYPTO-кадре.
    let hello = tls::build_realistic_hello(sni, rnd, Some(&[]), 0);
    let hs = &hello[5..];

    let pn_len = 4usize;
    let mut header = vec![0xc0 | (pn_len as u8 - 1)];
    header.extend_from_slice(&1u32.to_be_bytes());
    header.push(dcid.len() as u8);
    header.extend_from_slice(dcid);
    header.push(scid.len() as u8);
    header.extend_from_slice(scid);
    header.push(0); // token length
    // Длина: (pn + полезная нагрузка + тег 16) — подгоняем под 1200 байт всего пакета.
    let length_field_len = 2;
    let payload_total = QUIC_INITIAL_LEN - header.len() - length_field_len;
    let plain_len = payload_total - pn_len - 16;
    varint(payload_total as u64, &mut header);
    let pn_offset = header.len();
    header.extend_from_slice(&0u32.to_be_bytes()); // packet number 0

    let mut plain = Vec::with_capacity(plain_len);
    plain.push(0x06); // CRYPTO
    varint(0, &mut plain);
    varint(hs.len() as u64, &mut plain);
    plain.extend_from_slice(hs);
    plain.resize(plain_len, 0); // PADDING

    let cipher = Aes128Gcm::new_from_slice(&key).expect("aead key");
    let nonce = Nonce::clone_from_slice(&iv); // pn = 0 → nonce = iv
    cipher.encrypt_in_place(&nonce, &header, &mut plain).expect("encrypt");

    let mut pkt = header;
    pkt.extend_from_slice(&plain);
    let sample = &pkt[pn_offset + 4..pn_offset + 20];
    let mask = hp_mask(&hp, sample);
    pkt[0] ^= mask[0] & 0x0f;
    for i in 0..pn_len {
        pkt[pn_offset + i] ^= mask[1 + i];
    }
    debug_assert_eq!(pkt.len(), QUIC_INITIAL_LEN);
    pkt
}

/// Расшифровать Initial (для тестов и диагностики) → CRYPTO-данные.
pub fn decrypt_initial(pkt: &[u8]) -> Option<Vec<u8>> {
    if pkt.len() < 30 || pkt[0] & 0x80 == 0 || pkt[1..5] != [0, 0, 0, 1] {
        return None;
    }
    let mut i = 5;
    let dcil = pkt[i] as usize;
    let dcid = pkt.get(i + 1..i + 1 + dcil)?.to_vec();
    i += 1 + dcil;
    i += 1 + pkt[i] as usize; // scid
    i += 1 + pkt[i] as usize; // token (короткая длина)
    let (len, n) = match pkt[i] >> 6 {
        0 => (pkt[i] as usize & 0x3f, 1),
        1 => (((pkt[i] as usize & 0x3f) << 8) | pkt[i + 1] as usize, 2),
        _ => return None,
    };
    i += n;
    let pn_offset = i;
    let (key, iv, hp) = client_initial_keys(&dcid);
    let mask = hp_mask(&hp, pkt.get(pn_offset + 4..pn_offset + 20)?);
    let mut hdr = pkt[..pn_offset + 4].to_vec();
    hdr[0] ^= mask[0] & 0x0f;
    let pn_len = (hdr[0] & 0x03) as usize + 1;
    for k in 0..pn_len {
        hdr[pn_offset + k] ^= mask[1 + k];
    }
    hdr.truncate(pn_offset + pn_len);
    let mut pn = 0u64;
    for k in 0..pn_len {
        pn = (pn << 8) | hdr[pn_offset + k] as u64;
    }
    let mut nonce = iv.clone();
    for (k, b) in pn.to_be_bytes().iter().enumerate() {
        nonce[4 + k] ^= b;
    }
    let mut body = pkt.get(pn_offset + pn_len..pn_offset + len)?.to_vec();
    Aes128Gcm::new_from_slice(&key).ok()?.decrypt_in_place(Nonce::from_slice(&nonce), &hdr, &mut body).ok()?;
    if body.first() != Some(&0x06) {
        return None;
    }
    let mut j = 1;
    j += 1; // offset 0 (1 байт)
    let (clen, n) = match body[j] >> 6 {
        0 => (body[j] as usize & 0x3f, 1),
        1 => (((body[j] as usize & 0x3f) << 8) | body[j + 1] as usize, 2),
        _ => return None,
    };
    j += n;
    body.get(j..j + clen).map(|s| s.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тестовый вектор RFC 9001, приложение A.1: ключи клиента для DCID 8394c8f03e515708.
    #[test]
    fn rfc9001_keys() {
        let dcid = [0x83, 0x94, 0xc8, 0xf0, 0x3e, 0x51, 0x57, 0x08];
        let (key, iv, hp) = client_initial_keys(&dcid);
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(&key), "1f369613dd76d5467730efcbe3b1a22d");
        assert_eq!(hex(&iv), "fa044b2f42a3fd3b46fb255c");
        assert_eq!(hex(&hp), "9f50449e04a0e810283a1e9933adedd2");
    }

    #[test]
    fn initial_roundtrip_has_sni() {
        let rnd = [0x42u8; 64];
        let pkt = build_initial("www.google.com", &rnd);
        assert_eq!(pkt.len(), QUIC_INITIAL_LEN);
        assert_eq!(&pkt[1..5], &[0, 0, 0, 1]);
        assert_eq!(pkt[0] & 0xf0, 0xc0);
        let crypto = decrypt_initial(&pkt).expect("decrypt");
        assert_eq!(crypto[0], 0x01); // ClientHello
        // Вернём 5-байтовый заголовок записи, чтобы переиспользовать парсер SNI.
        let mut rec = vec![0x16, 0x03, 0x01];
        rec.extend_from_slice(&(crypto.len() as u16).to_be_bytes());
        rec.extend_from_slice(&crypto);
        assert_eq!(tls::parse_sni(&rec).unwrap().host, "www.google.com");
    }
}
