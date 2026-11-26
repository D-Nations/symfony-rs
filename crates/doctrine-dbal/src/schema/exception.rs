//! Port of the Schema exceptions the `SQLite` platform slice reaches:
//! Schema\SchemaException.php, Schema\Exception\*.php (those named below),
//! Schema\Collections\Exception.php and its Exception\*.php, and
//! Schema\Name\Parser\Exception.php and its Exception\*.php.
//!
//! Each named constructor returns the crate's one [`Exception`] value
//! with the class PHP throws and upstream's message. [`ancestors`] lists
//! the classes and interfaces each class extends or implements, for
//! `Exception::is_instance_of()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SchemaException.php src/Schema/Exception/ColumnAlreadyExists.php
//! Upstream: doctrine/dbal src/Schema/Exception/ColumnDoesNotExist.php src/Schema/Exception/ForeignKeyDoesNotExist.php
//! Upstream: doctrine/dbal src/Schema/Exception/IncomparableNames.php src/Schema/Exception/IndexAlreadyExists.php
//! Upstream: doctrine/dbal src/Schema/Exception/IndexDoesNotExist.php src/Schema/Exception/IndexNameInvalid.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidColumnDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidForeignKeyConstraintDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidIdentifier.php src/Schema/Exception/InvalidIndexDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidName.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidPrimaryKeyConstraintDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidState.php src/Schema/Exception/InvalidTableDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidTableModification.php src/Schema/Exception/InvalidTableName.php
//! Upstream: doctrine/dbal src/Schema/Exception/InvalidUniqueConstraintDefinition.php
//! Upstream: doctrine/dbal src/Schema/Exception/NotImplemented.php src/Schema/Exception/PrimaryKeyAlreadyExists.php
//! Upstream: doctrine/dbal src/Schema/Exception/UniqueConstraintDoesNotExist.php
//! Upstream: doctrine/dbal src/Schema/Exception/UnknownColumnOption.php
//! Upstream: doctrine/dbal src/Schema/Collections/Exception.php src/Schema/Collections/Exception/ObjectAlreadyExists.php
//! Upstream: doctrine/dbal src/Schema/Collections/Exception/ObjectDoesNotExist.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/Exception.php src/Schema/Name/Parser/Exception/ExpectedDot.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/Exception/ExpectedNextIdentifier.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/Exception/InvalidName.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/Exception/UnableToParseIdentifier.php

use crate::exception::Exception;

use super::name::{OptionallyQualifiedName, UnqualifiedName};

const NS: &str = "Doctrine\\DBAL\\Schema\\Exception\\";

macro_rules! classes {
    ($($konst:ident = $name:literal;)*) => {
        $(pub const $konst: &str = concat!("Doctrine\\DBAL\\Schema\\Exception\\", $name);)*
    };
}

classes! {
    COLUMN_ALREADY_EXISTS = "ColumnAlreadyExists";
    COLUMN_DOES_NOT_EXIST = "ColumnDoesNotExist";
    FOREIGN_KEY_DOES_NOT_EXIST = "ForeignKeyDoesNotExist";
    INCOMPARABLE_NAMES = "IncomparableNames";
    INDEX_ALREADY_EXISTS = "IndexAlreadyExists";
    INDEX_DOES_NOT_EXIST = "IndexDoesNotExist";
    INDEX_NAME_INVALID = "IndexNameInvalid";
    INVALID_COLUMN_DEFINITION = "InvalidColumnDefinition";
    INVALID_FOREIGN_KEY_CONSTRAINT_DEFINITION = "InvalidForeignKeyConstraintDefinition";
    INVALID_IDENTIFIER = "InvalidIdentifier";
    INVALID_INDEX_DEFINITION = "InvalidIndexDefinition";
    INVALID_NAME = "InvalidName";
    INVALID_PRIMARY_KEY_CONSTRAINT_DEFINITION = "InvalidPrimaryKeyConstraintDefinition";
    INVALID_STATE = "InvalidState";
    INVALID_TABLE_DEFINITION = "InvalidTableDefinition";
    INVALID_TABLE_MODIFICATION = "InvalidTableModification";
    INVALID_TABLE_NAME = "InvalidTableName";
    INVALID_UNIQUE_CONSTRAINT_DEFINITION = "InvalidUniqueConstraintDefinition";
    NOT_IMPLEMENTED = "NotImplemented";
    PRIMARY_KEY_ALREADY_EXISTS = "PrimaryKeyAlreadyExists";
    UNIQUE_CONSTRAINT_DOES_NOT_EXIST = "UniqueConstraintDoesNotExist";
    UNKNOWN_COLUMN_OPTION = "UnknownColumnOption";
}

