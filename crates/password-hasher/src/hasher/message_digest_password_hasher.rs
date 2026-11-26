//! Port of Symfony\Component\PasswordHasher\Hasher\MessageDigestPasswordHasher.php.
//!
//! `MessageDigestPasswordHasher` uses a message digest algorithm. The
//! algorithms are those of [`HashAlgorithm`]; any other name is "not
//! supported", as an unknown name is in PHP (README.md, "Differences").
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/MessageDigestPasswordHasher.php

use crate::{
    LegacyPasswordHasherInterface, PasswordHasherInterface,
    exception::Error,
    hasher::check_password_length_trait::is_password_too_long,
    php::hash::{HashAlgorithm, base64_encode, bin2hex, hash_equals},
};

/// Hashes with `hash($algorithm, ...)`, stretched over `$iterations`.
#[derive(Debug, Clone)]
pub struct MessageDigestPasswordHasher {
    algorithm: String,
    encode_hash_as_base64: bool,
    iterations: i64,
    /// `-1` in PHP when the algorithm is not supported.
    hash_length: Option<usize>,
}

impl Default for MessageDigestPasswordHasher {
    /// `new MessageDigestPasswordHasher()`: `sha512`, base64, 5000
    /// iterations.
    fn default() -> Self {
        Self::new("sha512", true, 5000)
    }
}

impl MessageDigestPasswordHasher {
    /// `$algorithm` is the digest algorithm to use, `$encodeHashAsBase64`
    /// whether to base64 encode the password hash (else hex), and
    /// `$iterations` the number of iterations to use to stretch the
    /// password hash.
    pub fn new(algorithm: impl Into<String>, encode_hash_as_base64: bool, iterations: i64) -> Self {
        let mut hasher = Self {
            algorithm: algorithm.into(),
            encode_hash_as_base64,
            iterations,
            hash_length: None,
        };
        // A LogicException (algorithm not supported) leaves it unset.
        hasher.hash_length = hasher.hash("", Some("salt")).ok().map(|h| h.len());
        hasher
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
}

/// `mergePasswordAndSalt()`: `password{salt}`, or the password alone for a
/// falsy salt (`null`, `''`, `'0'`).
pub(crate) fn merge_password_and_salt(password: &str, salt: Option<&str>) -> Result<String, Error> {
    let Some(salt) = salt.filter(|s| !s.is_empty() && *s != "0") else {
        return Ok(password.to_owned());
    };
    if salt.contains('{') || salt.contains('}') {
        return Err(Error::invalid_argument("Cannot use { or } in salt."));
    }
    Ok(format!("{password}{{{salt}}}"))
}

impl PasswordHasherInterface for MessageDigestPasswordHasher {
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
        let salted = merge_password_and_salt(plain_password, salt)?;
        let mut digest = algorithm.digest(salted.as_bytes());

        // "stretch" hash
        let mut i = 1;
        while i < self.iterations {
            digest.extend_from_slice(salted.as_bytes());
            digest = algorithm.digest(&digest);
            i += 1;
        }

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
        if Some(hashed_password.len()) != self.hash_length || hashed_password.contains('$') {
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
        "Symfony\\Component\\PasswordHasher\\Hasher\\MessageDigestPasswordHasher"
    }
}

impl LegacyPasswordHasherInterface for MessageDigestPasswordHasher {}
