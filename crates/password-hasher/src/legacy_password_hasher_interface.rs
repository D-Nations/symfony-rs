//! Port of Symfony\Component\PasswordHasher\LegacyPasswordHasherInterface.php.
//!
//! Provides password hashing and verification capabilities for "legacy"
//! hashers that require external salts. The methods are those of
//! [`PasswordHasherInterface`], which already take the salt; this trait
//! marks the hashers that use it, and such a hasher answers `true` from
//! [`PasswordHasherInterface::is_legacy`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher LegacyPasswordHasherInterface.php

use crate::PasswordHasherInterface;

/// A hasher that uses the salt its caller gives.
pub trait LegacyPasswordHasherInterface: PasswordHasherInterface {}
