//! Port of Doctrine\DBAL\Schema\AbstractAsset.php,
//! AbstractNamedObject.php, AbstractOptionallyNamedObject.php and the
//! interfaces NamedObject.php and OptionallyNamedObject.php.
//!
//! The PHP classes are a hierarchy; here [`AbstractAsset`] is a value
//! each schema object holds, told by [`AssetKind`] which of the three
//! classes it stands for and by [`NameParser`] which `getNameParser()`
//! its class overrides.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/AbstractAsset.php src/Schema/AbstractNamedObject.php
//! Upstream: doctrine/dbal src/Schema/AbstractOptionallyNamedObject.php src/Schema/NamedObject.php
//! Upstream: doctrine/dbal src/Schema/OptionallyNamedObject.php

use std::fmt::Write as _;

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;

use super::exception::{self, invalid_state};
use super::name::{
    Identifier, Name, parse_generic_name, parse_optionally_qualified_name, parse_unqualified_name,
};

/// Which `AbstractAsset` subclass an asset is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// A direct subclass of `AbstractAsset` (its `setName()` is not
    /// implemented).
    Bare,
    /// `AbstractNamedObject`: the name is required.
    Named,
    /// `AbstractOptionallyNamedObject`: the name may be empty.
    OptionallyNamed,
}

/// The parser `getNameParser()` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameParser {
    /// Not overridden: `NotImplemented` for the given class.
    NotImplemented(&'static str),
    Unqualified,
    OptionallyQualified,
    Generic,
}

impl NameParser {
    fn parse(self, input: &str) -> Result<Name, Exception> {
        match self {
            Self::NotImplemented(class) => Err(exception::not_implemented(class, "getNameParser")),
            Self::Unqualified => parse_unqualified_name(input).map(Name::Unqualified),
            Self::OptionallyQualified => {
                parse_optionally_qualified_name(input).map(Name::OptionallyQualified)
            }
            Self::Generic => parse_generic_name(input).map(Name::Generic),
        }
    }
}

/// The state of `AbstractAsset` and its named subclasses.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractAsset {
    class: &'static str,
    kind: AssetKind,
    parser: NameParser,
    pub(crate) name: String,
    is_name_initialized: bool,
    pub(crate) namespace: Option<String>,
    pub(crate) quoted: bool,
    identifiers: Vec<Identifier>,
    validate_future: bool,
    object_name: Option<Name>,
}

impl AbstractAsset {
    /// `parent::__construct($name)` from `class`.
    pub fn new(
        class: &'static str,
        kind: AssetKind,
        parser: NameParser,
        name: Option<&str>,
    ) -> Self {
        let mut asset = Self {
            class,
            kind,
            parser,
            name: String::new(),
            is_name_initialized: false,
            namespace: None,
            quoted: false,
            identifiers: Vec::new(),
            validate_future: false,
            object_name: None,
        };
        match name {
            None => deprecations::trigger("https://github.com/doctrine/dbal/pull/6610", || {
                "Not passing $name to Doctrine\\DBAL\\Schema\\AbstractAsset::__construct is deprecated."
                    .to_owned()
            }),
            Some(name) => asset.set_name_string(name),
        }
        asset
    }

