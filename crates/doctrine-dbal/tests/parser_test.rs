//! ParserTest.php at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879.
//! Names and assertions are PHPUnit's; the provider rows are upstream's,
//! recorded by running its providers (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/SQL/ParserTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use std::convert::Infallible;

use common::{bool, bytes, provider};
use doctrine_dbal::sql::{Parser, Visitor};

/// The test class is the visitor: parameters are kept as `{%s}`.
#[derive(Default)]
struct ParserTest {
    result: Vec<u8>,
}

impl Visitor for ParserTest {
    type Error = Infallible;

    fn accept_positional_parameter(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.result.push(b'{');
        self.result.extend_from_slice(sql);
        self.result.push(b'}');
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.result.push(b'{');
        self.result.extend_from_slice(sql);
        self.result.push(b'}');
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.result.extend_from_slice(sql);
        Ok(())
    }
}

fn assert_parsed(my_sql_string_escaping: bool, sql: &[u8], expected: &[u8], name: &str) {
    let mut test = ParserTest::default();
    let Ok(()) = Parser::new(my_sql_string_escaping).parse(sql, &mut test);
    assert_eq!(
        String::from_utf8_lossy(&test.result),
        String::from_utf8_lossy(expected),
        "{name}"
    );
}

fn rows(method: &str, expected: usize) -> Vec<(String, Vec<serde_json::Value>)> {
    let rows = provider("SQL/ParserTest", method);
    assert_eq!(rows.len(), expected, "{method} rows");
    rows
}

#[test]
fn testStatementsWithParameters() {
    for (name, args) in rows("testStatementsWithParameters", 72) {
        assert_parsed(bool(&args[0]), &bytes(&args[1]), &bytes(&args[2]), &name);
    }
}

#[test]
fn testStatementsWithoutParameters() {
    for (name, args) in rows("testStatementsWithoutParameters", 20) {
        let sql = bytes(&args[1]);
        assert_parsed(bool(&args[0]), &sql, &sql, &name);
    }
}

#[test]
fn testAnsiEscaping() {
    for (name, args) in rows("testAnsiEscaping", 2) {
        assert_parsed(false, &bytes(&args[0]), &bytes(&args[1]), &name);
    }
}

#[test]
fn testMySQLEscaping() {
    for (name, args) in rows("testMySQLEscaping", 2) {
        assert_parsed(true, &bytes(&args[0]), &bytes(&args[1]), &name);
    }
}
