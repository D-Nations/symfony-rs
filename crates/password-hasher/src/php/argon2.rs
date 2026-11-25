//! Argon2 hash strings as libsodium's `crypto_pwhash_str*()` (PHP's
//! `sodium_crypto_pwhash_str*()`) and `password_hash()` write and read
//! them, written from the PHC string format and PHP's recorded answers
//! (`tests/data/php.json`, `argon2_strings`, `needs_rehash`); the hashing
//! itself is the `RustCrypto` `argon2` crate.
//!
//! `$argon2id$v=19$m=<KiB>,t=<passes>,p=<lanes>$<salt>$<hash>`, with the
//! salt and hash in standard base64 without padding.
//!
//! Upstream: none

use argon2::{Algorithm, Argon2, Params, Version};

use crate::{
    exception::{Class, Error},
    php::hash::hash_equals,
};

/// The two variants PHP hashes with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Argon2i,
    Argon2id,
}

impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Self::Argon2i => "argon2i",
            Self::Argon2id => "argon2id",
        }
    }

    fn algorithm(self) -> Algorithm {
        match self {
            Self::Argon2i => Algorithm::Argon2i,
            Self::Argon2id => Algorithm::Argon2id,
        }
    }
}

/// A decoded hash string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub variant: Variant,
    pub m_cost: u32,
    pub t_cost: u32,
    pub lanes: u32,
    pub salt: Vec<u8>,
    pub hash: Vec<u8>,
}

/// Random bytes from the OS.
pub fn random_bytes<const N: usize>() -> Result<[u8; N], Error> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| {
        Error::new(
            Class::Error,
            format!("Could not gather sufficient random data: {e}"),
        )
    })?;
    Ok(bytes)
}

/// Raw Argon2 (version 1.3) output of `out_len` bytes; `None` for
/// parameters Argon2 refuses.
pub fn compute(
    variant: Variant,
    m_cost: u32,
    t_cost: u32,
    lanes: u32,
    password: &[u8],
    salt: &[u8],
    out_len: usize,
) -> Option<Vec<u8>> {
    let params = Params::new(m_cost, t_cost, lanes, Some(out_len)).ok()?;
    let argon2 = Argon2::new(variant.algorithm(), Version::V0x13, params);
    let mut out = vec![0u8; out_len];
    argon2.hash_password_into(password, salt, &mut out).ok()?;
    Some(out)
}

/// Writes the hash string.
pub fn encode(
    variant: Variant,
    m_cost: u32,
    t_cost: u32,
    lanes: u32,
    salt: &[u8],
    hash: &[u8],
) -> String {
    format!(
        "${}$v=19$m={m_cost},t={t_cost},p={lanes}${}${}",
        variant.name(),
        b64_encode(salt),
        b64_encode(hash)
    )
}

/// Hashes with a random 16-byte salt into a 32-byte hash, as both
/// `password_hash()` and `sodium_crypto_pwhash_str()` do.
pub fn hash(
    variant: Variant,
    m_cost: u32,
    t_cost: u32,
    lanes: u32,
    password: &[u8],
) -> Result<Option<String>, Error> {
    let salt = random_bytes::<16>()?;
    Ok(compute(variant, m_cost, t_cost, lanes, password, &salt, 32)
        .map(|hash| encode(variant, m_cost, t_cost, lanes, &salt, &hash)))
}

fn b64_encode(bytes: &[u8]) -> String {
    let mut text = php_value::base64::encode(bytes);
    while text.ends_with('=') {
        text.pop();
    }
    text
}

/// Standard base64 without padding; `None` for another character, a
/// length of 1 modulo 4 or unused bits that are not zero.
fn b64_decode(text: &[u8]) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in text {
        let v = ALPHABET.iter().position(|&a| a == c)?;
        acc = (acc << 6) | u32::try_from(v).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
            acc &= (1 << bits) - 1;
        }
    }
    (bits < 6 && acc == 0).then_some(out)
}

/// A decimal without sign or leading zero, at most `u32::MAX`, and the
/// rest of the text.
fn decimal(text: &[u8]) -> Option<(u32, &[u8])> {
    let digits = text.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || (digits > 1 && text[0] == b'0') {
        return None;
    }
    let value = std::str::from_utf8(&text[..digits]).ok()?.parse().ok()?;
    Some((value, &text[digits..]))
}