    /// `static::class` of the object holding this state.
    pub fn class(&self) -> &'static str {
        self.class
    }

    /// `setName(?Name)`, as each kind implements it.
    fn set_object_name(&mut self, name: Option<Name>) -> Result<(), Exception> {
        match self.kind {
            AssetKind::Bare => Err(exception::not_implemented(self.class, "setName")),
            AssetKind::Named => match name {
                None => Err(exception::name_from_empty()),
                Some(name) => {
                    self.object_name = Some(name);
                    Ok(())
                }
            },
            AssetKind::OptionallyNamed => {
                self.object_name = name;
                Ok(())
            }
        }
    }

    /// `_setName($name)`: sets the name of this asset.
    pub(crate) fn set_name_string(&mut self, name: &str) {
        self.is_name_initialized = false;

        let input = name;
        let mut name = name.to_owned();

        if is_identifier_quoted(&name) {
            self.quoted = true;
            name = trim_quotes(&name);
        }

        if name.contains('.') {
            // [$this->_namespace, $name] = explode('.', $name)
            let mut parts = name.split('.');
            self.namespace = parts.next().map(str::to_owned);
            parts.next().unwrap_or_default().clone_into(&mut self.name);
        } else {
            self.name = name;
        }
        self.validate_future = false;

        let parsed_name = if input.is_empty() {
            None
        } else {
            match self.parser.parse(input) {
                Ok(parsed) => Some(parsed),
                Err(e) => {
                    if e.class() == exception::NOT_IMPLEMENTED {
                        let message = e.message().to_string_lossy().into_owned();
                        deprecations::trigger("https://github.com/doctrine/dbal/pull/6592", || {
                            format!("Unable to parse object name: {message}.")
                        });
                    }
                    // Other errors: `Deprecation::triggerIfCalledFromOutside()`.
                    return;
                }
            }
        };

        if let Err(e) = self.set_object_name(parsed_name.clone()) {
            let message = e.message().to_string_lossy().into_owned();
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6646", || {
                format!("Using invalid database object names is deprecated: {message}.")
            });
            return;
        }

        self.is_name_initialized = true;

        let identifiers = match parsed_name {
            None => {
                self.identifiers = Vec::new();
                return;
            }
            Some(Name::Unqualified(name)) => vec![name.get_identifier().clone()],
            Some(Name::OptionallyQualified(name)) => match name.get_qualifier() {
                Some(qualifier) => vec![qualifier.clone(), name.get_unqualified_name().clone()],
                None => vec![name.get_unqualified_name().clone()],
            },
            Some(Name::Generic(name)) => name.get_identifiers().to_vec(),
        };

        let (namespace, name) = match identifiers.as_slice() {
            [name] => (None, name.clone()),
            [namespace, name] => (Some(namespace.clone()), name.clone()),
            _ => return,
        };

        self.identifiers = identifiers;
        self.validate_future = true;

        let future_name = name.get_value();
        let future_namespace = namespace.as_ref().map(Identifier::get_value);

        if self.name != future_name {
            let current = self.name.clone();
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6592", || {
                format!(
                    "Instead of \"{current}\", this name will be interpreted as \"{future_name}\" in 5.0"
                )
            });
        }

        if self.namespace.as_deref() != future_namespace {
            let current = self
                .namespace
                .as_ref()
                .map_or_else(|| "null".to_owned(), |n| format!("\"{n}\""));
            let future = future_namespace.map_or_else(|| "null".to_owned(), |n| format!("\"{n}\""));
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6592", || {
                format!(
                    "Instead of {current}, the namespace in this name will be interpreted as {future} in 5.0."
                )
            });
        }
    }

    /// `getObjectName()` of `AbstractNamedObject` and
    /// `AbstractOptionallyNamedObject`: the parsed name (`None` for an
    /// optional name not given).
    pub fn get_object_name(&self) -> Result<Option<&Name>, Exception> {
        if !self.is_name_initialized {
            return Err(invalid_state::object_name_not_initialized());
        }
        Ok(self.object_name.as_ref())
    }

    /// Is this asset in the default namespace?
    pub fn is_in_default_namespace(&self, default_namespace_name: &str) -> bool {
        self.namespace.is_none() || self.namespace.as_deref() == Some(default_namespace_name)
    }

    /// Gets the namespace name of this asset.
    pub fn get_namespace_name(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// The shortest name is stripped of the default namespace. All other
    /// namespaced elements are returned as full-qualified names.
    pub fn get_shortest_name(&self, default_namespace_name: Option<&str>) -> String {
        let mut shortest_name = self.get_name();
        if self.namespace.as_deref() == default_namespace_name {
            shortest_name.clone_from(&self.name);
        }
        shortest_name.to_ascii_lowercase()
    }

    /// Checks if this asset's name is quoted.
    pub fn is_quoted(&self) -> bool {
        self.quoted
    }

    /// Returns the name of this schema asset.
    pub fn get_name(&self) -> String {
        match &self.namespace {
            Some(namespace) => format!("{namespace}.{}", self.name),
            None => self.name.clone(),
        }
    }

    /// Gets the quoted representation of this asset but only if it was
    /// defined with one. Otherwise return the plain unquoted value.
    ///
    /// Reserved keywords are quoted.
    pub fn get_quoted_name(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        let keywords = platform.get_reserved_keywords_list();
        let folding = platform.get_unquoted_identifier_folding();
        let mut parts = Vec::new();
        let mut normalized_parts = Vec::new();

        for identifier in self.get_name().split('.') {
            let is_quoted = self.quoted || keywords.is_keyword(identifier);
            if is_quoted {
                parts.push(platform.quote_single_identifier(identifier));
                normalized_parts.push(identifier.to_owned());
            } else {
                parts.push(identifier.to_owned());
                normalized_parts.push(folding.fold_unquoted_identifier(identifier));
            }
        }

        let name = parts.join(".");

        if self.validate_future {
            let future_parts: Vec<String> = self
                .identifiers
                .iter()
                .map(|identifier| {
                    if identifier.is_quoted() {
                        identifier.get_value().to_owned()
                    } else {
                        folding.fold_unquoted_identifier(identifier.get_value())
                    }
                })
                .collect();

            if normalized_parts != future_parts {
                let future = future_parts
                    .iter()
                    .map(|part| platform.quote_single_identifier(part))
                    .collect::<Vec<_>>()
                    .join(".");
                deprecations::trigger("https://github.com/doctrine/dbal/pull/6592", || {
                    format!(
                        "Relying on implicitly quoted identifiers preserving their original case is deprecated. \
                         The current name {name} will become {future} in 5.0. \
                         Please quote the name if the case needs to be preserved."
                    )
                });
            }
        }

        name
    }

    /// `_generateIdentifierName($columnNames, $prefix, $maxSize)`.
    pub fn generate_identifier_name(
        column_names: &[String],
        prefix: &str,
        max_size: usize,
    ) -> String {
        generate_identifier_name(column_names, prefix, max_size)
    }
}

/// `isIdentifierQuoted()`: starts with `` ` ``, `"` or `[`.
pub(crate) fn is_identifier_quoted(identifier: &str) -> bool {
    matches!(identifier.as_bytes().first(), Some(b'`' | b'"' | b'['))
}

/// `trimQuotes()`: removes every `` ` ``, `"`, `[` and `]`.
pub(crate) fn trim_quotes(identifier: &str) -> String {
    identifier.replace(['`', '"', '[', ']'], "")
}

/// Generates an identifier from a list of column names obeying a certain
/// string length: `strtoupper(substr($prefix . '_' . $hash, 0,
/// $maxSize))`, the hash being each name's `dechex(crc32())`.
pub(crate) fn generate_identifier_name(
    column_names: &[String],
    prefix: &str,
    max_size: usize,
) -> String {
    let hash = column_names.iter().fold(String::new(), |mut hash, column| {
        let _ = write!(hash, "{:x}", crc32(column.as_bytes()));
        hash
    });
    let mut name = format!("{prefix}_{hash}").into_bytes();
    name.truncate(max_size);
    String::from_utf8_lossy(&name).to_ascii_uppercase()
}

/// CRC-32 (IEEE 802.3, reflected, as zlib and PHP's `crc32()` compute).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }
}
