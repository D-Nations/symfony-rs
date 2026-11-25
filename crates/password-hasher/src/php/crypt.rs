//! PHP's `crypt()` for the hash formats `password_verify()` hands to it,
//! written from the formats' public descriptions and PHP's recorded
//! answers (`tests/data/php.json`, "crypt"):
//!
//! - `$2a$`, `$2b$`, `$2x$`, `$2y$`: bcrypt (the `bcrypt` crate);
//! - `$5$`, `$6$`: SHA-crypt (the `sha-crypt` crate's core);
//! - `$1$`: MD5-crypt.
//!
//! The password is a C string: it ends at its first NUL byte.
//!
//! DES-based formats (`ab...`, `_...`) are not computed
//! ([`Crypt::Unsupported`]); `password_verify()` then gives `false`
//! (README.md, "Differences"). So does `$2x$` (bcrypt with the old
//! sign-extension bug) for a password with a byte above 0x7F. `$2a$` is
//! computed as `$2y$`: PHP's `$2a$` countermeasure changes the key only
//! when the bug would leave it unchanged although a byte above 0x7F
//! occurs, which needs each such byte at the start of a 4-byte group, and
//! UTF-8 (all a `&str` holds) never has a lone byte above 0x7F.
//!
//! Upstream: none

use md5::{Digest, Md5};

/// What `crypt($password, $setting)` gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crypt {
    /// The hash string.
    Hash(Vec<u8>),
    /// A failure string: `*0`, or `*1` when the setting is `*0`.
    Failure(&'static str),
    /// A format PHP computes but this port does not.
    Unsupported,
}

/// The crypt alphabet (`./0-9A-Za-z`), used by every format here.
const ITOA64: &[u8; 64] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// `crypt($password, $setting)`.
pub fn crypt(password: &[u8], setting: &[u8]) -> Crypt {
    let password = match password.iter().position(|&b| b == 0) {
        Some(nul) => &password[..nul],
        None => password,
    };
    let failure = || {
        Crypt::Failure(if setting.starts_with(b"*0") {
            "*1"
        } else {
            "*0"
        })
    };
    if let Some(rest) = setting.strip_prefix(b"$2") {
        return bcrypt(password, rest).unwrap_or_else(failure);
    }
    if let Some(rest) = setting.strip_prefix(b"$5$") {
        return sha_crypt(password, rest, false).unwrap_or_else(failure);
    }
    if let Some(rest) = setting.strip_prefix(b"$6$") {
        return sha_crypt(password, rest, true).unwrap_or_else(failure);
    }
    if let Some(rest) = setting.strip_prefix(b"$1$") {
        return md5_crypt(password, rest);
    }
    match setting.first() {
        // `$` with another identifier, `*`, or an empty setting.
        None | Some(b'$' | b'*') => failure(),
        // Extended (`_`) and standard DES.
        Some(_) => Crypt::Unsupported,
    }
}

/// Bcrypt, after `$2`: `<a|b|x|y>$<2-digit cost>$<22 salt characters>`;
/// anything after the salt is ignored. `None` for a malformed setting.
fn bcrypt(password: &[u8], rest: &[u8]) -> Option<Crypt> {
    let (&[minor, b'$', c1, c2, b'$'], salt) = rest.split_first_chunk::<5>()? else {
        return None;
    };
    let version = match minor {
        b'a' => bcrypt::Version::TwoA,
        b'b' => bcrypt::Version::TwoB,
        b'x' => bcrypt::Version::TwoX,
        b'y' => bcrypt::Version::TwoY,
        _ => return None,
    };
    if !c1.is_ascii_digit() || !c2.is_ascii_digit() {
        return None;
    }
    let cost = u32::from(c1 - b'0') * 10 + u32::from(c2 - b'0');
    if !(4..=31).contains(&cost) {
        return None;
    }
    let salt = decode_bcrypt_salt(salt.get(..22)?)?;
    if minor == b'x' && password.iter().any(|&b| b >= 0x80) {
        return Some(Crypt::Unsupported);
    }
    let parts = bcrypt::hash_with_salt(password, cost, salt).ok()?;
    Some(Crypt::Hash(parts.format_for_version(version).into_bytes()))
}

/// The 16 salt bytes of 22 bcrypt base64 characters (alphabet
/// `./A-Za-z0-9`); the unused low bits of the last character are dropped,
/// as the hash string then shows.
fn decode_bcrypt_salt(text: &[u8]) -> Option<[u8; 16]> {
    const BCRYPT64: &[u8; 64] = b"./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = [0u8; 16];
    let (mut acc, mut bits, mut n) = (0u32, 0u32, 0usize);
    for &c in text {
        let v = BCRYPT64.iter().position(|&a| a == c)?;
        acc = (acc << 6) | u32::try_from(v).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            if n < 16 {
                out[n] = u8::try_from((acc >> bits) & 0xff).ok()?;
                n += 1;
            }
            acc &= (1 << bits) - 1;
        }
    }
    (n == 16).then_some(out)
}

/// `strtoul($text, &$end, 10)` as SHA-crypt's `rounds=` reads it: leading
/// whitespace, a sign, digits; `None` when no digit follows (the end is
/// then the start of `text`). Overflow gives `u64::MAX`; a minus sign
/// negates modulo 2^64.
fn strtoul(text: &[u8]) -> Option<(u64, usize)> {
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
    let mut overflow = false;
    for &d in &text[i..i + digits] {
        match value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(d - b'0')))
        {
            Some(v) => value = v,
            None => overflow = true,
        }
    }
    let value = if overflow {
        u64::MAX
    } else if negative {
        value.wrapping_neg()
    } else {
        value
    };
    Some((value, i + digits))
}