/// libsodium's strict reading: the whole string as above, version 19,
/// then Argon2's limits (salt of 8 bytes or more, hash of 16 or more, one
/// pass or more, one lane or more and 8 KiB per lane or more).
pub fn decode(hash: &[u8]) -> Option<Decoded> {
    let (variant, rest) = match hash.strip_prefix(b"$argon2id$") {
        Some(rest) => (Variant::Argon2id, rest),
        None => (Variant::Argon2i, hash.strip_prefix(b"$argon2i$")?),
    };
    let rest = rest.strip_prefix(b"v=")?;
    let (version, rest) = decimal(rest)?;
    let rest = rest.strip_prefix(b"$m=").filter(|_| version == 19)?;
    let (m_cost, rest) = decimal(rest)?;
    let rest = rest.strip_prefix(b",t=")?;
    let (t_cost, rest) = decimal(rest)?;
    let rest = rest.strip_prefix(b",p=")?;
    let (lanes, rest) = decimal(rest)?;
    let rest = rest.strip_prefix(b"$")?;
    let split = rest.iter().position(|&b| b == b'$')?;
    let salt = b64_decode(&rest[..split])?;
    let hash = b64_decode(&rest[split + 1..])?;
    let valid = salt.len() >= 8
        && hash.len() >= 16
        && t_cost >= 1
        && lanes >= 1
        && u64::from(m_cost) >= 8 * u64::from(lanes);
    valid.then_some(Decoded {
        variant,
        m_cost,
        t_cost,
        lanes,
        salt,
        hash,
    })
}

/// `sodium_crypto_pwhash_str_verify($hash, $password)`, for `$argon2i$`
/// and `$argon2id$` strings.
pub fn sodium_verify(hash: &str, password: &[u8]) -> bool {
    let Some(d) = decode(hash.as_bytes()) else {
        return false;
    };
    compute(
        d.variant,
        d.m_cost,
        d.t_cost,
        d.lanes,
        password,
        &d.salt,
        d.hash.len(),
    )
    .is_some_and(|out| hash_equals(&d.hash, &out))
}

/// `sodium_crypto_pwhash_str_needs_rehash($hash, $opslimit, $memlimit)`:
/// `true` unless the string reads (either variant) and has `t` equal to
/// the ops limit and `m` equal to the memory limit in KiB.
pub fn sodium_needs_rehash(hash: &str, ops_limit: u64, mem_limit: u64) -> bool {
    decode(hash.as_bytes())
        .is_none_or(|d| u64::from(d.t_cost) != ops_limit || u64::from(d.m_cost) != mem_limit / 1024)
}

/// `sodium_crypto_pwhash_str($password, $opslimit, $memlimit)`: Argon2id
/// with one lane.
pub fn sodium_hash(password: &[u8], ops_limit: u64, mem_limit: u64) -> Result<String, Error> {
    let internal = || Error::new(Class::SodiumException, "internal error");
    let t_cost = u32::try_from(ops_limit).map_err(|_| internal())?;
    // libsodium's limit is 4 TiB minus 1 KiB.
    if mem_limit > 4_398_046_510_080 {
        return Err(internal());
    }
    let m_cost = u32::try_from(mem_limit / 1024).map_err(|_| internal())?;
    hash(Variant::Argon2id, m_cost, t_cost, 1, password)?.ok_or_else(internal)
}

/// The `m`, `t` and `p` numbers `password_needs_rehash()` reads from an
/// Argon2 string: `sscanf()` of `v=%u$m=%u,t=%u,p=%u` after the variant,
/// stopping at the first mismatch, with 0 for what it did not read.
pub fn scan_options(after_variant: &[u8]) -> (u64, u64, u64) {
    /// `%u`: whitespace, a sign, digits (modulo 2^32).
    fn number(text: &[u8]) -> Option<(u64, &[u8])> {
        let mut i = text.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let negative = match text.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let digits = text[i..].iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let mut value: u64 = 0;
        for &d in &text[i..i + digits] {
            value = value.saturating_mul(10).saturating_add(u64::from(d - b'0'));
        }
        if negative {
            value = value.wrapping_neg();
        }
        Some((value, &text[i + digits..]))
    }
    let mut values = [0u64; 3];
    let mut rest = after_variant;
    for (i, literal) in [&b"v="[..], b"$m=", b",t=", b",p="].into_iter().enumerate() {
        let Some(after) = rest.strip_prefix(literal) else {
            break;
        };
        let Some((value, after)) = number(after) else {
            break;
        };
        if i > 0 {
            values[i - 1] = value;
        }
        rest = after;
    }
    (values[0], values[1], values[2])
}
