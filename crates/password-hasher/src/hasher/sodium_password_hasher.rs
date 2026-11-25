//! Port of Symfony\Component\PasswordHasher\Hasher\SodiumPasswordHasher.php.
//!
//! Hashes passwords using libsodium's `crypto_pwhash_str()` format
//! (Argon2id, src/php/argon2.rs). Libsodium is always "available" here:
//! the port computes what the oracle's sodium 1.0.18 computes, so
//! [`SodiumPasswordHasher::is_supported`] is `true` and the
//! "Libsodium is not available" branches do not occur.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/SodiumPasswordHasher.php

use crate::{
    PasswordHasherInterface,
    exception::Error,
    hasher::{
        check_password_length_trait::is_password_too_long,
        native_password_hasher::{MEMLIMIT_INTERACTIVE, OPSLIMIT_INTERACTIVE, bcrypt_prehash},
    },
    php::{argon2, password::password_verify},
};

/// `SODIUM_CRYPTO_PWHASH_STRPREFIX`.
pub const SODIUM_CRYPTO_PWHASH_STRPREFIX: &str = "$argon2id$";

/// Hashes with `sodium_crypto_pwhash_str()`.
#[derive(Debug, Clone)]
pub struct SodiumPasswordHasher {
    ops_limit: i64,
    mem_limit: i64,
}

impl SodiumPasswordHasher {
    pub fn new(ops_limit: Option<i64>, mem_limit: Option<i64>) -> Result<Self, Error> {
        let ops_limit = ops_limit.unwrap_or(4.max(OPSLIMIT_INTERACTIVE));
        let mem_limit = mem_limit.unwrap_or((64 * 1024 * 1024).max(MEMLIMIT_INTERACTIVE));

        if 3 > ops_limit {
            return Err(Error::invalid_argument("$opsLimit must be 3 or greater."));
        }
        if 10 * 1024 > mem_limit {
            return Err(Error::invalid_argument("$memLimit must be 10k or greater."));
        }
        Ok(Self {
            ops_limit,
            mem_limit,
        })
    }

    /// Libsodium 1.0.14 or later is available: always, in this port.
    pub fn is_supported() -> bool {
        true
    }

    pub fn ops_limit(&self) -> i64 {
        self.ops_limit
    }

    pub fn mem_limit(&self) -> i64 {
        self.mem_limit
    }

    /// The limits as libsodium reads them (both at least 3 and 10 KiB).
    fn limits(&self) -> (u64, u64) {
        (
            u64::try_from(self.ops_limit).unwrap_or(0),
            u64::try_from(self.mem_limit).unwrap_or(0),
        )
    }
}

impl PasswordHasherInterface for SodiumPasswordHasher {
    fn hash(&self, plain_password: &str, _salt: Option<&str>) -> Result<String, Error> {
        if is_password_too_long(plain_password) {
            return Err(Error::invalid_password());
        }
        let (ops, mem) = self.limits();
        argon2::sodium_hash(plain_password.as_bytes(), ops, mem)
    }

    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        _salt: Option<&str>,
    ) -> Result<bool, Error> {
        if plain_password.is_empty() {
            return Ok(false);
        }
        if is_password_too_long(plain_password) {
            return Ok(false);
        }
        if !hashed_password.starts_with("$argon") {
            if hashed_password.starts_with("$2")
                && (72 < plain_password.len() || plain_password.contains('\0'))
            {
                let prehashed = bcrypt_prehash(plain_password);
                return Ok(password_verify(prehashed.as_bytes(), hashed_password));
            }
            // Accept validating non-argon passwords for seamless migrations
            return Ok(password_verify(plain_password.as_bytes(), hashed_password));
        }
        Ok(argon2::sodium_verify(
            hashed_password,
            plain_password.as_bytes(),
        ))
    }

    fn needs_rehash(&self, hashed_password: &str) -> bool {
        let (ops, mem) = self.limits();
        argon2::sodium_needs_rehash(hashed_password, ops, mem)
    }

    fn class_name(&self) -> &'static str {
        "Symfony\\Component\\PasswordHasher\\Hasher\\SodiumPasswordHasher"
    }
}
