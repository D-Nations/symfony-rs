//! Port of Symfony\Component\DependencyInjection\Alias.php.
//!
//! An `Alias` is a shared PHP object (see [`crate::object`]): the handle
//! a container returns is the alias it holds.
//!
//! Upstream: symfony/dependency-injection Alias.php

use std::fmt;

use php_value::{PhpArray, Value};

use crate::exception::{Error, Result};
use crate::object::{ObjectData, Shared};

const DEFAULT_DEPRECATION_TEMPLATE: &str = "The \"%alias_id%\" service alias is deprecated. You should stop using it, as it will be removed in the future.";

/// A deprecation as `getDeprecation($id)` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation {
    pub package: String,
    pub version: String,
    pub message: String,
}

impl Deprecation {
    /// The `['package' => …, 'version' => …, 'message' => …]` array.
    pub fn to_array(&self) -> PhpArray {
        let mut array = PhpArray::new();
        array.insert("package", Value::from(self.package.as_str()));
        array.insert("version", Value::from(self.version.as_str()));
        array.insert("message", Value::from(self.message.as_str()));
        array
    }
}

/// Checks a deprecation template as `setDeprecated()` does, with the
/// placeholder of the class (`%alias_id%` or `%service_id%`).
pub(crate) fn check_deprecation_template(message: &str, placeholder: &str) -> Result<()> {
    if message.is_empty() {
        return Ok(());
    }
    // preg_match('#[\r\n]|\*/#', $message)
    if message.contains(['\r', '\n']) || message.contains("*/") {
        return Err(Error::invalid_argument(
            "Invalid characters found in deprecation template.",
        ));
    }
    if !message.contains(placeholder) {
        return Err(Error::invalid_argument(format!(
            "The deprecation template must contain the \"{placeholder}\" placeholder."
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct AliasData {
    id: String,
    public: bool,
    deprecation: Option<Deprecation>,
}

impl ObjectData for AliasData {
    fn class(&self) -> &'static str {
        Alias::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("id", Value::from(self.id.as_str()));
        properties.insert("public", Value::Bool(self.public));
        properties.insert(
            "deprecation",
            Value::Array(
                self.deprecation
                    .as_ref()
                    .map(Deprecation::to_array)
                    .unwrap_or_default(),
            ),
        );
        properties
    }
}

/// `Alias`.
#[derive(Clone)]
pub struct Alias(Shared<AliasData>);

impl Alias {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Alias";

    /// `new Alias($id, $public)`.
    pub fn new(id: impl Into<String>, public: bool) -> Self {
        Self(Shared::new(AliasData {
            id: id.into(),
            public,
            deprecation: None,
        }))
    }

    /// `(string) $alias`: the aliased id.
    pub fn id(&self) -> String {
        self.0.read(|a| a.id.clone())
    }

    pub fn is_public(&self) -> bool {
        self.0.read(|a| a.public)
    }

    pub fn set_public(&self, public: bool) -> &Self {
        self.0.write(|a| a.public = public);
        self
    }

    pub fn is_private(&self) -> bool {
        !self.is_public()
    }

    /// `setDeprecated($package, $version, $message)`; an empty message
    /// takes the default template.
    pub fn set_deprecated(&self, package: &str, version: &str, message: &str) -> Result<&Self> {
        check_deprecation_template(message, "%alias_id%")?;
        let message = if message.is_empty() {
            DEFAULT_DEPRECATION_TEMPLATE
        } else {
            message
        };
        self.0.write(|a| {
            a.deprecation = Some(Deprecation {
                package: package.to_string(),
                version: version.to_string(),
                message: message.to_string(),
            });
        });
        Ok(self)
    }

    pub fn is_deprecated(&self) -> bool {
        self.0.read(|a| a.deprecation.is_some())
    }

    /// `getDeprecation($id)`: `%alias_id%` replaced by `id`. `None` when
    /// the alias is not deprecated (PHP reads undefined keys there).
    pub fn deprecation(&self, id: &str) -> Option<Deprecation> {
        self.0.read(|a| {
            a.deprecation.as_ref().map(|d| Deprecation {
                package: d.package.clone(),
                version: d.version.clone(),
                message: d.message.replace("%alias_id%", id),
            })
        })
    }

    /// `$a === $b`.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    /// `clone $alias`.
    #[must_use]
    pub fn clone_object(&self) -> Self {
        Self(self.0.clone_object())
    }

    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

impl fmt::Debug for Alias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.read(|a| {
            f.debug_struct("Alias")
                .field("id", &a.id)
                .field("public", &a.public)
                .finish_non_exhaustive()
        })
    }
}

impl fmt::Display for Alias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id())
    }
}

impl From<Alias> for Value {
    fn from(alias: Alias) -> Self {
        alias.to_value()
    }
}