/// The `Doctrine\DBAL\Schema\SchemaException` interface.
pub const SCHEMA_EXCEPTION: &str = "Doctrine\\DBAL\\Schema\\SchemaException";
/// The `Doctrine\DBAL\Schema\Collections\Exception` interface.
pub const COLLECTIONS_EXCEPTION: &str = "Doctrine\\DBAL\\Schema\\Collections\\Exception";
pub const OBJECT_ALREADY_EXISTS: &str =
    "Doctrine\\DBAL\\Schema\\Collections\\Exception\\ObjectAlreadyExists";
pub const OBJECT_DOES_NOT_EXIST: &str =
    "Doctrine\\DBAL\\Schema\\Collections\\Exception\\ObjectDoesNotExist";
/// The `Doctrine\DBAL\Schema\Name\Parser\Exception` interface (a
/// `Throwable`, not a DBAL exception).
pub const PARSER_EXCEPTION: &str = "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception";
pub const EXPECTED_DOT: &str = "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception\\ExpectedDot";
pub const EXPECTED_NEXT_IDENTIFIER: &str =
    "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception\\ExpectedNextIdentifier";
pub const PARSER_INVALID_NAME: &str =
    "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception\\InvalidName";
pub const UNABLE_TO_PARSE_IDENTIFIER: &str =
    "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception\\UnableToParseIdentifier";

const DBAL: &str = "Doctrine\\DBAL\\Exception";
const LOGIC: &str = "LogicException";
const INVALID_ARGUMENT: &str = "InvalidArgumentException";

/// The classes and interfaces `class` extends or implements, besides
/// itself and `Throwable` (empty for a class this module does not know).
pub fn ancestors(class: &str) -> &'static [&'static str] {
    const LOGIC_SCHEMA: &[&str] = &[LOGIC, SCHEMA_EXCEPTION, DBAL, "Exception"];
    const ARGUMENT_SCHEMA: &[&str] =
        &[INVALID_ARGUMENT, LOGIC, SCHEMA_EXCEPTION, DBAL, "Exception"];
    const LOGIC_COLLECTION: &[&str] = &[
        LOGIC,
        COLLECTIONS_EXCEPTION,
        SCHEMA_EXCEPTION,
        DBAL,
        "Exception",
    ];
    const LOGIC_PARSER: &[&str] = &[LOGIC, PARSER_EXCEPTION, "Exception"];
    const ARGUMENT_PARSER: &[&str] = &[INVALID_ARGUMENT, LOGIC, PARSER_EXCEPTION, "Exception"];
    if let Some(short) = class.strip_prefix(NS) {
        return match short {
            "IncomparableNames"
            | "InvalidIdentifier"
            | "InvalidName"
            | "IndexNameInvalid"
            | "InvalidTableName"
            | "UnknownColumnOption" => {
                if short == "IncomparableNames" {
                    LOGIC_SCHEMA
                } else {
                    ARGUMENT_SCHEMA
                }
            }
            "ColumnAlreadyExists"
            | "ColumnDoesNotExist"
            | "ForeignKeyDoesNotExist"
            | "IndexAlreadyExists"
            | "IndexDoesNotExist"
            | "InvalidColumnDefinition"
            | "InvalidForeignKeyConstraintDefinition"
            | "InvalidIndexDefinition"
            | "InvalidPrimaryKeyConstraintDefinition"
            | "InvalidState"
            | "InvalidTableDefinition"
            | "InvalidTableModification"
            | "InvalidUniqueConstraintDefinition"
            | "NotImplemented"
            | "PrimaryKeyAlreadyExists"
            | "UniqueConstraintDoesNotExist" => LOGIC_SCHEMA,
            _ => &[],
        };
    }
    match class {
        OBJECT_ALREADY_EXISTS | OBJECT_DOES_NOT_EXIST => LOGIC_COLLECTION,
        EXPECTED_DOT | EXPECTED_NEXT_IDENTIFIER | UNABLE_TO_PARSE_IDENTIFIER => LOGIC_PARSER,
        PARSER_INVALID_NAME => ARGUMENT_PARSER,
        _ => &[],
    }
}

