//! YamlTestSuiteTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//!
//! Each test has its upstream name and replays every call the upstream
//! test makes to the parser and the dumper, for each of its data-provider
//! rows, from `tests/data/yaml_test_suite_test.json` (common/replay.rs says how).
//!
//! The suite is yaml/yaml-test-suite at 6ad3d2c6 (2022.01.17, symfony/yaml's
//! require-dev; MIT, see tests/yaml-test-suite/License): the `in.yaml` of
//! each case upstream runs is in tests/yaml-test-suite. The cases upstream
//! skips (its SKIP list) are skipped here too.
//!
//! Upstream: symfony/yaml Tests/YamlTestSuiteTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::LazyLock;

use common::replay::{self, TestResult};
use serde_json::Value as Json;

fn recording() -> &'static Json {
    static DATA: LazyLock<Json> =
        LazyLock::new(|| replay::load(include_str!("data/yaml_test_suite_test.json")));
    &DATA
}

fn replay(test: &str) -> TestResult {
    replay::replay(recording(), test)
}

const REPLAYED: &[&str] = &["testYamlTestSuite"];

#[test]
fn every_upstream_test_is_here() {
    assert_eq!(replay::names(recording()), REPLAYED);
}

#[test]
fn testYamlTestSuite() -> TestResult {
    replay("testYamlTestSuite")
}
