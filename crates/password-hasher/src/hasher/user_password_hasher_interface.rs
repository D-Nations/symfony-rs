//! Port of Symfony\Component\PasswordHasher\Hasher\UserPasswordHasherInterface.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/UserPasswordHasherInterface.php

use crate::{exception::Error, user::PasswordAuthenticatedUserInterface};

/// Interface for the user password hasher service.
pub trait UserPasswordHasherInterface {
    /// Hashes the plain password for the given user.
    fn hash_password(
        &self,
        user: &dyn PasswordAuthenticatedUserInterface,
        plain_password: &str,
    ) -> Result<String, Error>;

    /// Checks if the plaintext password matches the user's password.
    fn is_password_valid(
        &self,
        user: &dyn PasswordAuthenticatedUserInterface,
        plain_password: &str,
    ) -> Result<bool, Error>;

    /// Checks if an encoded password would benefit from rehashing.
    fn needs_rehash(&self, user: &dyn PasswordAuthenticatedUserInterface) -> Result<bool, Error>;
}
