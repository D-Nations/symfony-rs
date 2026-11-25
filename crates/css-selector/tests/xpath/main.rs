//! The XPath tests at symfony/css-selector v8.1.6.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

#[path = "../common/mod.rs"]
mod common;

mod translator_test;
