//! Port of Symfony\Component\PasswordHasher\Hasher\Pbkdf2PasswordHasher.php.
//!
//! `Pbkdf2PasswordHasher` uses the PBKDF2 (Password-Based Key Derivation
//! Function 2). Providing a high level of Cryptographic security, PBKDF2
//! is recommended by the National Institute of Standards and Technology
//! (NIST). But also warrants a warning, using PBKDF2 (with a high number
//! of iterations) slows down the process. PBKDF2 should be used with
//! caution and care.
//!
//! PHP's constructor computes a hash to learn the encoded length, and lets
//! `hash_pbkdf2()`'s `\ValueError` for iterations below 1 or a negative
//! length through; here [`Pbkdf2PasswordHasher::new`] returns it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/Pbkdf2PasswordHasher.php

use crate::{
    LegacyPasswordHasherInterface, PasswordHasherInterface,
    exception::{Class, Error},
    hasher::check_password_length_trait::is_password_too_long,
    php::hash::{HashAlgorithm, base64_encode, bin2hex, hash_equals},
};

/// Hashes with `hash_pbkdf2()`.
#[derive(Debug, Clone)]
pub struct Pbkdf2PasswordHasher {
    algorithm: String,
    encode_hash_as_base64: bool,
    iterations: i64,
    length: i64,
    /// `-1` in PHP when the algorithm is not supported.
    encoded_length: Option<usize>,
}

impl Default for Pbkdf2PasswordHasher {
    /// `new Pbkdf2PasswordHasher()`: `sha512`, base64, 1000 iterations,
    /// 40 bytes.
    fn default() -> Self {
        let mut hasher = Self::build("sha512", true, 1000, 40);
        hasher.encoded_length = hasher.hash("", Some("salt")).ok().map(|h| h.len());
        hasher
    }
}

impl Pbkdf2PasswordHasher {
    /// `$algorithm` is the digest algorithm to use, `$encodeHashAsBase64`
    /// whether to base64 encode the password hash (else hex),
    /// `$iterations` the number of iterations to use to stretch the
    /// password hash and `$length` the length of derived key to create.
    pub fn new(
        algorithm: impl Into<String>,
        encode_hash_as_base64: bool,
        iterations: i64,
        length: i64,
    ) -> Result<Self, Error> {
        let mut hasher = Self::build(algorithm, encode_hash_as_base64, iterations, length);
        match hasher.hash("", Some("salt")) {
            Ok(hash) => hasher.encoded_length = Some(hash.len()),
            // ignore unsupported algorithm
            Err(e) if e.is_logic() => {}
            Err(e) => return Err(e),
        }
        Ok(hasher)
    }

    fn build(
        algorithm: impl Into<String>,
        encode_hash_as_base64: bool,
        iterations: i64,
        length: i64,
    ) -> Self {
        Self {
            algorithm: algorithm.into(),
            encode_hash_as_base64,
            iterations,
            length,
            encoded_length: None,
        }
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn encode_hash_as_base64(&self) -> bool {
        self.encode_hash_as_base64
    }

    pub fn iterations(&self) -> i64 {
        self.iterations
    }

    pub fn length(&self) -> i64 {
        self.length
    }
}

impl PasswordHasherInterface for Pbkdf2PasswordHasher {
    fn hash(&self, plain_password: &str, salt: Option<&str>) -> Result<String, Error> {
        if is_password_too_long(plain_password) {
            return Err(Error::invalid_password());
        }
        let Some(algorithm) = HashAlgorithm::from_name(&self.algorithm) else {
            return Err(Error::logic(format!(
                "The algorithm \"{}\" is not supported.",
                self.algorithm
            )));
        };
        let Ok(iterations @ 1..) = u64::try_from(self.iterations) else {
            return Err(Error::new(
                Class::ValueError,
                "hash_pbkdf2(): Argument #4 ($iterations) must be greater than 0",
            ));
        };
        let Ok(length) = usize::try_from(self.length) else {
            return Err(Error::new(
                Class::ValueError,
                "hash_pbkdf2(): Argument #5 ($length) must be greater than or equal to 0",
            ));
        };
        let digest = algorithm.pbkdf2(
            plain_password.as_bytes(),
            salt.unwrap_or("").as_bytes(),
            iterations,
            length,
        );
        Ok(if self.encode_hash_as_base64 {
            base64_encode(&digest)
        } else {
            bin2hex(&digest)
        })
    }

    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        salt: Option<&str>,
    ) -> Result<bool, Error> {
        if Some(hashed_password.len()) != self.encoded_length || hashed_password.contains('$') {
            return Ok(false);
        }
        if is_password_too_long(plain_password) {
            return Ok(false);
        }
        let hash = self.hash(plain_password, salt)?;
        Ok(hash_equals(hashed_password.as_bytes(), hash.as_bytes()))
    }

    fn needs_rehash(&self, _hashed_password: &str) -> bool {
        false
    }

    fn is_legacy(&self) -> bool {
        true
    }

    fn class_name(&self) -> &'static str {
        "Symfony\\Component\\PasswordHasher\\Hasher\\Pbkdf2PasswordHasher"
    }
}

impl LegacyPasswordHasherInterface for Pbkdf2PasswordHasher {}
