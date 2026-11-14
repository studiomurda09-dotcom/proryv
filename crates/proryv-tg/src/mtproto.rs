//! Обфускация MTProto (obfuscated2) и разбиение потока на транспортные пакеты.
//! Схема повторяет tg-ws-proxy (Flowseal, MIT): расшифровываем данные клиента
//! ключом с секретом прокси и заново шифруем стандартной обфускацией к Telegram.

use aes::Aes256;
use ctr::cipher::{KeyIvInit, StreamCipher};
use sha2::{Digest, Sha256};

pub type Ctr = ctr::Ctr128BE<Aes256>;

pub const HANDSHAKE_LEN: usize = 64;
const SKIP: usize = 8;
const PREKEY: usize = 32;
const IV: usize = 16;
const TAG_POS: usize = 56;
const DC_POS: usize = 60;

pub const TAG_ABRIDGED: [u8; 4] = [0xef; 4];
pub const TAG_INTERMEDIATE: [u8; 4] = [0xee; 4];
pub const TAG_SECURE: [u8; 4] = [0xdd; 4];

fn sha256(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(a);
    h.update(b);
    h.finalize().into()
}

fn ctr(key: &[u8], iv: &[u8]) -> Ctr {
    Ctr::new(key.into(), iv.into())
}

/// Результат разбора первых 64 байт от клиента.
#[derive(Debug, Clone)]
pub struct ClientInit {
    pub dc_idx: i16,
    pub tag: [u8; 4],
    /// Байты 8..56 исходного пакета (prekey + iv).
    pub prekey_iv: [u8; 48],
}

impl ClientInit {
    pub fn dc(&self) -> u16 {
        self.dc_idx.unsigned_abs()
    }
    pub fn is_media(&self) -> bool {
        self.dc_idx < 0
    }
}

/// Проверить init-пакет клиента нашим секретом.
pub fn parse_client_init(h: &[u8; HANDSHAKE_LEN], secret: &[u8; 16]) -> Option<ClientInit> {
    let prekey = &h[SKIP..SKIP + PREKEY];
    let iv = &h[SKIP + PREKEY..SKIP + PREKEY + IV];
    let mut dec = *h;
    ctr(&sha256(prekey, secret), iv).apply_keystream(&mut dec);
    let tag: [u8; 4] = dec[TAG_POS..TAG_POS + 4].try_into().ok()?;
    if ![TAG_ABRIDGED, TAG_INTERMEDIATE, TAG_SECURE].contains(&tag) {
        return None;
    }
    let dc_idx = i16::from_le_bytes([dec[DC_POS], dec[DC_POS + 1]]);
    let mut prekey_iv = [0u8; 48];
    prekey_iv.copy_from_slice(&h[SKIP..SKIP + PREKEY + IV]);
    Some(ClientInit { dc_idx, tag, prekey_iv })
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("random");
    b
}

/// Init-пакет к Telegram: стандартная обфускация без секрета.
pub fn relay_init(tag: [u8; 4], dc_idx: i16) -> [u8; HANDSHAKE_LEN] {
    const RESERVED: [[u8; 4]; 7] = [*b"HEAD", *b"POST", *b"GET ", [0xee; 4], [0xdd; 4], [0x16, 0x03, 0x01, 0x02], *b"OPTI"];
    let rnd: [u8; HANDSHAKE_LEN] = loop {
        let r = random::<HANDSHAKE_LEN>();
        let start: [u8; 4] = r[..4].try_into().unwrap();
        if r[0] != 0xef && !RESERVED.contains(&start) && r[4..8] != [0; 4] {
            break r;
        }
    };
    let mut enc = rnd;
    ctr(&rnd[SKIP..SKIP + PREKEY], &rnd[SKIP + PREKEY..SKIP + PREKEY + IV]).apply_keystream(&mut enc);
    let extra = random::<2>();
    let dc = dc_idx.to_le_bytes();
    let tail = [tag[0], tag[1], tag[2], tag[3], dc[0], dc[1], extra[0], extra[1]];
    let mut out = rnd;
    for i in 0..8 {
        let ks = enc[TAG_POS + i] ^ rnd[TAG_POS + i];
        out[TAG_POS + i] = tail[i] ^ ks;
    }
    out
}

