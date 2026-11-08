use crate::{NetError, Result};
use md5::{Digest, Md5};
use zeroize::{Zeroize, Zeroizing};

const HIGH: [u16; 128] = [
    0x20ac, 0x0081, 0x0082, 0x0083, 0x0084, 0x2026, 0x0086, 0x0087, 0x0088, 0x0089, 0x008a, 0x008b,
    0x008c, 0x008d, 0x008e, 0x008f, 0x0090, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
    0x0098, 0x0099, 0x009a, 0x009b, 0x009c, 0x009d, 0x009e, 0x009f, 0x00a0, 0x0e01, 0x0e02, 0x0e03,
    0x0e04, 0x0e05, 0x0e06, 0x0e07, 0x0e08, 0x0e09, 0x0e0a, 0x0e0b, 0x0e0c, 0x0e0d, 0x0e0e, 0x0e0f,
    0x0e10, 0x0e11, 0x0e12, 0x0e13, 0x0e14, 0x0e15, 0x0e16, 0x0e17, 0x0e18, 0x0e19, 0x0e1a, 0x0e1b,
    0x0e1c, 0x0e1d, 0x0e1e, 0x0e1f, 0x0e20, 0x0e21, 0x0e22, 0x0e23, 0x0e24, 0x0e25, 0x0e26, 0x0e27,
    0x0e28, 0x0e29, 0x0e2a, 0x0e2b, 0x0e2c, 0x0e2d, 0x0e2e, 0x0e2f, 0x0e30, 0x0e31, 0x0e32, 0x0e33,
    0x0e34, 0x0e35, 0x0e36, 0x0e37, 0x0e38, 0x0e39, 0x0e3a, 0xf8c1, 0xf8c2, 0xf8c3, 0xf8c4, 0x0e3f,
    0x0e40, 0x0e41, 0x0e42, 0x0e43, 0x0e44, 0x0e45, 0x0e46, 0x0e47, 0x0e48, 0x0e49, 0x0e4a, 0x0e4b,
    0x0e4c, 0x0e4d, 0x0e4e, 0x0e4f, 0x0e50, 0x0e51, 0x0e52, 0x0e53, 0x0e54, 0x0e55, 0x0e56, 0x0e57,
    0x0e58, 0x0e59, 0x0e5a, 0x0e5b, 0xf8c5, 0xf8c6, 0xf8c7, 0xf8c8,
];

pub(crate) fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if (c as u32) < 128 {
                c as u8
            } else {
                HIGH.iter()
                    .position(|&n| n as u32 == c as u32)
                    .map(|n| n as u8 + 128)
                    .unwrap_or(b'?')
            }
        })
        .collect()
}
pub(crate) fn decode(b: &[u8]) -> String {
    b.iter()
        .map(|&n| {
            char::from_u32(if n < 128 {
                n as u32
            } else {
                HIGH[n as usize - 128] as u32
            })
            .unwrap()
        })
        .collect()
}

/// An opaque in-memory authorization capability for one selected account.
/// Clones retain only normalized ID and derived key in zeroizing buffers.
/// Keep one owner for fresh authentication after the previous runtime has joined;
/// clear it on disconnect, mode change, quit or failed credential authorization.
/// The raw password is zeroed in `new`. No Debug, Serialize or raw public getter.
#[derive(Clone)]
pub struct NetCredentials {
    pub(crate) id: Zeroizing<String>,
    key: Zeroizing<u64>,
}
impl NetCredentials {
    pub fn new(id: String, password: String) -> Result<Self> {
        let id = Zeroizing::new(id);
        let password = Zeroizing::new(password);
        let normalized_id = Zeroizing::new(id.trim().to_owned());
        let normalized_password = Zeroizing::new(password.trim_end_matches(' ').to_owned());
        if normalized_id.is_empty()
            || normalized_id.len() > 12
            || !normalized_id
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) && !b",|\"".contains(&b))
            || normalized_password.is_empty()
            || normalized_password.len() > 18
            || !normalized_password
                .bytes()
                .all(|b| (0x20..=0x7e).contains(&b))
        {
            return Err(NetError::InvalidCredentials);
        }
        Ok(Self {
            id: normalized_id,
            key: Zeroizing::new(derive(&normalized_password)),
        })
    }
    pub(crate) fn bits(&self) -> u64 {
        *self.key
    }
    pub(crate) fn decimal(&self) -> Zeroizing<String> {
        Zeroizing::new((*self.key as i64).to_string())
    }
}
fn derive(password: &str) -> u64 {
    let mut normalized = Zeroizing::new(
        password
            .chars()
            .take(18)
            .collect::<String>()
            .trim_end_matches(' ')
            .to_ascii_lowercase(),
    );
    let mut digest = Md5::digest(normalized.as_bytes());
    let key = u64::from_be_bytes(digest[4..12].try_into().unwrap());
    digest.as_mut_slice().zeroize();
    normalized.zeroize();
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Reader, Writer};
    use sha2::Sha256;
    #[test]
    fn exact_custom_table_and_all_bytes_round_trip() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(encode(&decode(&bytes)), bytes);
        let table: Vec<u8> = decode(&bytes)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&table)),
            "32f78caea158a0dd47a634082c15619c1f57303152fc76dfff287575d88ee8c8"
        );
        assert_eq!(encode("🙂漢"), b"??");
    }
    #[test]
    fn fixed_width_and_terminator_contract() {
        for n in [16, 19, 20, 33, 256] {
            let mut w = Writer::new();
            w.fixed(&"A".repeat(n - 1), n).unwrap();
            assert_eq!(w.0.len(), n);
            assert!(Writer::new().fixed(&"A".repeat(n), n).is_err());
            assert_eq!(Reader::new(&w.0).fixed(n).unwrap(), "A".repeat(n - 1));
            assert!(Reader::new(&w.0[..n - 1]).fixed(n).is_err());
        }
        assert!(Reader::new(b"no nul").cstr().is_err());
    }
    #[test]
    fn synthetic_password_and_signed_key() {
        let c = NetCredentials::new("  ExampleId  ".into(), "ExampleOnlyA1   ".into()).unwrap();
        assert_eq!(&*c.id, "ExampleId");
        assert_eq!(&*c.decimal(), "-2896457993852450596");
        assert_eq!(
            c.bits().to_le_bytes(),
            [0xdc, 0x88, 0x95, 0xf9, 0xcc, 0xb6, 0xcd, 0xd7]
        );
        assert_eq!(derive(" A "), derive(" a"));
        assert_ne!(derive(" a"), derive("a"));
        assert_eq!(derive("123456789012345678x"), derive("123456789012345678y"));
        for n in [0u64, i64::MAX as u64, 1 << 63, u64::MAX] {
            assert_eq!((n as i64).to_string().parse::<i64>().unwrap() as u64, n);
        }
        for (id, pw) in [
            ("bad,id", "a"),
            ("a", "漢"),
            ("a", ""),
            ("a", "1234567890123456789"),
        ] {
            assert!(NetCredentials::new(id.into(), pw.into()).is_err());
        }
    }
}
