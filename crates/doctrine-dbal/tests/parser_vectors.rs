//! The parser and the list-parameter expansion against real PHP
//! (tests/data/php.json, `scripts/record.sh doctrine-dbal`): generated
//! statements parsed in ANSI and MySQL mode, compared call by call, and
//! expansion cases the upstream tests leave open.
//!
//! PHP refuses some statements at a PCRE limit (README.md, Differences).
//! For those, the check is that the statement is one of the documented
//! cases; any other difference fails.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod common;

use std::convert::Infallible;

use common::{array, bytes, data, types};
use doctrine_dbal::sql::{Parser, Visitor};
use doctrine_dbal::{ExpandArrayParameters, WrapperParameterType};
use php_value::vectors::{base64, decode, phpunit_equals};
use php_value::{PhpArray, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Named(Vec<u8>),
    Positional(Vec<u8>),
    Other(Vec<u8>),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
}

impl Visitor for Recorder {
    type Error = Infallible;

    fn accept_positional_parameter(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.calls.push(Call::Positional(sql.to_vec()));
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.calls.push(Call::Named(sql.to_vec()));
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Infallible> {
        self.calls.push(Call::Other(sql.to_vec()));
        Ok(())
    }
}

fn parse(sql: &[u8], my_sql: bool) -> Vec<Call> {
    let mut recorder = Recorder::default();
    let Ok(()) = Parser::new(my_sql).parse(sql, &mut recorder);
    recorder.calls
}

/// An `/*` that the Rust parser read as a lone `/`: no `*/` closes it.
fn unterminated_comment(calls: &[Call]) -> bool {
    calls.windows(2).any(|pair| {
        matches!(&pair[0], Call::Other(slash) if slash == b"/")
            && matches!(&pair[1], Call::Other(next) if next.first() == Some(&b'*'))
    })
}

#[test]
fn parser_matches_php() {
    let rows = data()["parser"].as_array().expect("parser rows");
    let (mut matched, mut backtrack, mut jit) = (0, 0, 0);
    for row in rows {
        let sql = base64(row[0].as_str().expect("sql")).expect("base64");
        for (my_sql, recorded) in [(false, &row[1]), (true, &row[2])] {
            let calls = parse(&sql, my_sql);
            let shown = String::from_utf8_lossy(&sql[..sql.len().min(80)]).into_owned();
            if recorded[0] == "throws" {
                let message = recorded[2].as_str().expect("message");
                match message {
                    "Backtrack limit exhausted" => {
                        assert!(
                            unterminated_comment(&calls),
                            "{shown:?} (MySQL {my_sql}): {message}"
                        );
                        backtrack += 1;
                    }
                    "JIT stack limit exhausted" => {
                        let longest = calls
                            .iter()
                            .map(|c| match c {
                                Call::Other(s) | Call::Named(s) | Call::Positional(s) => s.len(),
                            })
                            .max()
                            .unwrap_or(0);
                        assert!(
                            longest >= 6000 || unterminated_comment(&calls),
                            "{shown:?} (MySQL {my_sql}): {message}"
                        );
                        jit += 1;
                    }
                    other => panic!("{shown:?} (MySQL {my_sql}): unexpected PHP error {other}"),
                }
                continue;
            }
            let expected: Vec<Call> = recorded
                .as_array()
                .expect("calls")
                .iter()
                .map(|call| {
                    let fragment = base64(call[1].as_str().expect("fragment")).expect("base64");
                    match call[0].as_str() {
                        Some("named") => Call::Named(fragment),
                        Some("positional") => Call::Positional(fragment),
                        _ => Call::Other(fragment),
                    }
                })
                .collect();
            assert_eq!(calls, expected, "{shown:?} (MySQL {my_sql})");
            matched += 1;
        }
    }
    eprintln!(
        "parser: {matched} parses match PHP; PHP refused {backtrack} (backtrack limit) and {jit} (JIT stack)"
    );
    assert!(matched > 5000, "{matched}");
}

#[test]
fn expansion_matches_php() {
    let rows = data()["expand"].as_array().expect("expand rows");
    assert_eq!(rows.len(), 15);
    for row in rows {
        let sql = bytes(&row[0]);
        let params = array(&row[1]);
        let types_ = types(&row[2]);
        let mut visitor = ExpandArrayParameters::new(&params, &types_);
        let result = Parser::new(true).parse(&sql, &mut visitor);
        let shown = String::from_utf8_lossy(&sql).into_owned();
        let recorded = &row[3];
        if recorded[0] == "throws" {
            let error = result.expect_err(&shown);
            let message = recorded[2].as_str().expect("message");
            if recorded[1] == "TypeError" && message.contains("appendTypedParameter()") {
                // A \Countable object: README.md, Differences.
                assert_eq!(error.class(), "TypeError", "{shown}");
                continue;
            }
            assert_eq!(
                error.class(),
                recorded[1].as_str().expect("class"),
                "{shown}"
            );
            assert_eq!(error.message().as_bytes(), message.as_bytes(), "{shown}");
            continue;
        }
        result.unwrap_or_else(|e| panic!("{shown}: {e}"));
        let Value::Array(expected) = decode(recorded).expect("result") else {
            panic!("{shown}: not an array");
        };
        let mut expected = expected.values();
        let expected_sql = expected.next().expect("sql");
        assert_eq!(expected_sql, &Value::String(visitor.get_sql()), "{shown}");
        assert!(
            phpunit_equals(
                expected.next().expect("params"),
                &Value::Array(PhpArray::list(visitor.get_parameters().to_vec()))
            ),
            "{shown}: parameters"
        );
        let types_json = &row[3][1][2][1];
        let expected_types: Vec<(usize, WrapperParameterType)> = types(types_json)
            .into_iter()
            .map(|(k, v)| (usize::try_from(k.as_int().expect("int")).expect("index"), v))
            .collect();
        let actual: Vec<(usize, WrapperParameterType)> = visitor
            .get_types()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        assert_eq!(expected_types, actual, "{shown}: types");
    }
}
