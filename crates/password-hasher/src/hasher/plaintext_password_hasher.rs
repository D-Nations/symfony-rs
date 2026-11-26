//! Port of Symfony\Component\PasswordHasher\Hasher\PlaintextPasswordHasher.php.
//!
//! `PlaintextPasswordHasher` does not do any hashing but is useful in
//! testing environments. As this hasher is not cryptographically secure,
//! usage of it in production environments is discouraged.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/PlaintextPasswordHasher.php

use crate::{
    LegacyPasswordHasherInterface, PasswordHasherInterface,
    exception::Error,
    hasher::{
        check_password_length_trait::is_password_too_long,
        message_digest_password_hasher::merge_password_and_salt,
    },
    php::hash::hash_equals,
};

/// Stores `password{salt}` as it is.
#[derive(Debug, Clone, Default)]
pub struct PlaintextPasswordHasher {
    ignore_password_case: bool,
}

impl PlaintextPasswordHasher {
    /// `$ignorePasswordCase`: compare the password case-insensitive
    /// (`strtolower()`, ASCII letters only).
    pub fn new(ignore_password_case: bool) -> Self {
        Self {
            ignore_password_case,
        }
    }

    pub fn ignore_password_case(&self) -> bool {
        self.ignore_password_case
    }
}

impl PasswordHasherInterface for PlaintextPasswordHasher {
    fn hash(&self, plain_password: &str, salt: Option<&str>) -> Result<String, Error> {
        if is_password_too_long(plain_password) {
            return Err(Error::invalid_password());
        }
        merge_password_and_salt(plain_password, salt)
    }

    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        salt: Option<&str>,
    ) -> Result<bool, Error> {
        if is_password_too_long(plain_password) {
            return Ok(false);
        }
        let pass2 = merge_password_and_salt(plain_password, salt)?;
        if !self.ignore_password_case {
            return Ok(hash_equals(hashed_password.as_bytes(), pass2.as_bytes()));
        }
        Ok(hash_equals(
            hashed_password.to_ascii_lowercase().as_bytes(),
            pass2.to_ascii_lowercase().as_bytes(),
        ))
    }

    fn needs_rehash(&self, _hashed_password: &str) -> bool {
        false
    }

    fn is_legacy(&self) -> bool {
        true
    }

    fn class_name(&self) -> &'static str {
        "Symfony\\Component\\PasswordHasher\\Hasher\\PlaintextPasswordHasher"
    }
}

impl LegacyPasswordHasherInterface for PlaintextPasswordHasher {}
