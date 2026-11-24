//! Port of Symfony\Component\Routing\Alias.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Alias.php

use php_value::PhpString;

use crate::exception::{Class, Error};

/// A deprecation set on an alias: `['package' => ..., 'version' => ...,
/// 'message' => ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation {
    pub package: PhpString,
    pub version: PhpString,
    pub message: PhpString,
}

/// `Alias`: another name for a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    id: PhpString,
    deprecation: Option<Deprecation>,
}

impl Alias {
    pub fn new(id: impl Into<PhpString>) -> Self {
        Self {
            id: id.into(),
            deprecation: None,
        }
    }

    /// `withId()`: a copy pointing at another route.
    pub fn with_id(&self, id: impl Into<PhpString>) -> Self {
        Self {
            id: id.into(),
            ..self.clone()
        }
    }

    /// `getId()`: the target name of this alias.
    pub fn id(&self) -> &PhpString {
        &self.id
    }

    /// `setDeprecated()`.
    pub fn set_deprecated(
        &mut self,
        package: impl Into<PhpString>,
        version: impl Into<PhpString>,
        message: impl Into<PhpString>,
    ) -> Result<&mut Self, Error> {
        let message: PhpString = message.into();
        if !message.is_empty() {
            // preg_match('#[\r\n]|\*/#', $message)
            let m = message.as_bytes();
            if m.iter().any(|&c| c == b'\r' || c == b'\n') || m.windows(2).any(|w| w == b"*/") {
                return Err(Error::new(
                    Class::InvalidArgument,
                    "Invalid characters found in deprecation template.",
                ));
            }
            if !m.windows(10).any(|w| w == b"%alias_id%") {
                return Err(Error::new(
                    Class::InvalidArgument,
                    "The deprecation template must contain the \"%alias_id%\" placeholder.",
                ));
            }
        }
        self.deprecation = Some(Deprecation {
            package: package.into(),
            version: version.into(),
            // `$message ?: default`: only '' gets here falsy, "0" having
            // failed the placeholder check above.
            message: if message.is_empty() {
                PhpString::from(
                    "The \"%alias_id%\" route alias is deprecated. You should stop using it, as it will be removed in the future.",
                )
            } else {
                message
            },
        });
        Ok(self)
    }

    /// `isDeprecated()`.
    pub fn is_deprecated(&self) -> bool {
        self.deprecation.is_some()
    }

    /// `getDeprecation($name)`: the deprecation with `%alias_id%`
    /// replaced by the route name relying on this alias; `None` when not
    /// deprecated (PHP reads undefined keys there, with warnings).
    pub fn deprecation(&self, name: &[u8]) -> Option<Deprecation> {
        let d = self.deprecation.as_ref()?;
        Some(Deprecation {
            package: d.package.clone(),
            version: d.version.clone(),
            message: PhpString::from(replace(d.message.as_bytes(), b"%alias_id%", name)),
        })
    }
}

/// `str_replace($search, $replace, $subject)` for one non-empty search.
fn replace(subject: &[u8], search: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(search) {
            out.extend_from_slice(with);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}
