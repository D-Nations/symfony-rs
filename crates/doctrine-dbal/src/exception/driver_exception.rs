//! Port of the exceptions that `Connection.php`, its driver and its
//! exception converter throw: Exception\DriverException.php and its
//! subclasses (ServerException.php, ConstraintViolationException.php,
//! UniqueConstraintViolationException.php, ...), ConnectionException.php
//! (both the top-level class and Exception\ConnectionException.php),
//! Exception\NoActiveTransaction.php, Exception\CommitFailedRollbackOnly.php,
//! Exception\SavepointsNotSupported.php, the driver manager's argument
//! exceptions (DriverRequired.php, UnknownDriver.php,
//! InvalidDriverClass.php, InvalidWrapperClass.php, MalformedDsnException.php),
//! Exception\NoKeyValue.php, Exception\InvalidColumnIndex.php,
//! Exception\ParseError.php, Exception\DatabaseRequired.php,
//! Exception\RetryableException.php, and the driver exceptions
//! Driver\Exception.php, Driver\AbstractException.php,
//! Driver\Exception\IdentityColumnsNotSupported.php and NoIdentityValue.php.
//!
//! Each class is a constant of [`Exception`] (its PHP name) with the
//! constructor PHP gives it; [`parents`] is the class hierarchy that
//! `instanceof` walks.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Exception/DriverException.php src/Exception/ServerException.php
//! Upstream: doctrine/dbal src/Exception/ConstraintViolationException.php src/Exception/UniqueConstraintViolationException.php
//! Upstream: doctrine/dbal src/Exception/ForeignKeyConstraintViolationException.php src/Exception/NotNullConstraintViolationException.php
//! Upstream: doctrine/dbal src/Exception/DatabaseObjectNotFoundException.php src/Exception/TableNotFoundException.php
//! Upstream: doctrine/dbal src/Exception/DatabaseDoesNotExist.php src/Exception/SchemaDoesNotExist.php
//! Upstream: doctrine/dbal src/Exception/DatabaseObjectExistsException.php src/Exception/TableExistsException.php
//! Upstream: doctrine/dbal src/Exception/InvalidFieldNameException.php src/Exception/NonUniqueFieldNameException.php
//! Upstream: doctrine/dbal src/Exception/SyntaxErrorException.php src/Exception/ReadOnlyException.php
//! Upstream: doctrine/dbal src/Exception/DeadlockException.php src/Exception/LockWaitTimeoutException.php
//! Upstream: doctrine/dbal src/Exception/RetryableException.php src/Exception/ConnectionException.php
//! Upstream: doctrine/dbal src/Exception/ConnectionLost.php src/Exception/TransactionRolledBack.php
//! Upstream: doctrine/dbal src/ConnectionException.php src/Exception/NoActiveTransaction.php
//! Upstream: doctrine/dbal src/Exception/CommitFailedRollbackOnly.php src/Exception/SavepointsNotSupported.php
//! Upstream: doctrine/dbal src/Exception/DriverRequired.php src/Exception/UnknownDriver.php
//! Upstream: doctrine/dbal src/Exception/InvalidDriverClass.php src/Exception/InvalidWrapperClass.php
//! Upstream: doctrine/dbal src/Exception/MalformedDsnException.php src/Exception/NoKeyValue.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnIndex.php src/Exception/ParseError.php
//! Upstream: doctrine/dbal src/Exception/DatabaseRequired.php
//! Upstream: doctrine/dbal src/Driver/Exception.php src/Driver/AbstractException.php
//! Upstream: doctrine/dbal src/Driver/Exception/IdentityColumnsNotSupported.php src/Driver/Exception/NoIdentityValue.php

use php_value::PhpString;

use super::Exception;
use crate::Query;

