//! ExpressionBuilderTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the provider rows are upstream's, recorded by running its
//! providers (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Query/Expression/ExpressionBuilderTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use common::{StubPlatform, TestConnection, expression_parts, provider, text};
use doctrine_dbal::ConnectionStandIn;
use doctrine_dbal::query::expression::ExpressionBuilder;

type Expr<'c> = ExpressionBuilder<'c, TestConnection<StubPlatform>>;

/// `setUp()`: a builder on a connection stub.
fn conn() -> TestConnection<StubPlatform> {
    TestConnection {
        platform: StubPlatform,
    }
}

#[test]
fn testAnd() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    let rows = provider("Query/Expression/ExpressionBuilderTest", "testAnd");
    assert_eq!(rows.len(), 6);
    for (name, args) in rows {
        let mut parts = expression_parts(&args[0]).into_iter();
        let first = parts.next().expect("a part");

        let composite = expr.and(first, parts);

        assert_eq!(text(&args[1]), composite.to_string(), "{name}");
    }
}

#[test]
fn testOr() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    let rows = provider("Query/Expression/ExpressionBuilderTest", "testOr");
    assert_eq!(rows.len(), 6);
    for (name, args) in rows {
        let mut parts = expression_parts(&args[0]).into_iter();
        let first = parts.next().expect("a part");

        let composite = expr.or(first, parts);

        assert_eq!(text(&args[1]), composite.to_string(), "{name}");
    }
}

#[test]
fn testComparison() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    let rows = provider("Query/Expression/ExpressionBuilderTest", "testComparison");
    assert_eq!(rows.len(), 6);
    for (name, args) in rows {
        let part = expr.comparison(&text(&args[0]), &text(&args[1]), &text(&args[2]));

        assert_eq!(text(&args[3]), part, "{name}");
    }

    // The provider's operators are ExpressionBuilder's constants.
    assert_eq!(
        [
            Expr::EQ,
            Expr::NEQ,
            Expr::LT,
            Expr::LTE,
            Expr::GT,
            Expr::GTE
        ],
        ["=", "<>", "<", "<=", ">", ">="]
    );
}

#[test]
fn testEq() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.user_id = 1", expr.eq("u.user_id", "1"));
}

#[test]
fn testNeq() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.user_id <> 1", expr.neq("u.user_id", "1"));
}

#[test]
fn testLt() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.salary < 10000", expr.lt("u.salary", "10000"));
}

#[test]
fn testLte() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.salary <= 10000", expr.lte("u.salary", "10000"));
}

#[test]
fn testGt() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.salary > 10000", expr.gt("u.salary", "10000"));
}

#[test]
fn testGte() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.salary >= 10000", expr.gte("u.salary", "10000"));
}

#[test]
fn testIsNull() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.deleted IS NULL", expr.is_null("u.deleted"));
}

#[test]
fn testIsNotNull() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!("u.updated IS NOT NULL", expr.is_not_null("u.updated"));
}

#[test]
fn testIn() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "u.groups IN (1, 3, 4, 7)",
        expr.r#in("u.groups", &["1", "3", "4", "7"])
    );
}

#[test]
fn testInWithPlaceholder() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    // PHP passes the string '?'; `(array) '?'` is `['?']`.
    assert_eq!("u.groups IN (?)", expr.r#in("u.groups", &["?"]));
}

#[test]
fn testNotIn() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "u.groups NOT IN (1, 3, 4, 7)",
        expr.not_in("u.groups", &["1", "3", "4", "7"])
    );
}

#[test]
fn testNotInWithPlaceholder() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "u.groups NOT IN (:values)",
        expr.not_in("u.groups", &[":values"])
    );
}

#[test]
fn testLikeWithoutEscape() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "a.song LIKE 'a virgin'",
        expr.like("a.song", "'a virgin'", None)
    );
}

#[test]
fn testLikeWithEscape() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "a.song LIKE 'a virgin' ESCAPE '💩'",
        expr.like("a.song", "'a virgin'", Some("'💩'"))
    );
}

#[test]
fn testNotLikeWithoutEscape() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "s.last_words NOT LIKE 'this'",
        expr.not_like("s.last_words", "'this'", None)
    );
}

#[test]
fn testNotLikeWithEscape() {
    let conn = conn();
    let expr = conn.create_expression_builder();
    assert_eq!(
        "p.description NOT LIKE '20💩%' ESCAPE '💩'",
        expr.not_like("p.description", "'20💩%'", Some("'💩'"))
    );
}
