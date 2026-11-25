//! The user interfaces of symfony/security-core that this component's
//! signatures name (`PasswordAuthenticatedUserInterface`,
//! `LegacyPasswordAuthenticatedUserInterface`), as traits, until a
//! security-core crate holds them. PHP checks these with `instanceof`;
//! here a user tells its class and the classes and interfaces it extends
//! or implements ([`PhpClass`]), which `PasswordHasherFactory` matches
//! against its configuration keys as PHP does (case-insensitive class
//! names).
//!
//! Upstream: none

use crate::hasher::password_hasher_aware_interface::PasswordHasherAwareInterface;

/// A PHP class name and the names of its parent classes and interfaces.
/// A string that names no class has no parents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhpClass<'a> {
    pub name: &'a str,
    pub parents: &'a [&'a str],
}

/// PHP class names compare case-insensitively, with or without a leading
/// backslash.
pub(crate) fn same_class(a: &str, b: &str) -> bool {
    a.trim_start_matches('\\')
        .eq_ignore_ascii_case(b.trim_start_matches('\\'))
}

impl<'a> PhpClass<'a> {
    pub fn new(name: &'a str, parents: &'a [&'a str]) -> Self {
        Self { name, parents }
    }

    /// `$object instanceof $class`, for an object of this class.
    pub fn instance_of(&self, class: &str) -> bool {
        same_class(self.name, class) || self.is_subclass_of(class)
    }

    /// `is_subclass_of($this->name, $class)`.
    pub fn is_subclass_of(&self, class: &str) -> bool {
        self.parents.iter().any(|p| same_class(p, class))
    }
}

/// An object `PasswordHasherFactory::getPasswordHasher()` can take.
pub trait UserObject: Send + Sync {
    /// `get_class($this)` and its parents and interfaces.
    fn php_class(&self) -> PhpClass<'_>;

    /// `$this instanceof PasswordHasherAwareInterface`.
    fn as_password_hasher_aware(&self) -> Option<&dyn PasswordHasherAwareInterface> {
        None
    }
}

/// Security Core's `PasswordAuthenticatedUserInterface`.
pub trait PasswordAuthenticatedUserInterface: UserObject {
    /// Returns the hashed password used to authenticate the user.
    fn get_password(&self) -> Option<String>;

    /// `$this instanceof LegacyPasswordAuthenticatedUserInterface`.
    fn as_legacy(&self) -> Option<&dyn LegacyPasswordAuthenticatedUserInterface> {
        None
    }
}

/// Security Core's `LegacyPasswordAuthenticatedUserInterface`: a user
/// whose password hash needs a salt of its own.
pub trait LegacyPasswordAuthenticatedUserInterface: PasswordAuthenticatedUserInterface {
    /// Returns the salt that was originally used to hash the password.
    fn get_salt(&self) -> Option<String>;
}