fn new(class: &'static str, message: String) -> Exception {
    Exception::new(class, message)
}

/// `ColumnAlreadyExists::new($tableName, $columnName)`.
pub fn column_already_exists(table_name: &str, column_name: &str) -> Exception {
    new(
        COLUMN_ALREADY_EXISTS,
        format!("The column \"{column_name}\" on table \"{table_name}\" already exists."),
    )
}

/// `ColumnDoesNotExist::new($columnName, $table)`.
pub fn column_does_not_exist(column_name: &str, table: &str) -> Exception {
    new(
        COLUMN_DOES_NOT_EXIST,
        format!("There is no column with name \"{column_name}\" on table \"{table}\"."),
    )
}

/// `ForeignKeyDoesNotExist::new($foreignKeyName, $table)`.
pub fn foreign_key_does_not_exist(foreign_key_name: &str, table: &str) -> Exception {
    new(
        FOREIGN_KEY_DOES_NOT_EXIST,
        format!(
            "There exists no foreign key with the name \"{foreign_key_name}\" on table \"{table}\"."
        ),
    )
}

/// `IncomparableNames::fromOptionallyQualifiedNames($name1, $name2)`.
pub fn incomparable_names(
    name1: &OptionallyQualifiedName,
    name2: &OptionallyQualifiedName,
) -> Exception {
    new(
        INCOMPARABLE_NAMES,
        format!("Non-equally qualified names are incomparable: {name1}, {name2}."),
    )
}

/// `IndexAlreadyExists::new($indexName, $table)`.
pub fn index_already_exists(index_name: &str, table: &str) -> Exception {
    new(
        INDEX_ALREADY_EXISTS,
        format!("An index with name \"{index_name}\" was already defined on table \"{table}\"."),
    )
}

/// `IndexDoesNotExist::new($indexName, $table)`.
pub fn index_does_not_exist(index_name: &str, table: &str) -> Exception {
    new(
        INDEX_DOES_NOT_EXIST,
        format!("Index \"{index_name}\" does not exist on table \"{table}\"."),
    )
}

/// `IndexNameInvalid::new($indexName)`.
pub fn index_name_invalid(index_name: &str) -> Exception {
    new(
        INDEX_NAME_INVALID,
        format!("Invalid index name \"{index_name}\" given, has to be [a-zA-Z0-9_]."),
    )
}

/// `InvalidColumnDefinition::nameNotSpecified()`.
pub fn column_name_not_specified() -> Exception {
    new(
        INVALID_COLUMN_DEFINITION,
        "Column name is not specified.".to_owned(),
    )
}

/// `InvalidColumnDefinition::dataTypeNotSpecified($columnName)`.
pub fn column_data_type_not_specified(column_name: &UnqualifiedName) -> Exception {
    new(
        INVALID_COLUMN_DEFINITION,
        format!("Data type is not specified for column {column_name}."),
    )
}

fn format_optional_name(name: Option<&UnqualifiedName>) -> String {
    name.map_or_else(|| "<unnamed>".to_owned(), UnqualifiedName::to_string)
}

