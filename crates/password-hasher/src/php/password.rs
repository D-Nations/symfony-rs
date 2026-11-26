//! PHP's `password_hash()`, `password_verify()` and
//! `password_needs_rehash()` for bcrypt and Argon2, written from PHP's
//! documentation and recorded answers (`tests/data/php.json`).
//!
//! Upstream: none

use crate::{
    exception::{Class, Error},
    php::{
        argon2::{self, Variant},
        crypt::{Crypt, crypt},
        hash::hash_equals,
    },
};

/// `PASSWORD_BCRYPT`.
pub const PASSWORD_BCRYPT: &str = "2y";
/// `PASSWORD_ARGON2I`.
pub const PASSWORD_ARGON2I: &str = "argon2i";
/// `PASSWORD_ARGON2ID`.
pub const PASSWORD_ARGON2ID: &str = "argon2id";

/// A `password_hash()` algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordAlgorithm {
    Bcrypt,
    Argon2i,
    Argon2id,
    /// A string PHP does not know as an algorithm.
    Other(String),
}

impl PasswordAlgorithm {
    /// The algorithm a `PASSWORD_*` value names.
    pub fn from_name(name: &str) -> Self {
        match name {
            PASSWORD_BCRYPT => Self::Bcrypt,
            PASSWORD_ARGON2I => Self::Argon2i,
            PASSWORD_ARGON2ID => Self::Argon2id,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The `PASSWORD_*` value.
    pub fn name(&self) -> &str {
        match self {
            Self::Bcrypt => PASSWORD_BCRYPT,
            Self::Argon2i => PASSWORD_ARGON2I,
            Self::Argon2id => PASSWORD_ARGON2ID,
            Self::Other(name) => name,
        }
    }
}

/// The `cost`, `time_cost`, `memory_cost` (KiB) and `threads` options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordOptions {
    pub cost: i64,
    pub time_cost: i64,
    pub memory_cost: i64,
    pub threads: i64,
}

fn value_error(message: &str) -> Error {
    Error::new(Class::ValueError, message)
}

/// `password_hash($password, $algo, $options)`.
pub fn password_hash(
    password: &[u8],
    algorithm: &PasswordAlgorithm,
    options: &PasswordOptions,
) -> Result<String, Error> {
    let variant = match algorithm {
        PasswordAlgorithm::Bcrypt => {
            let cost = u32::try_from(options.cost)
                .ok()
                .filter(|c| (4..=31).contains(c))
                .ok_or_else(|| {
                    value_error(&format!(
                        "Invalid bcrypt cost parameter specified: {}",
                        options.cost
                    ))
                })?;
            if password.contains(&0) {
                return Err(value_error(
                    "Bcrypt password must not contain null character",
                ));
            }
            let salt = argon2::random_bytes::<16>()?;
            let parts = bcrypt::hash_with_salt(password, cost, salt)
                .map_err(|e| Error::new(Class::Error, e.to_string()))?;
            return Ok(parts.format_for_version(bcrypt::Version::TwoY));
        }
        PasswordAlgorithm::Argon2i => Variant::Argon2i,
        PasswordAlgorithm::Argon2id => Variant::Argon2id,
        PasswordAlgorithm::Other(_) => {
            return Err(value_error(
                "password_hash(): Argument #2 ($algo) must be a valid password hashing algorithm",
            ));
        }
    };
    let threads = u32::try_from(options.threads)
        .ok()
        .filter(|t| *t >= 1)
        .ok_or_else(|| value_error("Invalid number of threads"))?;
    let memory = u32::try_from(options.memory_cost)
        .ok()
        .filter(|m| u64::from(*m) >= 8 * u64::from(threads))
        .ok_or_else(|| value_error("Memory cost is outside of allowed memory range"))?;
    let time = u32::try_from(options.time_cost)
        .ok()
        .filter(|t| *t >= 1)
        .ok_or_else(|| value_error("Time cost is outside of allowed time range"))?;
    argon2::hash(variant, memory, time, threads, password)?
        .ok_or_else(|| Error::new(Class::Error, "Password hashing failed for unknown reason"))
}

/// `password_verify($password, $hash)`: Argon2 strings through Argon2,
/// anything else through `crypt()`, compared in constant time.
pub fn password_verify(password: &[u8], hash: &str) -> bool {
    if hash.starts_with("$argon2i$") || hash.starts_with("$argon2id$") {
        return argon2::sodium_verify(hash, password);
    }
    match crypt(password, hash.as_bytes()) {
        Crypt::Hash(computed) => hash_equals(&computed, hash.as_bytes()),
        Crypt::Failure(_) | Crypt::Unsupported => false,
    }
}

/// The algorithm `password_get_info()` names for a hash: bcrypt for a
/// 60-byte `$2y$` string, Argon2 by prefix.
fn hash_algorithm(hash: &str) -> Option<PasswordAlgorithm> {
    if hash.len() == 60 && hash.starts_with("$2y$") {
        Some(PasswordAlgorithm::Bcrypt)
    } else if hash.starts_with("$argon2id$") {
        Some(PasswordAlgorithm::Argon2id)
    } else if hash.starts_with("$argon2i$") {
        Some(PasswordAlgorithm::Argon2i)
    } else {
        None
    }
}

/// `password_needs_rehash($hash, $algo, $options)`: `false` for an
/// algorithm PHP does not know; otherwise `true` when the hash is of
/// another algorithm or other options.
pub fn password_needs_rehash(
    hash: &str,
    algorithm: &PasswordAlgorithm,
    options: &PasswordOptions,
) -> bool {
    if let PasswordAlgorithm::Other(_) = algorithm {
        return false;
    }
    if hash_algorithm(hash).as_ref() != Some(algorithm) {
        return true;
    }
    match algorithm {
        PasswordAlgorithm::Bcrypt => {
            let digits: String = hash[4..].chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<i64>().ok() != Some(options.cost)
        }
        PasswordAlgorithm::Argon2i | PasswordAlgorithm::Argon2id => {
            let prefix = if *algorithm == PasswordAlgorithm::Argon2id {
                10
            } else {
                9
            };
            let (m, t, p) = argon2::scan_options(&hash.as_bytes()[prefix..]);
            i64::try_from(m).ok() != Some(options.memory_cost)
                || i64::try_from(t).ok() != Some(options.time_cost)
                || i64::try_from(p).ok() != Some(options.threads)
        }
        PasswordAlgorithm::Other(_) => false,
    }
}
