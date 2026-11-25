//! Port of Symfony\Component\PasswordHasher\Hasher\NativePasswordHasher.php.
//!
//! Hashes passwords using `password_hash()` (src/php/password.rs). The
//! environment is that of the oracle (PHP 8.4.27 with sodium 1.0.18 and
//! libargon2): `PASSWORD_ARGON2I` and `PASSWORD_ARGON2ID` exist, and
//! Argon2 hashes are verified with `sodium_crypto_pwhash_str_verify()`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/NativePasswordHasher.php

use sha2::{Digest, Sha512};

use crate::{
    PasswordHasherInterface,
    exception::Error,
    hasher::check_password_length_trait::is_password_too_long,
    php::{
        argon2,
        hash::base64_encode,
        password::{
            PasswordAlgorithm, PasswordOptions, password_hash, password_needs_rehash,
            password_verify,
        },
    },
};

/// `SODIUM_CRYPTO_PWHASH_OPSLIMIT_INTERACTIVE` of libsodium 1.0.18.
pub(crate) const OPSLIMIT_INTERACTIVE: i64 = 2;
/// `SODIUM_CRYPTO_PWHASH_MEMLIMIT_INTERACTIVE` of libsodium 1.0.18.
pub(crate) const MEMLIMIT_INTERACTIVE: i64 = 64 * 1024 * 1024;

/// Hashes with `password_hash()`: bcrypt unless another algorithm is
/// given.
#[derive(Debug, Clone)]
pub struct NativePasswordHasher {
    algorithm: PasswordAlgorithm,
    options: PasswordOptions,
}

/// `base64_encode(hash('sha512', $password, true))`: what bcrypt gets for
/// a password longer than 72 bytes or with a NUL byte.
pub(crate) fn bcrypt_prehash(password: &str) -> String {
    base64_encode(&Sha512::digest(password.as_bytes()))
}

impl NativePasswordHasher {
    /// `$algorithm` is an algorithm supported by `password_hash()` or
    /// `None` to use the best available algorithm (bcrypt); `"1"`, `"2"`
    /// and `"3"` are the legacy names of bcrypt, Argon2i and Argon2id.
    pub fn new(
        ops_limit: Option<i64>,
        mem_limit: Option<i64>,
        cost: Option<i64>,
        algorithm: Option<&str>,
    ) -> Result<Self, Error> {
        let cost = cost.unwrap_or(13);
        let ops_limit = ops_limit.unwrap_or(4.max(OPSLIMIT_INTERACTIVE));
        let mem_limit = mem_limit.unwrap_or((64 * 1024 * 1024).max(MEMLIMIT_INTERACTIVE));

        if 3 > ops_limit {
            return Err(Error::invalid_argument("$opsLimit must be 3 or greater."));
        }
        if 10 * 1024 > mem_limit {
            return Err(Error::invalid_argument("$memLimit must be 10k or greater."));
        }
        if !(4..=31).contains(&cost) {
            return Err(Error::invalid_argument(
                "$cost must be in the range of 4-31.",
            ));
        }

        let algorithm = match algorithm {
            None | Some("1" | "2y") => PasswordAlgorithm::Bcrypt,
            Some("2" | "argon2i") => PasswordAlgorithm::Argon2i,
            Some("3" | "argon2id") => PasswordAlgorithm::Argon2id,
            Some(other) => PasswordAlgorithm::Other(other.to_owned()),
        };

        Ok(Self {
            algorithm,
            options: PasswordOptions {
                cost,
                time_cost: ops_limit,
                memory_cost: mem_limit >> 10,
                threads: 1,
            },
        })
    }

    /// The `password_hash()` algorithm.
    pub fn algorithm(&self) -> &PasswordAlgorithm {
        &self.algorithm
    }

    /// The `password_hash()` options.
    pub fn options(&self) -> &PasswordOptions {
        &self.options
    }
}

impl PasswordHasherInterface for NativePasswordHasher {
    fn hash(&self, plain_password: &str, _salt: Option<&str>) -> Result<String, Error> {
        if is_password_too_long(plain_password) {
            return Err(Error::invalid_password());
        }
        if self.algorithm == PasswordAlgorithm::Bcrypt
            && (72 < plain_password.len() || plain_password.contains('\0'))
        {
            let prehashed = bcrypt_prehash(plain_password);
            return password_hash(prehashed.as_bytes(), &self.algorithm, &self.options);
        }
        password_hash(plain_password.as_bytes(), &self.algorithm, &self.options)
    }

    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        _salt: Option<&str>,
    ) -> Result<bool, Error> {
        if plain_password.is_empty() || is_password_too_long(plain_password) {
            return Ok(false);
        }
        if !hashed_password.starts_with("$argon") {
            // Bcrypt cuts on NUL chars and after 72 bytes
            if hashed_password.starts_with("$2")
                && (72 < plain_password.len() || plain_password.contains('\0'))
            {
                let prehashed = bcrypt_prehash(plain_password);
                return Ok(password_verify(prehashed.as_bytes(), hashed_password));
            }
            return Ok(password_verify(plain_password.as_bytes(), hashed_password));
        }
        Ok(argon2::sodium_verify(
            hashed_password,
            plain_password.as_bytes(),
        ))
    }

    fn needs_rehash(&self, hashed_password: &str) -> bool {
        password_needs_rehash(hashed_password, &self.algorithm, &self.options)
    }

    fn class_name(&self) -> &'static str {
        "Symfony\\Component\\PasswordHasher\\Hasher\\NativePasswordHasher"
    }
}