/// The parents of the classes of this slice (and of the PHP classes they
/// reach), interfaces included: `is_a()` walks them.
pub(super) fn parents(class: &str) -> &'static [&'static str] {
    const DBAL: &str = "Doctrine\\DBAL\\Exception";
    match class {
        Exception::DRIVER_EXCEPTION => &[DBAL, Exception::DRIVER_EXCEPTION_INTERFACE, "Exception"],
        Exception::SERVER_EXCEPTION
        | Exception::CONNECTION_EXCEPTION_DRIVER
        | Exception::TRANSACTION_ROLLED_BACK => &[Exception::DRIVER_EXCEPTION],
        Exception::CONSTRAINT_VIOLATION_EXCEPTION
        | Exception::DATABASE_OBJECT_NOT_FOUND_EXCEPTION
        | Exception::DATABASE_OBJECT_EXISTS_EXCEPTION
        | Exception::INVALID_FIELD_NAME_EXCEPTION
        | Exception::NON_UNIQUE_FIELD_NAME_EXCEPTION
        | Exception::SYNTAX_ERROR_EXCEPTION
        | Exception::READ_ONLY_EXCEPTION => &[Exception::SERVER_EXCEPTION],
        Exception::DEADLOCK_EXCEPTION | Exception::LOCK_WAIT_TIMEOUT_EXCEPTION => {
            &[Exception::SERVER_EXCEPTION, Exception::RETRYABLE_EXCEPTION]
        }
        Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION
        | Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION
        | Exception::NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION => {
            &[Exception::CONSTRAINT_VIOLATION_EXCEPTION]
        }
        Exception::TABLE_NOT_FOUND_EXCEPTION
        | Exception::DATABASE_DOES_NOT_EXIST
        | Exception::SCHEMA_DOES_NOT_EXIST => &[Exception::DATABASE_OBJECT_NOT_FOUND_EXCEPTION],
        Exception::TABLE_EXISTS_EXCEPTION => &[Exception::DATABASE_OBJECT_EXISTS_EXCEPTION],
        Exception::CONNECTION_LOST => &[Exception::CONNECTION_EXCEPTION_DRIVER],
        Exception::CONNECTION_EXCEPTION
        | Exception::NO_KEY_VALUE
        | Exception::PARSE_ERROR
        | Exception::DATABASE_REQUIRED => &[DBAL, "Exception"],
        Exception::NO_ACTIVE_TRANSACTION
        | Exception::COMMIT_FAILED_ROLLBACK_ONLY
        | Exception::SAVEPOINTS_NOT_SUPPORTED => &[Exception::CONNECTION_EXCEPTION],
        Exception::INVALID_COLUMN_INDEX => &[DBAL, "LogicException"],
        Exception::DRIVER_REQUIRED
        | Exception::UNKNOWN_DRIVER
        | Exception::INVALID_DRIVER_CLASS
        | Exception::INVALID_WRAPPER_CLASS
        | Exception::MALFORMED_DSN_EXCEPTION => &[Exception::INVALID_ARGUMENT_EXCEPTION],
        Exception::INVALID_ARGUMENT_EXCEPTION => &[DBAL, "InvalidArgumentException"],
        Exception::ABSTRACT_DRIVER_EXCEPTION => {
            &[Exception::DRIVER_EXCEPTION_INTERFACE, "Exception"]
        }
        Exception::PDO_EXCEPTION
        | Exception::IDENTITY_COLUMNS_NOT_SUPPORTED
        | Exception::NO_IDENTITY_VALUE
        | Exception::PDO_INVALID_CONFIGURATION => &[Exception::ABSTRACT_DRIVER_EXCEPTION],
        Exception::DRIVER_EXCEPTION_INTERFACE
        | Exception::RETRYABLE_EXCEPTION
        | "Exception"
        | "Error" => &["Throwable"],
        "InvalidArgumentException" => &["LogicException"],
        "LogicException" | "RuntimeException" => &["Exception"],
        "TypeError" | "ValueError" | "ArgumentCountError" => &["Error"],
        _ => &[],
    }
}

