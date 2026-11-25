//! CompositeExpressionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the provider rows are upstream's, recorded by running its
//! provider (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Query/Expression/CompositeExpressionTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use common::{expression_part, provider, text};
use doctrine_dbal::query::expression::{CompositeExpression, ExpressionPart};

#[test]
fn testCount() {
    let expr = CompositeExpression::or("u.group_id = 1", []);

    assert_eq!(1, expr.count());

    let expr = expr.with("u.group_id = 2", []);

    assert_eq!(2, expr.count());
}

#[test]
fn testWith() {
    let expr = CompositeExpression::or("u.group_id = 1", []);

    assert_eq!(1, expr.count());

    // test immutability
    let _ = expr.with(CompositeExpression::or("u.user_id = 1", []), []);

    assert_eq!(1, expr.count());

    let expr = expr.with(CompositeExpression::or("u.user_id = 1", []), []);

    assert_eq!(2, expr.count());

    let expr = expr.with("u.user_id = 1", []);

    assert_eq!(3, expr.count());
}

#[test]
fn testCompositeUsageAndGeneration() {
    let rows = provider(
        "Query/Expression/CompositeExpressionTest",
        "testCompositeUsageAndGeneration",
    );
    assert_eq!(rows.len(), 6);
    for (name, args) in rows {
        let ExpressionPart::Composite(expr) = expression_part(&args[0]) else {
            panic!("{name}: not a composite expression");
        };
        let expects = text(&args[1]);

        assert_eq!(expects, expr.to_string(), "{name}");
    }
}
