//! Port of Doctrine\DBAL\Types\StringType.php and the types built on it:
//! AsciiStringType.php, GuidType.php, and TextType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/StringType.php src/Types/AsciiStringType.php
//! Upstream: doctrine/dbal src/Types/GuidType.php src/Types/TextType.php

use php_value::{PhpArray, PhpString};

use super::Type;
use crate::{ParameterType, exception::Exception, platforms::AbstractPlatform};

/// Type that maps an SQL VARCHAR to a PHP string.
#[derive(Debug, Default)]
pub struct StringType;

impl Type for StringType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\StringType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_string_type_declaration_sql(column)
    }
}

/// A string of ASCII characters (`extends StringType`).
#[derive(Debug, Default)]
pub struct AsciiStringType;

impl Type for AsciiStringType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\AsciiStringType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_ascii_string_type_declaration_sql(column)
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::Ascii
    }
}

/// Represents a GUID/UUID datatype (both are actually synonyms) in the
/// database (`extends StringType`).
#[derive(Debug, Default)]
pub struct GuidType;

impl Type for GuidType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\GuidType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_guid_type_declaration_sql(column)
    }
}

/// Type that maps an SQL CLOB to a PHP string. A stream resource from the
/// driver is read whole in PHP; `php_value::Value` has no resources, so
/// values pass unchanged.
#[derive(Debug, Default)]
pub struct TextType;

impl Type for TextType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\TextType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_clob_type_declaration_sql(column)
    }
}
