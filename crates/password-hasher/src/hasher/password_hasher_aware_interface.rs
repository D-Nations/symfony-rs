//! Port of Symfony\Component\PasswordHasher\Hasher\PasswordHasherAwareInterface.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/PasswordHasherAwareInterface.php

/// An object that names the password hasher of its password.
pub trait PasswordHasherAwareInterface {
    /// Gets the name of the password hasher used to hash the password.
    ///
    /// If the method returns null, the standard way to retrieve the
    /// password hasher will be used instead.
    fn get_password_hasher_name(&self) -> Option<String>;
}
