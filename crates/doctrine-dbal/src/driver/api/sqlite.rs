//! Port of Doctrine\DBAL\Driver\API\SQLite\ExceptionConverter.php
//! (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/API/SQLite/ExceptionConverter.php

use super::ExceptionConverter as ExceptionConverterInterface;
use crate::{Exception, Query};

/// The `SQLite` exception converter: it reads the message
/// (<http://www.sqlite.org/c3ref/c_abort.html>).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExceptionConverter;

/// `str_contains()` on bytes.
fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

impl ExceptionConverterInterface for ExceptionConverter {
    fn convert(&self, exception: Exception, query: Option<Query>) -> Exception {
        let message = exception.message().as_bytes();
        let class = if contains(message, "database is locked") {
            Exception::LOCK_WAIT_TIMEOUT_EXCEPTION
        } else if contains(message, "must be unique")
            || contains(message, "is not unique")
            || contains(message, "are not unique")
            || contains(message, "UNIQUE constraint failed")
        {
            Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION
        } else if contains(message, "may not be NULL")
            || contains(message, "NOT NULL constraint failed")
        {
            Exception::NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION
        } else if contains(message, "no such table:") {
            Exception::TABLE_NOT_FOUND_EXCEPTION
        } else if contains(message, "already exists") {
            Exception::TABLE_EXISTS_EXCEPTION
        } else if contains(message, "has no column named") {
            Exception::INVALID_FIELD_NAME_EXCEPTION
        } else if contains(message, "ambiguous column name") {
            Exception::NON_UNIQUE_FIELD_NAME_EXCEPTION
        } else if contains(message, "syntax error") {
            Exception::SYNTAX_ERROR_EXCEPTION
        } else if contains(message, "attempt to write a readonly database") {
            Exception::READ_ONLY_EXCEPTION
        } else if contains(message, "unable to open database file") {
            Exception::CONNECTION_EXCEPTION_DRIVER
        } else if contains(message, "FOREIGN KEY constraint failed") {
            Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION
        } else {
            Exception::DRIVER_EXCEPTION
        };
        Exception::driver_exception(class, exception, query)
    }
}