/// `InvalidForeignKeyConstraintDefinition::referencedTableNameNotSet()`.
pub fn foreign_key_referenced_table_name_not_set(name: Option<&UnqualifiedName>) -> Exception {
    new(
        INVALID_FOREIGN_KEY_CONSTRAINT_DEFINITION,
        format!(
            "Referenced table name is not set for foreign key constraint {}.",
            format_optional_name(name)
        ),
    )
}

/// `InvalidForeignKeyConstraintDefinition::referencingColumnNamesNotSet()`.
pub fn foreign_key_referencing_column_names_not_set(name: Option<&UnqualifiedName>) -> Exception {
    new(
        INVALID_FOREIGN_KEY_CONSTRAINT_DEFINITION,
        format!(
            "Referencing column names are not set for foreign key constraint {}.",
            format_optional_name(name)
        ),
    )
}

/// `InvalidForeignKeyConstraintDefinition::referencedColumnNamesNotSet()`.
pub fn foreign_key_referenced_column_names_not_set(name: Option<&UnqualifiedName>) -> Exception {
    new(
        INVALID_FOREIGN_KEY_CONSTRAINT_DEFINITION,
        format!(
            "Referenced column names are not set for foreign key constraint {}.",
            format_optional_name(name)
        ),
    )
}

/// `InvalidIdentifier::fromEmpty()`.
pub fn identifier_from_empty() -> Exception {
    new(INVALID_IDENTIFIER, "Identifier cannot be empty.".to_owned())
}

/// `InvalidIndexDefinition::nameNotSet()`.
pub fn index_name_not_set() -> Exception {
    new(
        INVALID_INDEX_DEFINITION,
        "Index name is not set.".to_owned(),
    )
}

/// `InvalidIndexDefinition::columnsNotSet($indexName)`.
pub fn index_columns_not_set(index_name: &UnqualifiedName) -> Exception {
    new(
        INVALID_INDEX_DEFINITION,
        format!("Columns are not set for index {index_name}."),
    )
}

/// `InvalidIndexDefinition::fromNonPositiveColumnLength($columnName, $length)`.
pub fn index_non_positive_column_length(column_name: &UnqualifiedName, length: i64) -> Exception {
    new(
        INVALID_INDEX_DEFINITION,
        format!(
            "Indexed column length must be a positive integer, {length} given for column {column_name}."
        ),
    )
}

/// `InvalidName::fromEmpty()`.
pub fn name_from_empty() -> Exception {
    new(INVALID_NAME, "Name cannot be empty.".to_owned())
}

/// `InvalidPrimaryKeyConstraintDefinition::columnNamesNotSet()`.
pub fn primary_key_column_names_not_set() -> Exception {
    new(
        INVALID_PRIMARY_KEY_CONSTRAINT_DEFINITION,
        "Primary key constraint column names are not set.".to_owned(),
    )
}

/// The `InvalidState` named constructors.
pub mod invalid_state {
    use super::{INVALID_STATE, new};
    use crate::exception::Exception;

    pub fn object_name_not_initialized() -> Exception {
        new(
            INVALID_STATE,
            "Object name has not been initialized.".to_owned(),
        )
    }

