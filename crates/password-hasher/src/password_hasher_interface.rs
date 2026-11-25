//! Port of Symfony\Component\PasswordHasher\PasswordHasherInterface.php.
//!
//! PHP lets a caller pass a salt to any hasher (`MigratingPasswordHasher`
//! and `UserPasswordHasher` do): a hasher that declares no `$salt`
//! parameter ignores it. Rust has no optional arguments, so every method
//! here takes the salt, and the self-salting hashers ignore it.
//! [`LegacyPasswordHasherInterface`](crate::LegacyPasswordHasherInterface)
//! marks the hashers that use it.
//!
//! `verify()` returns a `Result` because the salted hashers throw from it
//! for a salt with `{` or `}`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher PasswordHasherInterface.php

use std::any::Any;

use crate::exception::Error;

/// `PasswordHasherInterface::MAX_PASSWORD_LENGTH`.
pub const MAX_PASSWORD_LENGTH: usize = 4096;

/// Provides password hashing capabilities.
pub trait PasswordHasherInterface: Any + Send + Sync {
    /// Hashes a plain password.
    ///
    /// Fails with `InvalidPasswordException` when the plain password is
    /// invalid, e.g. excessively long.
    fn hash(&self, plain_password: &str, salt: Option<&str>) -> Result<String, Error>;

    /// Verifies a plain password against a hash.
    fn verify(
        &self,
        hashed_password: &str,
        plain_password: &str,
        salt: Option<&str>,
    ) -> Result<bool, Error>;

    /// Checks if a password hash would benefit from rehashing.
    fn needs_rehash(&self, hashed_password: &str) -> bool;

    /// `$hasher instanceof LegacyPasswordHasherInterface`.
    fn is_legacy(&self) -> bool {
        false
    }

    /// `$hasher::class`.
    fn class_name(&self) -> &'static str;
}

impl dyn PasswordHasherInterface {
    /// `$hasher instanceof T`, as a reference to the concrete hasher.
    pub fn downcast_ref<T: PasswordHasherInterface>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
}
