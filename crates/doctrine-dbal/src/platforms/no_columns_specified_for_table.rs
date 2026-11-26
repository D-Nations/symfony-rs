//! Port of Doctrine\DBAL\Platforms\Exception\NoColumnsSpecifiedForTable.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Exception/NoColumnsSpecifiedForTable.php

use crate::exception::Exception;

/// `Doctrine\DBAL\Platforms\Exception\NoColumnsSpecifiedForTable` (a
/// `LogicException` and a `PlatformException`).
pub const NO_COLUMNS_SPECIFIED_FOR_TABLE: &str =
    "Doctrine\\DBAL\\Platforms\\Exception\\NoColumnsSpecifiedForTable";

/// The `Doctrine\DBAL\Platforms\Exception\PlatformException` interface.
pub const PLATFORM_EXCEPTION: &str = "Doctrine\\DBAL\\Platforms\\Exception\\PlatformException";

/// `NoColumnsSpecifiedForTable::new($tableName)`.
pub fn no_columns_specified_for_table(table_name: &str) -> Exception {
    Exception::new(
        NO_COLUMNS_SPECIFIED_FOR_TABLE,
        format!("No columns specified for table \"{table_name}\"."),
    )
}
