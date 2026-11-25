//! Port of Doctrine\DBAL\Types\EnumType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/EnumType.php

use php_value::{PhpArray, PhpString};

use super::Type;
use crate::{exception::Exception, platforms::AbstractPlatform};

/// A column of one of a list of string values.
#[derive(Debug, Default)]
pub struct EnumType;

impl Type for EnumType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\EnumType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_enum_declaration_sql(column)
    }
}
