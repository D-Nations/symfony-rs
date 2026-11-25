//! PHP's `hash()`, `hash_pbkdf2()` and `hash_equals()` for the digest
//! algorithms this port supports, written from PHP's documented and
//! recorded behaviour (`tests/data/php.json`).
//!
//! PHP's `hash_algos()` lists 60 algorithms. Only the MD5, SHA-1 and SHA-2
//! ones are here; [`HashAlgorithm::from_name`] gives `None` for the others,
//! and the hashers then report them as not supported (README.md,
//! "Differences").
//!
//! Upstream: none

use hmac::{Hmac, KeyInit, Mac};
use sha2::Digest;
use subtle::ConstantTimeEq;

/// A digest algorithm of `hash_algos()` that this port computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512_224,
    Sha512_256,
    Sha512,
}

impl HashAlgorithm {
    /// Every algorithm, in `hash_algos()` order.
    pub const ALL: [Self; 8] = [
        Self::Md5,
        Self::Sha1,
        Self::Sha224,
        Self::Sha256,
        Self::Sha384,
        Self::Sha512_224,
        Self::Sha512_256,
        Self::Sha512,
    ];

    /// The algorithm's name in `hash_algos()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha224 => "sha224",
            Self::Sha256 => "sha256",
            Self::Sha384 => "sha384",
            Self::Sha512_224 => "sha512/224",
            Self::Sha512_256 => "sha512/256",
            Self::Sha512 => "sha512",
        }
    }

    /// The algorithm `name` gives, compared as `in_array($name,
    /// hash_algos(), true)` does (case-sensitive); `None` for an
    /// algorithm this port does not compute.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }

    /// `hash($algo, $data, true)`.
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Md5 => md5::Md5::digest(data).to_vec(),
            Self::Sha1 => sha1::Sha1::digest(data).to_vec(),
            Self::Sha224 => sha2::Sha224::digest(data).to_vec(),
            Self::Sha256 => sha2::Sha256::digest(data).to_vec(),
            Self::Sha384 => sha2::Sha384::digest(data).to_vec(),
            Self::Sha512_224 => sha2::Sha512_224::digest(data).to_vec(),
            Self::Sha512_256 => sha2::Sha512_256::digest(data).to_vec(),
            Self::Sha512 => sha2::Sha512::digest(data).to_vec(),
        }
    }

    /// The digest length in bytes.
    pub fn output_len(self) -> usize {
        match self {
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha224 | Self::Sha512_224 => 28,
            Self::Sha256 | Self::Sha512_256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    /// `hash_pbkdf2($algo, $password, $salt, $iterations, $length, true)`
    /// for `$iterations > 0` and `$length >= 0` (the caller checks them):
    /// `$length` bytes of RFC 8018's PBKDF2 with HMAC, or one digest's
    /// length when `$length` is 0.
    pub fn pbkdf2(self, password: &[u8], salt: &[u8], iterations: u64, length: usize) -> Vec<u8> {
        let length = if length == 0 {
            self.output_len()
        } else {
            length
        };
        match self {
            Self::Md5 => pbkdf2::<Hmac<md5::Md5>>(password, salt, iterations, length),
            Self::Sha1 => pbkdf2::<Hmac<sha1::Sha1>>(password, salt, iterations, length),
            Self::Sha224 => pbkdf2::<Hmac<sha2::Sha224>>(password, salt, iterations, length),
            Self::Sha256 => pbkdf2::<Hmac<sha2::Sha256>>(password, salt, iterations, length),
            Self::Sha384 => pbkdf2::<Hmac<sha2::Sha384>>(password, salt, iterations, length),
            Self::Sha512_224 => {
                pbkdf2::<Hmac<sha2::Sha512_224>>(password, salt, iterations, length)
            }
            Self::Sha512_256 => {
                pbkdf2::<Hmac<sha2::Sha512_256>>(password, salt, iterations, length)
            }
            Self::Sha512 => pbkdf2::<Hmac<sha2::Sha512>>(password, salt, iterations, length),
        }
    }
}

/// PBKDF2 with the HMAC `M`: `length` bytes.
fn pbkdf2<M: Mac + KeyInit + Clone>(
    password: &[u8],
    salt: &[u8],
    iterations: u64,
    length: usize,
) -> Vec<u8> {
    // HMAC takes keys of any length.
    let Ok(keyed) = <M as KeyInit>::new_from_slice(password) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(length);
    let mut block: u32 = 1;
    while out.len() < length {
        let mut mac = keyed.clone();
        mac.update(salt);
        mac.update(&block.to_be_bytes());
        let mut u = mac.finalize().into_bytes();
        let mut t = u.clone();
        for _ in 1..iterations {
            let mut mac = keyed.clone();
            mac.update(&u);
            u = mac.finalize().into_bytes();
            for (t, u) in t.iter_mut().zip(u.iter()) {
                *t ^= u;
            }
        }
        let take = t.len().min(length - out.len());
        out.extend_from_slice(&t[..take]);
        block = block.wrapping_add(1);
    }
    out
}

/// `hash_equals($known, $user)`: equal lengths and equal bytes, compared
/// in constant time for a given length.
pub fn hash_equals(known: &[u8], user: &[u8]) -> bool {
    known.len() == user.len() && bool::from(known.ct_eq(user))
}

/// `bin2hex($bytes)`.
pub fn bin2hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 15)]));
    }
    out
}

/// `base64_encode($bytes)`.
pub fn base64_encode(bytes: &[u8]) -> String {
    php_value::base64::encode(bytes)
}