    pub fn index_has_invalid_type(index_name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Index \"{index_name}\" has invalid type."),
        )
    }

    pub fn index_has_invalid_predicate(index_name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Index \"{index_name}\" has invalid predicate."),
        )
    }

    pub fn index_has_invalid_columns(index_name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Index \"{index_name}\" has invalid columns."),
        )
    }

    pub fn foreign_key_constraint_has_invalid_referenced_table_name(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Foreign key constraint \"{name}\" has invalid referenced table name."),
        )
    }

    pub fn foreign_key_constraint_has_invalid_referencing_column_names(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!(
                "Foreign key constraint \"{name}\" has one or more invalid referencing column names."
            ),
        )
    }

    pub fn foreign_key_constraint_has_invalid_referenced_column_names(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!(
                "Foreign key constraint \"{name}\" has one or more invalid referenced column name."
            ),
        )
    }

    pub fn foreign_key_constraint_has_invalid_match_type(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Foreign key constraint \"{name}\" has invalid match type."),
        )
    }

    pub fn foreign_key_constraint_has_invalid_on_update_action(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Foreign key constraint \"{name}\" has invalid ON UPDATE action."),
        )
    }

    pub fn foreign_key_constraint_has_invalid_on_delete_action(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Foreign key constraint \"{name}\" has invalid ON DELETE action."),
        )
    }

    pub fn foreign_key_constraint_has_invalid_deferrability(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Foreign key constraint \"{name}\" has invalid deferrability."),
        )
    }

    pub fn unique_constraint_has_invalid_column_names(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Unique constraint \"{name}\" has one or more invalid column names."),
        )
    }

    pub fn unique_constraint_has_empty_column_names(name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Unique constraint \"{name}\" has no column names."),
        )
    }

    pub fn table_has_invalid_primary_key_constraint(table_name: &str) -> Exception {
        new(
            INVALID_STATE,
            format!("Table \"{table_name}\" has invalid primary key constraint."),
        )
    }

    pub fn table_diff_contains_unnamed_dropped_foreign_key_constraints() -> Exception {
        new(
            INVALID_STATE,
            "Table diff contains unnamed dropped foreign key constraints".to_owned(),
        )
    }
}

/// `InvalidTableDefinition::nameNotSet()`.
pub fn table_name_not_set() -> Exception {
    new(
        INVALID_TABLE_DEFINITION,
        "Table name is not set.".to_owned(),
    )
}

/// `InvalidTableDefinition::columnsNotSet($tableName)`.
pub fn table_columns_not_set(table_name: &OptionallyQualifiedName) -> Exception {
    new(
        INVALID_TABLE_DEFINITION,
        format!("Columns are not set for table {table_name}."),
    )
}

/// What `InvalidTableModification` reports about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableObject {
    Column,
    Index,
    UniqueConstraint,
    ForeignKeyConstraint,
}

impl TableObject {
    fn label(self) -> &'static str {
        match self {
            Self::Column => "Column",
            Self::Index => "Index",
            Self::UniqueConstraint => "Unique constraint",
            Self::ForeignKeyConstraint => "Foreign key constraint",
        }
    }
}

fn format_table_name(table_name: Option<&OptionallyQualifiedName>) -> String {
    table_name.map_or_else(
        || "<unnamed>".to_owned(),
        OptionallyQualifiedName::to_string,
    )
}

/// `InvalidTableModification::{column,index,...}AlreadyExists($tableName,
/// $previous)` (`previous` an `ObjectAlreadyExists`).
pub fn table_object_already_exists(
    object: TableObject,
    table_name: Option<&OptionallyQualifiedName>,
    object_name: &UnqualifiedName,
    previous: Exception,
) -> Exception {
    new(
        INVALID_TABLE_MODIFICATION,
        format!(
            "{} {} already exists on table {}.",
            object.label(),
            object_name,
            format_table_name(table_name)
        ),
    )
    .with_previous(previous)
}

/// `InvalidTableModification::{column,index,...}DoesNotExist($tableName,
/// $previous)` (`previous` an `ObjectDoesNotExist`).
pub fn table_object_does_not_exist(
    object: TableObject,
    table_name: Option<&OptionallyQualifiedName>,
    object_name: &UnqualifiedName,
    previous: Exception,
) -> Exception {
    new(
        INVALID_TABLE_MODIFICATION,
        format!(
            "{} {} does not exist on table {}.",
            object.label(),
            object_name,
            format_table_name(table_name)
        ),
    )
    .with_previous(previous)
}

/// `InvalidTableModification::primaryKeyConstraintAlreadyExists()`.
pub fn primary_key_constraint_already_exists(
    table_name: Option<&OptionallyQualifiedName>,
) -> Exception {
    new(
        INVALID_TABLE_MODIFICATION,
        format!(
            "Primary key constraint already exists on table {}.",
            format_table_name(table_name)
        ),
    )
}

