//! Port of Symfony\Component\PasswordHasher\Hasher\PasswordHasherFactoryInterface.php.
//!
//! PHP's `string|PasswordAuthenticatedUserInterface|PasswordHasherAwareInterface`
//! argument is [`HasherSubject`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/PasswordHasherFactoryInterface.php

use std::sync::Arc;

use crate::{
    PasswordHasherInterface,
    exception::Error,
    user::{PhpClass, UserObject},
};

/// What a password hasher is looked up for.
#[derive(Clone, Copy)]
pub enum HasherSubject<'a> {
    /// A class name (or any other string).
    Class(PhpClass<'a>),
    /// A user object.
    Object(&'a dyn UserObject),
}

impl<'a> HasherSubject<'a> {
    /// A string that names no class.
    pub fn name(name: &'a str) -> Self {
        Self::Class(PhpClass::new(name, &[]))
    }
}

/// Supports different password hashers for different user accounts.
pub trait PasswordHasherFactoryInterface: Send + Sync {
    /// Returns the password hasher to use for the given user.
    ///
    /// Fails with a `\RuntimeException` when no password hasher could be
    /// found for the user.
    fn get_password_hasher(
        &self,
        user: HasherSubject<'_>,
    ) -> Result<Arc<dyn PasswordHasherInterface>, Error>;
}
