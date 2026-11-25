//! Port of the symfony/password-hasher component. See README.md for the
//! contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod exception;
pub mod hasher;
pub mod legacy_password_hasher_interface;
pub mod password_hasher_interface;
pub mod php;
pub mod user;

pub use exception::{Class, Error};
pub use hasher::{
    message_digest_password_hasher::MessageDigestPasswordHasher,
    migrating_password_hasher::MigratingPasswordHasher,
    native_password_hasher::NativePasswordHasher,
    password_hasher_aware_interface::PasswordHasherAwareInterface,
    password_hasher_factory::{HasherConfig, PasswordHasherFactory},
    password_hasher_factory_interface::{HasherSubject, PasswordHasherFactoryInterface},
    pbkdf2_password_hasher::Pbkdf2PasswordHasher,
    plaintext_password_hasher::PlaintextPasswordHasher,
    sodium_password_hasher::{SODIUM_CRYPTO_PWHASH_STRPREFIX, SodiumPasswordHasher},
    user_password_hasher::UserPasswordHasher,
    user_password_hasher_interface::UserPasswordHasherInterface,
};
pub use legacy_password_hasher_interface::LegacyPasswordHasherInterface;
pub use password_hasher_interface::{MAX_PASSWORD_LENGTH, PasswordHasherInterface};
pub use php::password::{PASSWORD_ARGON2I, PASSWORD_ARGON2ID, PASSWORD_BCRYPT};
pub use user::{
    LegacyPasswordAuthenticatedUserInterface, PasswordAuthenticatedUserInterface, PhpClass,
    UserObject,
};
