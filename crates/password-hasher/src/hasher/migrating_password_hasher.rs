//! Port of Symfony\Component\PasswordHasher\Hasher\MigratingPasswordHasher.php.
//!
//! Hashes passwords using the best available hasher. Verifies them using
//! a chain of hashers.
//!
//! /!\ Don't put a `PlaintextPasswordHasher` in the list as that'd mean a
//! leaked hash could be used to authenticate successfully without knowing
//! the cleartext password.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/MigratingPasswordHasher.php

use std::sync::Arc;

use crate::{PasswordHasherInterface, exception::Error};

/// The best hasher and the hashers of older hashes.
#[derive(Clone)]
pub struct MigratingPasswordHasher {
    best_hasher: Arc<dyn PasswordHasherInterface>,
    extra_hashers: Vec<Arc<dyn PasswordHasherInterface>>,
}

impl MigratingPasswordHasher {
    pub fn new(
        best_hasher: Arc<dyn PasswordHasherInterface>,
        extra_hashers: Vec<Arc<dyn PasswordHasherInterface>>,
    ) -> Self {
        Self {
            best_hasher,
            extra_hashers,
        }
    }

    pub fn best_hasher(&self) -> &Arc<dyn PasswordHasherInterface> {
        &self.best_hasher
    }

    pub fn extra_hashers(&self) -> &[Arc<dyn PasswordHasherInterface>] {
        &self.extra_hashers
    }
}

impl PasswordHasherInterface for MigratingPasswordHasher {
    fn hash(&self, plain_password: &str, salt: Option<&str>) -> Result<String, Error> {
        self.best_hasher.hash(plain_password, salt)
    }

    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        salt: Option<&str>,
    ) -> Result<bool, Error> {
        if self
            .best_hasher
            .verify(hashed_password, plain_password, salt)?
        {
            return Ok(true);
        }
        if !self.best_hasher.needs_rehash(hashed_password) {
            return Ok(false);
        }
        for hasher in &self.extra_hashers {
            if hasher.verify(hashed_password, plain_password, salt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn needs_rehash(&self, hashed_password: &str) -> bool {
        self.best_hasher.needs_rehash(hashed_password)
    }

    fn class_name(&self) -> &'static str {
        "Symfony\\Component\\PasswordHasher\\Hasher\\MigratingPasswordHasher"
    }
}