/// SHA-crypt after `$5$` / `$6$`: an optional `rounds=<n>$` (1000 to
/// 999,999,999, else a failure), then up to 16 salt bytes ending at `$`.
fn sha_crypt(password: &[u8], rest: &[u8], sha512: bool) -> Option<Crypt> {
    let mut salt = rest;
    let mut rounds = None;
    if let Some(after) = rest.strip_prefix(b"rounds=") {
        let (value, end) = strtoul(after).unwrap_or((0, 0));
        if after.get(end) == Some(&b'$') {
            if !(1000..=999_999_999).contains(&value) {
                return None;
            }
            rounds = Some(u32::try_from(value).ok()?);
            salt = &after[end + 1..];
        }
    }
    let end = salt
        .iter()
        .take(16)
        .position(|&b| b == b'$')
        .unwrap_or(salt.len().min(16));
    let salt = &salt[..end];
    let params = sha_crypt::Params::new(rounds.unwrap_or(5000)).ok()?;

    let mut out = if sha512 {
        b"$6$".to_vec()
    } else {
        b"$5$".to_vec()
    };
    if let Some(rounds) = rounds {
        out.extend_from_slice(format!("rounds={rounds}$").as_bytes());
    }
    out.extend_from_slice(salt);
    out.push(b'$');
    if sha512 {
        let raw = sha_crypt::sha512_crypt(password, salt, params);
        for (a, b, c) in SHA512_ORDER {
            encode24(&mut out, raw[a], raw[b], raw[c], 4);
        }
        encode24(&mut out, 0, 0, raw[63], 2);
    } else {
        let raw = sha_crypt::sha256_crypt(password, salt, params);
        for (a, b, c) in SHA256_ORDER {
            encode24(&mut out, raw[a], raw[b], raw[c], 4);
        }
        encode24(&mut out, 0, raw[31], raw[30], 3);
    }
    Some(Crypt::Hash(out))
}

/// The byte order of SHA-512-crypt's output encoding.
const SHA512_ORDER: [(usize, usize, usize); 21] = [
    (0, 21, 42),
    (22, 43, 1),
    (44, 2, 23),
    (3, 24, 45),
    (25, 46, 4),
    (47, 5, 26),
    (6, 27, 48),
    (28, 49, 7),
    (50, 8, 29),
    (9, 30, 51),
    (31, 52, 10),
    (53, 11, 32),
    (12, 33, 54),
    (34, 55, 13),
    (56, 14, 35),
    (15, 36, 57),
    (37, 58, 16),
    (59, 17, 38),
    (18, 39, 60),
    (40, 61, 19),
    (62, 20, 41),
];

/// The byte order of SHA-256-crypt's output encoding.
const SHA256_ORDER: [(usize, usize, usize); 10] = [
    (0, 10, 20),
    (21, 1, 11),
    (12, 22, 2),
    (3, 13, 23),
    (24, 4, 14),
    (15, 25, 5),
    (6, 16, 26),
    (27, 7, 17),
    (18, 28, 8),
    (9, 19, 29),
];

/// Appends `n` crypt-alphabet characters of the 24 bits `b2 b1 b0`, low
/// bits first.
fn encode24(out: &mut Vec<u8>, b2: u8, b1: u8, b0: u8, n: usize) {
    let mut w = (u32::from(b2) << 16) | (u32::from(b1) << 8) | u32::from(b0);
    for _ in 0..n {
        out.push(ITOA64[(w & 0x3f) as usize]);
        w >>= 6;
    }
}

/// MD5-crypt after `$1$`: up to 8 salt bytes ending at `$`.
fn md5_crypt(password: &[u8], rest: &[u8]) -> Crypt {
    let end = rest
        .iter()
        .take(8)
        .position(|&b| b == b'$')
        .unwrap_or(rest.len().min(8));
    let salt = &rest[..end];

    let alternate = Md5::new()
        .chain_update(password)
        .chain_update(salt)
        .chain_update(password)
        .finalize();
    let mut ctx = Md5::new();
    ctx.update(password);
    ctx.update(b"$1$");
    ctx.update(salt);
    let mut left = password.len();
    while left > 0 {
        let take = left.min(16);
        ctx.update(&alternate[..take]);
        left -= take;
    }
    let mut i = password.len();
    while i > 0 {
        if i & 1 == 1 {
            ctx.update([0u8]);
        } else {
            ctx.update(&password[..1]);
        }
        i >>= 1;
    }
    let mut digest = ctx.finalize();
    for round in 0..1000 {
        let mut ctx = Md5::new();
        if round & 1 == 1 {
            ctx.update(password);
        } else {
            ctx.update(digest);
        }
        if round % 3 != 0 {
            ctx.update(salt);
        }
        if round % 7 != 0 {
            ctx.update(password);
        }
        if round & 1 == 1 {
            ctx.update(digest);
        } else {
            ctx.update(password);
        }
        digest = ctx.finalize();
    }

    let mut out = b"$1$".to_vec();
    out.extend_from_slice(salt);
    out.push(b'$');
    for (a, b, c) in [(0, 6, 12), (1, 7, 13), (2, 8, 14), (3, 9, 15), (4, 10, 5)] {
        encode24(&mut out, digest[a], digest[b], digest[c], 4);
    }
    encode24(&mut out, 0, 0, digest[11], 2);
    Crypt::Hash(out)
}