/// Slice Connection: the classes and constructors of the connection, its
/// driver and its exception converter.
impl Exception {
    /// `Doctrine\DBAL\Exception\DriverException`: base class for all
    /// errors detected in the driver.
    pub const DRIVER_EXCEPTION: &'static str = "Doctrine\\DBAL\\Exception\\DriverException";
    /// Base class for all server related errors detected in the driver.
    pub const SERVER_EXCEPTION: &'static str = "Doctrine\\DBAL\\Exception\\ServerException";
    /// Base class for all constraint violation related errors detected in
    /// the driver.
    pub const CONSTRAINT_VIOLATION_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\ConstraintViolationException";
    /// Exception for a unique constraint violation detected in the driver.
    pub const UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\UniqueConstraintViolationException";
    /// Exception for a foreign key constraint violation detected in the
    /// driver.
    pub const FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\ForeignKeyConstraintViolationException";
    /// Exception for a NOT NULL constraint violation detected in the
    /// driver.
    pub const NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\NotNullConstraintViolationException";
    /// Base class for all unknown database object related errors detected
    /// in the driver.
    pub const DATABASE_OBJECT_NOT_FOUND_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\DatabaseObjectNotFoundException";
    /// Exception for an unknown table referenced in a statement detected in
    /// the driver.
    pub const TABLE_NOT_FOUND_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\TableNotFoundException";
    pub const DATABASE_DOES_NOT_EXIST: &'static str =
        "Doctrine\\DBAL\\Exception\\DatabaseDoesNotExist";
    pub const SCHEMA_DOES_NOT_EXIST: &'static str = "Doctrine\\DBAL\\Exception\\SchemaDoesNotExist";
    /// Base class for all already existing database object related errors
    /// detected in the driver.
    pub const DATABASE_OBJECT_EXISTS_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\DatabaseObjectExistsException";
    /// Exception for an already existing table referenced in a statement
    /// detected in the driver.
    pub const TABLE_EXISTS_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\TableExistsException";
    /// Exception for an invalid specified field name in a statement
    /// detected in the driver.
    pub const INVALID_FIELD_NAME_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidFieldNameException";
    /// Exception for a non-unique/ambiguous specified field name in a
    /// statement detected in the driver.
    pub const NON_UNIQUE_FIELD_NAME_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\NonUniqueFieldNameException";
    /// Exception for a syntax error in a statement detected in the driver.
    pub const SYNTAX_ERROR_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\SyntaxErrorException";
    /// Exception for a write operation attempt on a read-only database
    /// element detected in the driver.
    pub const READ_ONLY_EXCEPTION: &'static str = "Doctrine\\DBAL\\Exception\\ReadOnlyException";
    /// Exception for a deadlock error of a transaction detected in the
    /// driver.
    pub const DEADLOCK_EXCEPTION: &'static str = "Doctrine\\DBAL\\Exception\\DeadlockException";
    /// Exception for a lock wait timeout error of a transaction detected in
    /// the driver.
    pub const LOCK_WAIT_TIMEOUT_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\LockWaitTimeoutException";
    /// Marker interface for all exceptions where retrying the transaction
    /// makes sense.
    pub const RETRYABLE_EXCEPTION: &'static str = "Doctrine\\DBAL\\Exception\\RetryableException";
    /// `Doctrine\DBAL\Exception\ConnectionException`: base class for all
    /// connection related errors detected in the driver.
    pub const CONNECTION_EXCEPTION_DRIVER: &'static str =
        "Doctrine\\DBAL\\Exception\\ConnectionException";
    pub const CONNECTION_LOST: &'static str = "Doctrine\\DBAL\\Exception\\ConnectionLost";
    pub const TRANSACTION_ROLLED_BACK: &'static str =
        "Doctrine\\DBAL\\Exception\\TransactionRolledBack";
    /// `Doctrine\DBAL\ConnectionException`.
    pub const CONNECTION_EXCEPTION: &'static str = "Doctrine\\DBAL\\ConnectionException";
    pub const NO_ACTIVE_TRANSACTION: &'static str =
        "Doctrine\\DBAL\\Exception\\NoActiveTransaction";
    pub const COMMIT_FAILED_ROLLBACK_ONLY: &'static str =
        "Doctrine\\DBAL\\Exception\\CommitFailedRollbackOnly";
    pub const SAVEPOINTS_NOT_SUPPORTED: &'static str =
        "Doctrine\\DBAL\\Exception\\SavepointsNotSupported";
    pub const DRIVER_REQUIRED: &'static str = "Doctrine\\DBAL\\Exception\\DriverRequired";
    pub const UNKNOWN_DRIVER: &'static str = "Doctrine\\DBAL\\Exception\\UnknownDriver";
    pub const INVALID_DRIVER_CLASS: &'static str = "Doctrine\\DBAL\\Exception\\InvalidDriverClass";
    pub const INVALID_WRAPPER_CLASS: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidWrapperClass";
    pub const MALFORMED_DSN_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\MalformedDsnException";
    pub const NO_KEY_VALUE: &'static str = "Doctrine\\DBAL\\Exception\\NoKeyValue";
    pub const INVALID_COLUMN_INDEX: &'static str = "Doctrine\\DBAL\\Exception\\InvalidColumnIndex";
    pub const PARSE_ERROR: &'static str = "Doctrine\\DBAL\\Exception\\ParseError";
    pub const DATABASE_REQUIRED: &'static str = "Doctrine\\DBAL\\Exception\\DatabaseRequired";
    /// `Doctrine\DBAL\Driver\Exception`: the contract for a driver
    /// exception, which provides the `SQLSTATE` of the driver.
    pub const DRIVER_EXCEPTION_INTERFACE: &'static str = "Doctrine\\DBAL\\Driver\\Exception";
    /// `Doctrine\DBAL\Driver\AbstractException`.
    pub const ABSTRACT_DRIVER_EXCEPTION: &'static str = "Doctrine\\DBAL\\Driver\\AbstractException";
    /// `Doctrine\DBAL\Driver\PDO\Exception`, what the `SQLite` driver throws
    /// (it stands where the PDO `SQLite` driver stands).
    pub const PDO_EXCEPTION: &'static str = "Doctrine\\DBAL\\Driver\\PDO\\Exception";
    pub const PDO_INVALID_CONFIGURATION: &'static str =
        "Doctrine\\DBAL\\Driver\\PDO\\Exception\\InvalidConfiguration";
    pub const IDENTITY_COLUMNS_NOT_SUPPORTED: &'static str =
        "Doctrine\\DBAL\\Driver\\Exception\\IdentityColumnsNotSupported";
    pub const NO_IDENTITY_VALUE: &'static str =
        "Doctrine\\DBAL\\Driver\\Exception\\NoIdentityValue";
    /// PHP's `\InvalidArgumentException` (not the DBAL one).
    pub const PHP_INVALID_ARGUMENT_EXCEPTION: &'static str = "InvalidArgumentException";
    /// PHP's `\LogicException`.
    pub const LOGIC_EXCEPTION: &'static str = "LogicException";
    /// PHP's `\ValueError`.
    pub const VALUE_ERROR: &'static str = "ValueError";
    /// PHP's `\Error`.
    pub const ERROR: &'static str = "Error";