/// `InvalidTableModification::primaryKeyConstraintDoesNotExist()`.
pub fn primary_key_constraint_does_not_exist(
    table_name: Option<&OptionallyQualifiedName>,
) -> Exception {
    new(
        INVALID_TABLE_MODIFICATION,
        format!(
            "Primary key constraint does not exist on table {}.",
            format_table_name(table_name)
        ),
    )
}

/// `InvalidTableName::new($tableName)`.
pub fn invalid_table_name(table_name: &str) -> Exception {
    new(
        INVALID_TABLE_NAME,
        format!("Invalid table name specified \"{table_name}\"."),
    )
}

/// `InvalidUniqueConstraintDefinition::columnNamesAreNotSet($name)`.
pub fn unique_constraint_column_names_not_set(name: Option<&UnqualifiedName>) -> Exception {
    new(
        INVALID_UNIQUE_CONSTRAINT_DEFINITION,
        format!(
            "Column names are not set for unique constraint {}.",
            format_optional_name(name)
        ),
    )
}

/// `NotImplemented::fromMethod($class, $method)`.
pub fn not_implemented(class: &str, method: &str) -> Exception {
    new(
        NOT_IMPLEMENTED,
        format!("Class {class} does not implement method {method}()."),
    )
}

/// `PrimaryKeyAlreadyExists::new($tableName)`.
pub fn primary_key_already_exists(table_name: &str) -> Exception {
    new(
        PRIMARY_KEY_ALREADY_EXISTS,
        format!("Primary key was already defined on table \"{table_name}\"."),
    )
}

/// `UniqueConstraintDoesNotExist::new($constraintName, $table)`.
pub fn unique_constraint_does_not_exist(constraint_name: &str, table: &str) -> Exception {
    new(
        UNIQUE_CONSTRAINT_DOES_NOT_EXIST,
        format!(
            "There exists no unique constraint with the name \"{constraint_name}\" on table \"{table}\"."
        ),
    )
}

/// `UnknownColumnOption::new($name)`.
pub fn unknown_column_option(name: &str) -> Exception {
    new(
        UNKNOWN_COLUMN_OPTION,
        format!("The \"{name}\" column option is not supported."),
    )
}

/// `ObjectAlreadyExists::new($objectName)`.
pub fn object_already_exists(object_name: &UnqualifiedName) -> Exception {
    new(
        OBJECT_ALREADY_EXISTS,
        format!("Object {object_name} already exists."),
    )
}

/// `ObjectDoesNotExist::new($objectName)`.
pub fn object_does_not_exist(object_name: &UnqualifiedName) -> Exception {
    new(
        OBJECT_DOES_NOT_EXIST,
        format!("Object {object_name} does not exist."),
    )
}

/// `ExpectedDot::new($position, $got)`.
pub fn expected_dot(position: usize, got: u8) -> Exception {
    let mut message = format!("Expected dot at position {position}, got \"").into_bytes();
    message.push(got);
    message.extend_from_slice(b"\".");
    Exception::new(EXPECTED_DOT, message)
}

/// `ExpectedNextIdentifier::new()`.
pub fn expected_next_identifier() -> Exception {
    new(
        EXPECTED_NEXT_IDENTIFIER,
        "Unexpected end of input. Next identifier expected.".to_owned(),
    )
}

/// `Parser\Exception\InvalidName::forUnqualifiedName($count)`.
pub fn invalid_unqualified_name(count: usize) -> Exception {
    new(
        PARSER_INVALID_NAME,
        format!("An unqualified name must consist of one identifier, {count} given."),
    )
}

/// `Parser\Exception\InvalidName::forOptionallyQualifiedName($count)`.
pub fn invalid_optionally_qualified_name(count: usize) -> Exception {
    new(
        PARSER_INVALID_NAME,
        format!(
            "An optionally qualified name must consist of one or two identifiers, {count} given."
        ),
    )
}

/// `UnableToParseIdentifier::new($offset)`.
pub fn unable_to_parse_identifier(offset: usize) -> Exception {
    new(
        UNABLE_TO_PARSE_IDENTIFIER,
        format!("Unable to parse identifier at offset {offset}."),
    )
}
