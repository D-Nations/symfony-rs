//! Port of Symfony\Component\PasswordHasher\Hasher\CheckPasswordLengthTrait.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/CheckPasswordLengthTrait.php

use crate::password_hasher_interface::MAX_PASSWORD_LENGTH;

/// `isPasswordTooLong()`: more than `MAX_PASSWORD_LENGTH` bytes.
pub(crate) fn is_password_too_long(password: &str) -> bool {
    MAX_PASSWORD_LENGTH < password.len()
}