    /// `getCode()`.
    pub fn code(&self) -> i64 {
        self.code
    }

    /// Sets the code (the constructor's `$code`).
    #[must_use]
    pub fn with_code(mut self, code: i64) -> Self {
        self.code = code;
        self
    }

    /// `getSQLState()`: the `SQLSTATE` the driver was in at the time the
    /// error occurred, if any. A `DriverException` reads it from the
    /// driver exception it wraps.
    pub fn sql_state(&self) -> Option<&str> {
        if self.is_instance_of(Self::DRIVER_EXCEPTION) {
            return self.previous.as_deref().and_then(Exception::sql_state);
        }
        self.sql_state.as_deref()
    }

    /// `getQuery()` of a `DriverException`: the SQL query that triggered
    /// the exception, if any.
    pub fn query(&self) -> Option<&Query> {
        self.query.as_deref()
    }

    /// `new AbstractException($message, $sqlState, $code)` for a driver
    /// exception class.
    pub fn driver(
        class: &'static str,
        message: impl Into<PhpString>,
        sql_state: Option<String>,
        code: i64,
    ) -> Self {
        let mut e = Self::new(class, message).with_code(code);
        e.sql_state = sql_state;
        e
    }

    /// `new DriverException($driverException, $query)` with the class of
    /// the subclass: the message says whether a query was executing, the
    /// code is the driver exception's.
    pub fn driver_exception(class: &'static str, driver: Exception, query: Option<Query>) -> Self {
        let prefix: &[u8] = if query.is_some() {
            b"An exception occurred while executing a query: "
        } else {
            b"An exception occurred in the driver: "
        };
        let mut message = prefix.to_vec();
        message.extend_from_slice(driver.message().as_bytes());
        let code = driver.code();
        let mut e = Self::new(class, message)
            .with_code(code)
            .with_previous(driver);
        e.query = query.map(Box::new);
        e
    }

    /// Attaches `previous` at the end of this exception's chain, as PHP
    /// does when an exception is thrown while another one propagates out
    /// of a `finally` block.
    #[must_use]
    pub fn chain(mut self, previous: Exception) -> Self {
        match self.previous.take() {
            Some(inner) => self.previous = Some(Box::new(inner.chain(previous))),
            None => self.previous = Some(Box::new(previous)),
        }
        self
    }

    /// `NoActiveTransaction::new()`.
    pub fn no_active_transaction() -> Self {
        Self::new(
            Self::NO_ACTIVE_TRANSACTION,
            "There is no active transaction.",
        )
    }

    /// `CommitFailedRollbackOnly::new()`.
    pub fn commit_failed_rollback_only() -> Self {
        Self::new(
            Self::COMMIT_FAILED_ROLLBACK_ONLY,
            "Transaction commit failed because the transaction has been marked for rollback only.",
        )
    }