/// Четыре шифра соединения.
pub struct CryptoCtx {
    pub clt_dec: Ctr,
    pub clt_enc: Ctr,
    pub tg_enc: Ctr,
    pub tg_dec: Ctr,
}

pub fn crypto_ctx(client: &ClientInit, secret: &[u8; 16], relay: &[u8; HANDSHAKE_LEN]) -> CryptoCtx {
    let p = &client.prekey_iv;
    let mut clt_dec = ctr(&sha256(&p[..PREKEY], secret), &p[PREKEY..]);
    let mut skip = [0u8; HANDSHAKE_LEN];
    clt_dec.apply_keystream(&mut skip);

    let mut rev = *p;
    rev.reverse();
    let clt_enc = ctr(&sha256(&rev[..PREKEY], secret), &rev[PREKEY..]);

    let mut tg_enc = ctr(&relay[SKIP..SKIP + PREKEY], &relay[SKIP + PREKEY..SKIP + PREKEY + IV]);
    let mut skip = [0u8; HANDSHAKE_LEN];
    tg_enc.apply_keystream(&mut skip);

    let mut rrev = [0u8; 48];
    rrev.copy_from_slice(&relay[SKIP..SKIP + PREKEY + IV]);
    rrev.reverse();
    let tg_dec = ctr(&rrev[..PREKEY], &rrev[PREKEY..]);
    CryptoCtx { clt_dec, clt_enc, tg_enc, tg_dec }
}

/// Делит поток на транспортные пакеты MTProto, чтобы каждый ушёл отдельным WS-кадром.
pub struct MsgSplitter {
    tag: [u8; 4],
    cipher: Vec<u8>,
    plain: Vec<u8>,
    disabled: bool,
}

impl MsgSplitter {
    pub fn new(tag: [u8; 4]) -> Self {
        MsgSplitter { tag, cipher: Vec::new(), plain: Vec::new(), disabled: false }
    }

    /// `cipher` — уже перешифрованные к Telegram байты, `plain` — те же байты в открытом виде.
    pub fn split(&mut self, cipher: &[u8], plain: &[u8]) -> Vec<Vec<u8>> {
        if cipher.is_empty() {
            return vec![];
        }
        if self.disabled {
            return vec![cipher.to_vec()];
        }
        self.cipher.extend_from_slice(cipher);
        self.plain.extend_from_slice(plain);
        let mut parts = Vec::new();
        let mut off = 0;
        while off < self.cipher.len() {
            match self.next_len(off) {
                None => break,
                Some(0) => {
                    parts.push(self.cipher[off..].to_vec());
                    off = self.cipher.len();
                    self.disabled = true;
                }
                Some(n) => {
                    parts.push(self.cipher[off..off + n].to_vec());
                    off += n;
                }
            }
        }
        self.cipher.drain(..off);
        self.plain.drain(..off);
        parts
    }

    pub fn flush(&mut self) -> Option<Vec<u8>> {
        self.plain.clear();
        (!self.cipher.is_empty()).then(|| std::mem::take(&mut self.cipher))
    }

