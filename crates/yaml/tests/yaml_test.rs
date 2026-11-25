//! YamlTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//!
//! Each test has its upstream name and replays every call the upstream
//! test makes to the parser and the dumper, for each of its data-provider
//! rows, from `tests/data/yaml_test.json` (common/replay.rs says how).
//!
//! Upstream: symfony/yaml Tests/YamlTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::LazyLock;

use common::replay::{self, TestResult};
use serde_json::Value as Json;

fn recording() -> &'static Json {
    static DATA: LazyLock<Json> =
        LazyLock::new(|| replay::load(include_str!("data/yaml_test.json")));
    &DATA
}

fn replay(test: &str) -> TestResult {
    replay::replay(recording(), test)
}

const REPLAYED: &[&str] = &[
    "testNegativeIndentationThrowsException",
    "testParseAllowsConfiguringTheMaximumNestingLevel",
    "testParseAndDump",
    "testParseFileAllowsConfiguringTheMaximumCollectionAliasCount",
    "testZeroIndentationThrowsException",
];

#[test]
fn every_upstream_test_is_here() {
    assert_eq!(replay::names(recording()), REPLAYED);
}

#[test]
fn testNegativeIndentationThrowsException() -> TestResult {
    replay("testNegativeIndentationThrowsException")
}

#[test]
fn testParseAllowsConfiguringTheMaximumNestingLevel() -> TestResult {
    replay("testParseAllowsConfiguringTheMaximumNestingLevel")
}

#[test]
fn testParseAndDump() -> TestResult {
    replay("testParseAndDump")
}

#[test]
fn testParseFileAllowsConfiguringTheMaximumCollectionAliasCount() -> TestResult {
    replay("testParseFileAllowsConfiguringTheMaximumCollectionAliasCount")
}

#[test]
fn testZeroIndentationThrowsException() -> TestResult {
    replay("testZeroIndentationThrowsException")
}