    /// `SavepointsNotSupported::new()`.
    pub fn savepoints_not_supported() -> Self {
        Self::new(
            Self::SAVEPOINTS_NOT_SUPPORTED,
            "Savepoints are not supported by this driver.",
        )
    }

    /// `DriverRequired::new($url)`.
    pub fn driver_required(url: Option<&[u8]>) -> Self {
        match url {
            Some(url) => {
                let mut message = b"The options \"driver\" or \"driverClass\" are mandatory if a connection URL without scheme is given to DriverManager::getConnection(). Given URL \"".to_vec();
                message.extend_from_slice(url);
                message.extend_from_slice(b"\".");
                Self::new(Self::DRIVER_REQUIRED, message)
            }
            None => Self::new(
                Self::DRIVER_REQUIRED,
                "The options \"driver\" or \"driverClass\" are mandatory if no PDO instance is given to DriverManager::getConnection().",
            ),
        }
    }

    /// `UnknownDriver::new($unknownDriverName, $knownDrivers)`.
    pub fn unknown_driver(unknown_driver_name: &[u8], known_drivers: &[&str]) -> Self {
        let mut message = b"The given driver \"".to_vec();
        message.extend_from_slice(unknown_driver_name);
        message.extend_from_slice(
            b"\" is unknown, Doctrine currently supports only the following drivers: ",
        );
        message.extend_from_slice(known_drivers.join(", ").as_bytes());
        Self::new(Self::UNKNOWN_DRIVER, message)
    }

    /// `InvalidDriverClass::new($driverClass)`.
    pub fn invalid_driver_class(driver_class: &[u8]) -> Self {
        let mut message = b"The given driver class ".to_vec();
        message.extend_from_slice(driver_class);
        message.extend_from_slice(b" has to implement the Doctrine\\DBAL\\Driver interface.");
        Self::new(Self::INVALID_DRIVER_CLASS, message)
    }

    /// `InvalidWrapperClass::new($wrapperClass)`.
    pub fn invalid_wrapper_class(wrapper_class: &[u8]) -> Self {
        let mut message = b"The given wrapper class ".to_vec();
        message.extend_from_slice(wrapper_class);
        message.extend_from_slice(b" has to be a subtype of Doctrine\\DBAL\\Connection.");
        Self::new(Self::INVALID_WRAPPER_CLASS, message)
    }

    /// `MalformedDsnException::new()`.
    pub fn malformed_dsn() -> Self {
        Self::new(
            Self::MALFORMED_DSN_EXCEPTION,
            "Malformed database connection URL",
        )
    }

    /// `NoKeyValue::fromColumnCount($columnCount)`.
    pub fn no_key_value(column_count: usize) -> Self {
        Self::new(
            Self::NO_KEY_VALUE,
            format!(
                "Fetching as key-value pairs requires the result to contain at least 2 columns, {column_count} given."
            ),
        )
    }

    /// `InvalidColumnIndex::new($index, $previous)`.
    pub fn invalid_column_index(index: i64, previous: Option<Exception>) -> Self {
        let e = Self::new(
            Self::INVALID_COLUMN_INDEX,
            format!("Invalid column index \"{index}\"."),
        );
        match previous {
            Some(previous) => e.with_previous(previous),
            None => e,
        }
    }

    /// `ParseError::fromParserException($exception)`.
    pub fn parse_error(previous: Exception) -> Self {
        Self::new(Self::PARSE_ERROR, "Unable to parse query.").with_previous(previous)
    }

    /// `DatabaseRequired::new($methodName)`.
    pub fn database_required(method_name: &str) -> Self {
        Self::new(
            Self::DATABASE_REQUIRED,
            format!("A database is required for the method: {method_name}."),
        )
    }

    /// `IdentityColumnsNotSupported::new($previous)`.
    pub fn identity_columns_not_supported(previous: Option<Exception>) -> Self {
        let e = Self::driver(
            Self::IDENTITY_COLUMNS_NOT_SUPPORTED,
            "The driver does not support identity columns.",
            None,
            0,
        );
        match previous {
            Some(previous) => e.with_previous(previous),
            None => e,
        }
    }

    /// `NoIdentityValue::new($previous)`.
    pub fn no_identity_value(previous: Option<Exception>) -> Self {
        let e = Self::driver(
            Self::NO_IDENTITY_VALUE,
            "No identity value was generated by the last statement.",
            None,
            0,
        );
        match previous {
            Some(previous) => e.with_previous(previous),
            None => e,
        }
    }
}
