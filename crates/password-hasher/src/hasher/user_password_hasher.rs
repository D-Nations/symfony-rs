//! Port of Symfony\Component\PasswordHasher\Hasher\UserPasswordHasher.php.
//!
//! Hashes passwords based on the user and the `PasswordHasherFactory`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/UserPasswordHasher.php

use std::sync::Arc;

use crate::{
    exception::Error,
    hasher::{
        password_hasher_factory_interface::{HasherSubject, PasswordHasherFactoryInterface},
        user_password_hasher_interface::UserPasswordHasherInterface,
    },
    user::{LegacyPasswordAuthenticatedUserInterface, PasswordAuthenticatedUserInterface},
};

/// Hashes and checks a user's password with the hasher the factory gives
/// for the user.
#[derive(Clone)]
pub struct UserPasswordHasher {
    hasher_factory: Arc<dyn PasswordHasherFactoryInterface>,
}

impl UserPasswordHasher {
    pub fn new(hasher_factory: Arc<dyn PasswordHasherFactoryInterface>) -> Self {
        Self { hasher_factory }
    }
}

fn salt(user: &dyn PasswordAuthenticatedUserInterface) -> Option<String> {
    user.as_legacy()
        .and_then(LegacyPasswordAuthenticatedUserInterface::get_salt)
}

impl UserPasswordHasherInterface for UserPasswordHasher {
    fn hash_password(
        &self,
        user: &dyn PasswordAuthenticatedUserInterface,
        plain_password: &str,
    ) -> Result<String, Error> {
        let salt = salt(user);
        let hasher = self
            .hasher_factory
            .get_password_hasher(HasherSubject::Object(user))?;
        hasher.hash(plain_password, salt.as_deref())
    }

    fn is_password_valid(
        &self,
        user: &dyn PasswordAuthenticatedUserInterface,
        plain_password: &str,
    ) -> Result<bool, Error> {
        let salt = salt(user);
        let Some(password) = user.get_password() else {
            return Ok(false);
        };
        let hasher = self
            .hasher_factory
            .get_password_hasher(HasherSubject::Object(user))?;
        hasher.verify(&password, plain_password, salt.as_deref())
    }

    fn needs_rehash(&self, user: &dyn PasswordAuthenticatedUserInterface) -> Result<bool, Error> {
        let Some(password) = user.get_password() else {
            return Ok(false);
        };
        let hasher = self
            .hasher_factory
            .get_password_hasher(HasherSubject::Object(user))?;
        Ok(hasher.needs_rehash(&password))
    }
}
