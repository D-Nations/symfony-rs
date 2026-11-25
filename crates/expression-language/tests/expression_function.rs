//! ExpressionFunctionTest.php at symfony/expression-language v8.1.9. The
//! PHP functions `fromPhp()` can find are a `PhpFunctions` table.
//!
//! Upstream: symfony/expression-language Tests/ExpressionFunctionTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use php_value::{Value, constants::Registry};
use symfony_expression_language::{Class, ExpressionFunction, PhpFunctions};

/// PHP's functions, with the test file's `fn_namespaced()`.
fn php_functions() -> PhpFunctions {
    let mut php = PhpFunctions::builtin(Arc::new(Registry::with_core()));
    php.define(
        "Symfony\\Component\\ExpressionLanguage\\Tests\\fn_namespaced",
        |_| Ok(Value::Null),
    );
    php
}

#[test]
fn testFunctionDoesNotExist() {
    let error =
        ExpressionFunction::from_php("fn_does_not_exist", None, &php_functions()).unwrap_err();
    assert_eq!(error.class(), &Class::InvalidArgumentException);
    assert_eq!(
        error.message().to_string_lossy(),
        "PHP function \"fn_does_not_exist\" does not exist."
    );
}

#[test]
fn testFunctionNamespaced() {
    let error = ExpressionFunction::from_php(
        "Symfony\\Component\\ExpressionLanguage\\Tests\\fn_namespaced",
        None,
        &php_functions(),
    )
    .unwrap_err();
    assert_eq!(error.class(), &Class::InvalidArgumentException);
    assert_eq!(
        error.message().to_string_lossy(),
        "An expression function name must be defined when PHP function \"Symfony\\Component\\ExpressionLanguage\\Tests\\fn_namespaced\" is namespaced."
    );
}