    /// Длина следующего пакета; None — данных не хватает; Some(0) — непонятный формат.
    fn next_len(&self, off: usize) -> Option<usize> {
        let avail = self.plain.len() - off;
        let p = &self.plain[off..];
        let (header, payload) = if self.tag == TAG_ABRIDGED {
            let first = p[0];
            if first == 0x7f || first == 0xff {
                if avail < 4 {
                    return None;
                }
                (4, (u32::from_le_bytes([p[1], p[2], p[3], 0]) as usize) * 4)
            } else {
                (1, ((first & 0x7f) as usize) * 4)
            }
        } else {
            if avail < 4 {
                return None;
            }
            (4, (u32::from_le_bytes([p[0], p[1], p[2], p[3]]) & 0x7fff_ffff) as usize)
        };
        if payload == 0 {
            return Some(0);
        }
        (avail >= header + payload).then_some(header + payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сделать init-пакет так, как это делает Telegram Desktop при подключении к прокси с секретом.
    fn client_init(secret: &[u8; 16], tag: [u8; 4], dc: i16) -> ([u8; 64], Ctr, Ctr) {
        let mut r = random::<64>();
        r[0] = 0x11;
        let key = sha256(&r[8..40], secret);
        let mut enc = ctr(&key, &r[40..56]);
        let mut ks = [0u8; 64];
        enc.apply_keystream(&mut ks); // ks = keystream (r xor 0)
        let d = dc.to_le_bytes();
        let tail = [tag[0], tag[1], tag[2], tag[3], d[0], d[1], 0, 0];
        for i in 0..8 {
            r[56 + i] = tail[i] ^ ks[56 + i];
        }
        // Шифр клиента «на отправку» уже сдвинут на 64 байта (enc). Шифр «на приём»:
        let mut rev = [0u8; 48];
        rev.copy_from_slice(&r[8..56]);
        rev.reverse();
        let dec = ctr(&sha256(&rev[..32], secret), &rev[32..]);
        (r, enc, dec)
    }

    #[test]
    fn client_init_roundtrip_and_reencrypt() {
        let secret = random::<16>();
        let (init, mut c_enc, mut c_dec) = client_init(&secret, TAG_INTERMEDIATE, -4);
        let ci = parse_client_init(&init, &secret).expect("init");
        assert_eq!((ci.dc(), ci.is_media(), ci.tag), (4, true, TAG_INTERMEDIATE));
        assert!(parse_client_init(&init, &random::<16>()).is_none(), "чужой секрет не подходит");

        let relay = relay_init(ci.tag, ci.dc_idx);
        // Telegram расшифровывает relay стандартной обфускацией (ключ без секрета).
        let mut dec = relay;
        ctr(&relay[8..40], &relay[40..56]).apply_keystream(&mut dec);
        assert_eq!(&dec[56..60], &TAG_INTERMEDIATE);
        assert_eq!(i16::from_le_bytes([dec[60], dec[61]]), -4);

        let mut ctx = crypto_ctx(&ci, &secret, &relay);
        // Клиент → прокси → Telegram
        let msg = b"\x08\x00\x00\x00hello!!!".to_vec();
        let mut wire = msg.clone();
        c_enc.apply_keystream(&mut wire);
        ctx.clt_dec.apply_keystream(&mut wire);
        assert_eq!(wire, msg);
        // Telegram → прокси → клиент
        let mut down = b"answer".to_vec();
        ctx.clt_enc.apply_keystream(&mut down);
        c_dec.apply_keystream(&mut down);
        assert_eq!(down, b"answer");
    }

    #[test]
    fn splitter_intermediate_and_abridged() {
        let mut s = MsgSplitter::new(TAG_INTERMEDIATE);
        let mut plain = Vec::new();
        plain.extend_from_slice(&4u32.to_le_bytes());
        plain.extend_from_slice(b"abcd");
        plain.extend_from_slice(&8u32.to_le_bytes());
        plain.extend_from_slice(b"12345");
        let parts = s.split(&plain, &plain);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].len(), 8);
        let parts = s.split(b"678", b"678");
        assert_eq!(parts, vec![b"\x08\x00\x00\x0012345678".to_vec()]);

        let mut a = MsgSplitter::new(TAG_ABRIDGED);
        let pkt = [2u8, 1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(a.split(&pkt, &pkt), vec![pkt.to_vec()]);
    }
}
