//! ExpressionLanguageTest.php at symfony/expression-language v8.1.9.
//!
//! The tests that `eval()` a compiled expression compare the compiled
//! source with the source PHP 8.4.27 ran, and PHP's result with upstream's
//! assertion (`tests/data/php.json`, `compiled`). The recorder's `ElEnum`
//! and `ElObj` stand in for the `FooEnum` fixture and the anonymous
//! classes there.
//!
//! Upstream: symfony/expression-language Tests/ExpressionLanguageTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_names,
    clippy::cast_possible_truncation
)]

mod common;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use common::vectors::{b64, check, check_text, data, decode, same};
use php_value::{
    Key, PhpArray, PhpString, Value,
    constants::Registry,
    object::{EnumCase, StdClass},
};
use serde_json::Value as Json;
use symfony_expression_language::{
    CacheItemPool, Class, Error, ExpressionFunction, ExpressionFunctionProvider,
    ExpressionLanguage, IGNORE_UNKNOWN_VARIABLES, MethodObject, ParsedExpression, PhpFunctions,
};

const FOO_ENUM: &str = "Symfony\\Component\\ExpressionLanguage\\Tests\\Fixtures\\FooEnum";
const FOO_BACKED_ENUM: &str =
    "Symfony\\Component\\ExpressionLanguage\\Tests\\Fixtures\\FooBackedEnum";

fn array(pairs: &[(&str, Value)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in pairs {
        array.insert(Key::from(*k), v.clone());
    }
    array
}

fn names(list: &[&str]) -> PhpArray {
    PhpArray::list(list.iter().map(|n| Value::from(*n)))
}

fn none() -> PhpArray {
    PhpArray::new()
}

/// The fixtures (`FooEnum`, `FooBackedEnum`) and `PHP_VERSION`, which
/// the PHP that runs the tests defines.
fn registry() -> Registry {
    let mut registry = Registry::with_core();
    registry.define("PHP_VERSION", Value::from("8.4.27"));
    registry.define_enum(FOO_ENUM, vec![EnumCase::new(FOO_ENUM, "Foo", None)]);
    registry.define_enum(
        FOO_BACKED_ENUM,
        vec![EnumCase::new(
            FOO_BACKED_ENUM,
            "Bar",
            Some(Value::from("Foo")),
        )],
    );
    registry
}

fn language() -> ExpressionLanguage {
    ExpressionLanguage::with_constants(None, registry())
}

fn expect_error(result: Result<impl std::fmt::Debug, Error>, class: &Class, message: &str) {
    let error = result.expect_err("an exception");
    assert!(error.is_a(class), "{error}");
    assert!(
        error.message().to_string_lossy().contains(message),
        "{error}"
    );
}

/// A mock pool: counts `getItem()` and `save()` and checks the key.
struct CountingPool {
    key: &'static str,
    gets: AtomicUsize,
    saves: AtomicUsize,
    saved: Mutex<Option<Arc<ParsedExpression>>>,
}

impl CountingPool {
    fn new(key: &'static str) -> Arc<Self> {
        Arc::new(Self {
            key,
            gets: AtomicUsize::new(0),
            saves: AtomicUsize::new(0),
            saved: Mutex::new(None),
        })
    }
}

impl CacheItemPool for CountingPool {
    fn get_item(&self, key: &[u8]) -> Option<Arc<ParsedExpression>> {
        assert_eq!(key, self.key.as_bytes());
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.saved.lock().unwrap().clone()
    }

    fn save(&self, key: &[u8], value: Arc<ParsedExpression>) -> bool {
        assert_eq!(key, self.key.as_bytes());
        self.saves.fetch_add(1, Ordering::SeqCst);
        *self.saved.lock().unwrap() = Some(value);
        true
    }
}

#[test]
fn testCachedParse() {
    let pool = CountingPool::new("1%20%2B%201%2F%2F");
    let expression_language =
        ExpressionLanguage::with(Some(Arc::clone(&pool) as Arc<dyn CacheItemPool>), &[]).unwrap();

    let parsed_expression = expression_language.parse("1 + 1", &none(), 0).unwrap();
    let saved = pool.saved.lock().unwrap().clone().unwrap();
    assert!(Arc::ptr_eq(&saved, &parsed_expression));

    let parsed_expression = expression_language.parse("1 + 1", &none(), 0).unwrap();
    assert!(Arc::ptr_eq(&saved, &parsed_expression));

    assert_eq!(pool.gets.load(Ordering::SeqCst), 2);
    assert_eq!(pool.saves.load(Ordering::SeqCst), 1);
}

#[test]
fn testBasicPhpFunction() {
    for (expression, expected, compiled) in [
        (
            "constant(\"PHP_VERSION\")",
            Value::from("8.4.27"),
            "\\constant(\"PHP_VERSION\")",
        ),
        ("min(1,2,3)", Value::Int(1), "\\min(1, 2, 3)"),
        ("max(1,2,3)", Value::Int(3), "\\max(1, 2, 3)"),
    ] {
        let expression_language = language();
        assert!(
            expression_language
                .evaluate(expression, &none())
                .unwrap()
                .identical(&expected)
        );
        assert_eq!(
            expression_language.compile(expression, &none()).unwrap(),
            compiled
        );
    }
}

#[test]
fn testEnumFunctionWithConstantThrows() {
    let expression_language = language();
    expect_error(
        expression_language.evaluate("enum(\"PHP_VERSION\")", &none()),
        &Class::TypeError,
        "The string \"PHP_VERSION\" is not the name of a valid enum case.",
    );
}

/// The row of `compiled` for `expression`, `names` and, when given, the
/// variable `foo`: asserts that our compiled source is the one PHP ran,
/// and returns PHP's outcome and warnings.
fn php_eval(expression: &str, names: &PhpArray, foo: Option<&Value>) -> (Json, Vec<String>) {
    let data = data();
    let row = data["compiled"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            b64(&row[0]) == expression.as_bytes()
                && same(&decode(&row[1]), &Value::Array(names.clone()))
                && foo.is_none_or(|foo| {
                    row[2]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|pair| pair[0] == "foo" && same(&decode(&pair[1]), foo))
                })
        })
        .unwrap_or_else(|| panic!("no recorded eval of {expression}"))
        .clone();
    let compiled = common::vectors::language().compile(expression, names);
    if let Some(difference) = check_text(&row[3], &compiled) {
        panic!("{expression}: {difference}");
    }
    let warnings = row[5]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| common::vectors::text(&data["warnings"][i.as_u64().unwrap() as usize]))
        .collect();
    (row[4].clone(), warnings)
}

fn assert_php_returned(outcome: &Json, expected: Value) {
    if let Some(difference) = check(outcome, &Ok(expected)) {
        panic!("{difference}");
    }
}

#[test]
fn testCompiledEnumFunctionWithConstantThrows() {
    let (outcome, _) = php_eval("enum(\"PHP_VERSION\")", &none(), None);
    let expected = Err(Error::new(
        Class::TypeError,
        "The string \"PHP_VERSION\" is not the name of a valid enum case.",
    ));
    assert_eq!(check(&outcome, &expected), None);
}

fn foo_enum() -> Value {
    registry()
        .constant(&format!("{FOO_ENUM}::Foo"))
        .unwrap()
        .clone()
}

fn el_enum_one() -> Value {
    common::vectors::registry()
        .constant("ElEnum::One")
        .unwrap()
        .clone()
}

#[test]
fn testEnumFunction() {
    let expression_language = language();
    let value = expression_language
        .evaluate(
            "enum(\"Symfony\\\\Component\\\\ExpressionLanguage\\\\Tests\\\\Fixtures\\\\FooEnum::Foo\")",
            &none(),
        )
        .unwrap();
    assert!(same(&foo_enum(), &value));
}

#[test]
fn testCompiledEnumFunction() {
    let (outcome, _) = php_eval("enum(\"ElEnum::One\")", &none(), None);
    assert_php_returned(&outcome, el_enum_one());
}

#[test]
fn testBackedEnumFunction() {
    let expression_language = language();
    let value = expression_language
        .evaluate(
            "enum(\"Symfony\\\\Component\\\\ExpressionLanguage\\\\Tests\\\\Fixtures\\\\FooBackedEnum::Bar\")",
            &none(),
        )
        .unwrap();
    let bar = registry()
        .constant(&format!("{FOO_BACKED_ENUM}::Bar"))
        .unwrap()
        .clone();
    assert!(same(&bar, &value));
    assert_eq!(
        expression_language
            .evaluate(
                "enum(\"Symfony\\\\Component\\\\ExpressionLanguage\\\\Tests\\\\Fixtures\\\\FooBackedEnum::Bar\").value",
                &none(),
            )
            .unwrap(),
        Value::from("Foo")
    );
}

#[test]
fn testCompiledEnumFunctionWithBackedEnum() {
    let (outcome, _) = php_eval("enum(\"ElBacked::Bar\")", &none(), None);
    let bar = common::vectors::registry()
        .constant("ElBacked::Bar")
        .unwrap()
        .clone();
    assert_php_returned(&outcome, bar);
}

#[test]
fn testCompiledEnumFunctionInsideExpression() {
    let r#enum = "enum(\"ElEnum::One\")";
    let foo = Some(Value::Null);
    let v = Value::from("bar");
    let (outcome, _) = php_eval(&format!("{enum} == foo"), &names(&["foo"]), foo.as_ref());
    assert_php_returned(&outcome, Value::Bool(false));
    let (outcome, _) = php_eval(&format!("foo == {enum}"), &names(&["foo"]), foo.as_ref());
    assert_php_returned(&outcome, Value::Bool(false));
    let (outcome, _) = php_eval(&format!("{enum}.name ~ v"), &names(&["v"]), None);
    assert_php_returned(&outcome, Value::from("Onebar"));
    let (outcome, _) = php_eval(&format!("[{enum}, v]"), &names(&["v"]), None);
    assert_php_returned(
        &outcome,
        Value::Array(PhpArray::list([el_enum_one(), v.clone()])),
    );
    assert_eq!(v, Value::from("bar"));
}

/// `Fixtures/TestProvider`.
struct TestProvider;

impl ExpressionFunctionProvider for TestProvider {
    fn functions(&self) -> Result<Vec<ExpressionFunction>, Error> {
        let mut php = PhpFunctions::builtin(Arc::new(Registry::with_core()));
        php.define(
            "Symfony\\Component\\ExpressionLanguage\\Tests\\Fixtures\\fn_namespaced",
            |_| Ok(Value::Bool(true)),
        );
        Ok(vec![
            ExpressionFunction::new(
                "identity",
                |arguments: &[PhpString]| Ok(arguments[0].clone()),
                |_values: &PhpArray, arguments: &[Value]| Ok(arguments[0].clone()),
            ),
            ExpressionFunction::from_php("strtoupper", None, &php)?,
            ExpressionFunction::from_php("\\strtolower", None, &php)?,
            ExpressionFunction::from_php(
                "Symfony\\Component\\ExpressionLanguage\\Tests\\Fixtures\\fn_namespaced",
                Some("fn_namespaced"),
                &php,
            )?,
        ])
    }
}

#[test]
fn testProviders() {
    let provider = TestProvider;
    // 'array' and 'Traversable': both give the providers one by one
    let array: Vec<&dyn ExpressionFunctionProvider> = vec![&provider];
    let traversable: Vec<&dyn ExpressionFunctionProvider> =
        std::iter::once(&provider as &dyn ExpressionFunctionProvider).collect();
    for providers in [array, traversable] {
        let expression_language = ExpressionLanguage::with(None, &providers).unwrap();
        let evaluate = |e: &str| expression_language.evaluate(e, &none()).unwrap();
        let compile = |e: &str| expression_language.compile(e, &none()).unwrap();
        assert_eq!(evaluate("identity(\"foo\")"), Value::from("foo"));
        assert_eq!(compile("identity(\"foo\")"), "\"foo\"");
        assert_eq!(evaluate("strtoupper(\"foo\")"), Value::from("FOO"));
        assert_eq!(compile("strtoupper(\"foo\")"), "\\strtoupper(\"foo\")");
        assert_eq!(evaluate("strtolower(\"FOO\")"), Value::from("foo"));
        assert_eq!(compile("strtolower(\"FOO\")"), "\\strtolower(\"FOO\")");
        assert_eq!(evaluate("fn_namespaced()"), Value::Bool(true));
        assert_eq!(
            compile("fn_namespaced()"),
            "\\Symfony\\Component\\ExpressionLanguage\\Tests\\Fixtures\\fn_namespaced()"
        );
    }
}

#[test]
fn testShortCircuitOperatorsEvaluate() {
    let object = MethodObject::new("class@anonymous")
        .method("foo", |_| panic!("object.foo() was called"))
        .into_value();
    for (expression, expected) in [
        ("false and object.foo()", false),
        ("false && object.foo()", false),
        ("true || object.foo()", true),
        ("true or object.foo()", true),
    ] {
        let expression_language = language();
        assert_eq!(
            expression_language
                .evaluate(expression, &array(&[("object", object.clone())]))
                .unwrap(),
            Value::Bool(expected)
        );
    }
}

#[test]
fn testShortCircuitOperatorsCompile() {
    for (expression, expected) in [
        ("false and foo", false),
        ("false && foo", false),
        ("true || foo", true),
        ("true or foo", true),
    ] {
        let names = array(&[("foo", Value::from("foo"))]);
        let (outcome, _) = php_eval(expression, &names, None);
        assert_php_returned(&outcome, Value::Bool(expected));
    }
}

#[test]
fn testParseThrowsInsteadOfNotice() {
    let expression_language = language();
    expect_error(
        expression_language.parse("node.", &names(&["node"]), 0),
        &Class::SyntaxError,
        "Unexpected end of expression around position 6 for expression `node.`.",
    );
}

#[test]
fn testParseReturnsObjectOnAlreadyParsedExpression() {
    let expression_language = language();
    let expression = expression_language.parse("1 + 1", &none(), 0).unwrap();
    assert!(Arc::ptr_eq(
        &expression,
        &expression_language.parse(&expression, &none(), 0).unwrap()
    ));
}

/// `['a', 'B' => 'b']` and `['B' => 'b', 'a']`.
fn a_and_b(b_first: bool) -> PhpArray {
    let mut names = PhpArray::new();
    if b_first {
        names.insert(Key::from("B"), Value::from("b"));
        names.insert(0, Value::from("a"));
    } else {
        names.insert(0, Value::from("a"));
        names.insert(Key::from("B"), Value::from("b"));
    }
    names
}

#[test]
fn testCachingForOverriddenVariableNames() {
    let expression_language = language();
    let expression = "a + b";
    expression_language
        .evaluate(
            expression,
            &array(&[("a", Value::Int(1)), ("b", Value::Int(1))]),
        )
        .unwrap();
    let result = expression_language
        .compile(expression, &a_and_b(false))
        .unwrap();
    assert_eq!(result, "($a + $B)");
}

#[test]
fn testStrictEquality() {
    let expression_language = language();
    let result = expression_language
        .compile("123 === a", &names(&["a"]))
        .unwrap();
    assert_eq!(result, "(123 === $a)");
}

#[test]
fn testCachingWithDifferentNamesOrder() {
    let pool = CountingPool::new("a%20%2B%20b%2F%2Fa%7CB%3Ab");
    let expression_language =
        ExpressionLanguage::with(Some(Arc::clone(&pool) as Arc<dyn CacheItemPool>), &[]).unwrap();
    let expression = "a + b";
    expression_language
        .compile(expression, &a_and_b(false))
        .unwrap();
    expression_language
        .compile(expression, &a_and_b(true))
        .unwrap();
    assert_eq!(pool.gets.load(Ordering::SeqCst), 2);
    assert_eq!(pool.saves.load(Ordering::SeqCst), 1);
}

#[test]
fn testOperatorCollisions() {
    let expression_language = language();
    let expression = "foo.not in [bar]";
    let compiled = expression_language
        .compile(expression, &names(&["foo", "bar"]))
        .unwrap();
    assert_eq!(compiled, "\\in_array($foo->not, [0 => $bar], true)");

    let result = expression_language
        .evaluate(
            expression,
            &array(&[
                (
                    "foo",
                    StdClass::object(array(&[("not", Value::from("test"))])),
                ),
                ("bar", Value::from("test")),
            ]),
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

type RegisterCallback = fn(&mut ExpressionLanguage) -> Result<(), Error>;

fn get_register_callbacks() -> Vec<RegisterCallback> {
    vec![
        |el| el.register("fn", |_| Ok(PhpString::default()), |_, _| Ok(Value::Null)),
        |el| {
            el.add_function(ExpressionFunction::new(
                "fn",
                |_: &[PhpString]| Ok(PhpString::default()),
                |_: &PhpArray, _: &[Value]| Ok(Value::Null),
            ))
        },
        |el| el.register_provider(&TestProvider),
    ]
}

#[test]
fn testRegisterAfterParse() {
    for register_callback in get_register_callbacks() {
        let mut el = language();
        el.parse("1 + 1", &none(), 0).unwrap();
        expect_error(register_callback(&mut el), &Class::LogicException, "");
    }
}

#[test]
fn testRegisterAfterEval() {
    for register_callback in get_register_callbacks() {
        let mut el = language();
        el.evaluate("1 + 1", &none()).unwrap();
        expect_error(register_callback(&mut el), &Class::LogicException, "");
    }
}

#[test]
fn testRegisterAfterCompile() {
    for register_callback in get_register_callbacks() {
        let mut el = language();
        el.compile("1 + 1", &none()).unwrap();
        expect_error(register_callback(&mut el), &Class::LogicException, "");
    }
}

/// The anonymous `stdClass` subclass whose `bar()` returns null.
fn foo_with_bar() -> Value {
    MethodObject::new("stdClass@anonymous")
        .method("bar", |_| Ok(Value::Null))
        .into_value()
}

fn object(pairs: &[(&str, Value)]) -> Value {
    StdClass::object(array(pairs))
}

fn provide_null_safe() -> Vec<(&'static str, Value)> {
    let bar_null = || object(&[("bar", Value::Null)]);
    let array_bar_null = || Value::Array(array(&[("bar", Value::Null)]));
    vec![
        ("foo?.bar", Value::Null),
        ("foo?.bar()", Value::Null),
        ("foo.bar?.baz", bar_null()),
        ("foo.bar?.baz()", bar_null()),
        ("foo[\"bar\"]?.baz", array_bar_null()),
        ("foo[\"bar\"]?.baz()", array_bar_null()),
        ("foo.bar()?.baz", foo_with_bar()),
        ("foo.bar()?.baz()", foo_with_bar()),
        ("foo?.[0]", Value::Null),
        ("foo?.[0].bar", Value::Null),
        ("foo?.bar.baz", Value::Null),
        ("foo?.bar[\"baz\"]", Value::Null),
        ("foo?.bar[\"baz\"][\"qux\"]", Value::Null),
        ("foo?.bar[\"baz\"][\"qux\"].quux", Value::Null),
        ("foo?.bar[\"baz\"][\"qux\"].quux()", Value::Null),
        ("foo?.bar().baz", Value::Null),
        ("foo?.bar()[\"baz\"]", Value::Null),
        ("foo?.bar()[\"baz\"][\"qux\"]", Value::Null),
        ("foo?.bar()[\"baz\"][\"qux\"].quux", Value::Null),
        ("foo?.bar()[\"baz\"][\"qux\"].quux()", Value::Null),
    ]
}

/// The expression the recorder ran: `ElObj::nothing()` stands in for
/// the anonymous class's `bar()`.
fn recorded(expression: &str, foo: &Value) -> (String, Option<Value>) {
    if expression.starts_with("foo.bar()") {
        (expression.replacen("foo.bar()", "foo.nothing()", 1), None)
    } else {
        (expression.to_owned(), Some(foo.clone()))
    }
}

fn foo_name() -> PhpArray {
    array(&[("foo", Value::from("foo"))])
}

#[test]
fn testNullSafeEvaluate() {
    for (expression, foo) in provide_null_safe() {
        let expression_language = language();
        assert_eq!(
            expression_language
                .evaluate(expression, &array(&[("foo", foo)]))
                .unwrap_or_else(|e| panic!("{expression}: {e}")),
            Value::Null,
            "{expression}"
        );
    }
}

#[test]
fn testNullSafeCompile() {
    for (expression, foo) in provide_null_safe() {
        let (expression, foo) = recorded(expression, &foo);
        let (outcome, _) = php_eval(&expression, &foo_name(), foo.as_ref());
        assert_php_returned(&outcome, Value::Null);
    }
}

fn provide_invalid_null_safe() -> Vec<(&'static str, Value, &'static str)> {
    vec![
        (
            "foo?.bar.baz",
            object(&[("bar", Value::Null)]),
            "Unable to get property \"baz\" of non-object \"foo?.bar\".",
        ),
        (
            "foo?.bar[\"baz\"]",
            object(&[("bar", Value::Null)]),
            "Unable to get an item of non-array \"foo?.bar\".",
        ),
        (
            "foo?.bar[\"baz\"].qux.quux",
            object(&[("bar", Value::Array(array(&[("baz", Value::Null)])))]),
            "Unable to get property \"qux\" of non-object \"foo?.bar[\"baz\"]\".",
        ),
        (
            "foo?.[0].bar",
            Value::Array(PhpArray::list([Value::Null])),
            "Unable to get property \"bar\" of non-object \"foo?.[0]\".",
        ),
    ]
}

#[test]
fn testNullSafeEvaluateFails() {
    for (expression, foo, message) in provide_invalid_null_safe() {
        let expression_language = language();
        expect_error(
            expression_language.evaluate(expression, &array(&[("foo", foo)])),
            &Class::RuntimeException,
            message,
        );
    }
}

/// PHP warns ("Attempt to read property ..." or "Trying to access ..."),
/// which upstream's error handler turns into an `ErrorException`.
#[test]
fn testNullSafeCompileFails() {
    for (expression, foo, _) in provide_invalid_null_safe() {
        let (_, warnings) = php_eval(expression, &foo_name(), Some(&foo));
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Attempt to read property") || w.contains("Trying to access")),
            "{expression}: {warnings:?}"
        );
    }
}

#[test]
fn testNullSafeEvaluateFailsWithReusedParsedExpression() {
    let expression_language = language();
    for (expression, foo, message) in provide_invalid_null_safe() {
        let parsed_expression = expression_language
            .parse(expression, &names(&["foo"]), 0)
            .unwrap();
        assert_eq!(
            expression_language
                .evaluate(&parsed_expression, &array(&[("foo", Value::Null)]))
                .unwrap(),
            Value::Null
        );
        match expression_language.evaluate(&parsed_expression, &array(&[("foo", foo)])) {
            Err(e) if e.is_a(&Class::RuntimeException) => {
                assert_eq!(e.message().to_string_lossy(), message);
            }
            other => panic!("Evaluating \"{expression}\" again should throw: {other:?}"),
        }
    }
}

fn provide_null_coalescing() -> Vec<(&'static str, Value)> {
    let bar_null = || object(&[("bar", Value::Null)]);
    let array_bar_null = || Value::Array(array(&[("bar", Value::Null)]));
    let mut nested = PhpArray::new();
    nested.insert(123, Value::Array(PhpArray::new()));
    vec![
        ("bar ?? \"default\"", Value::Null),
        ("foo.bar ?? \"default\"", Value::Null),
        ("foo.bar.baz ?? \"default\"", bar_null()),
        ("foo.bar ?? foo.baz ?? \"default\"", Value::Null),
        ("foo[0] ?? \"default\"", Value::Array(PhpArray::new())),
        ("foo[\"bar\"] ?? \"default\"", array_bar_null()),
        ("foo[\"baz\"] ?? \"default\"", array_bar_null()),
        ("foo[\"bar\"][\"baz\"] ?? \"default\"", array_bar_null()),
        ("foo[\"bar\"].baz ?? \"default\"", array_bar_null()),
        ("foo.bar().baz ?? \"default\"", foo_with_bar()),
        ("foo.bar.baz.bam ?? \"default\"", bar_null()),
        ("foo?.bar?.baz?.qux ?? \"default\"", bar_null()),
        ("foo[123][456][789] ?? \"default\"", Value::Array(nested)),
        ("foo.bar ?? \"default\"", Value::from("foo")),
        ("foo.bar ?? \"default\"", Value::Int(123)),
        (
            "foo.bar.baz ?? \"default\"",
            object(&[("bar", Value::from("foo"))]),
        ),
        ("foo[\"bar\"] ?? \"default\"", Value::from("foo")),
        ("foo[0] ?? \"default\"", Value::Int(123)),
        (
            "foo[0][\"bar\"] ?? \"default\"",
            Value::Array(PhpArray::list([Value::from("foo")])),
        ),
    ]
}

#[test]
fn testNullCoalescingEvaluate() {
    for (expression, foo) in provide_null_coalescing() {
        let expression_language = language();
        assert_eq!(
            expression_language
                .evaluate(expression, &array(&[("foo", foo)]))
                .unwrap_or_else(|e| panic!("{expression}: {e}")),
            Value::from("default"),
            "{expression}"
        );
    }
}

#[test]
fn testNullCoalescingCompile() {
    for (expression, foo) in provide_null_coalescing() {
        let (expression, foo) = recorded(expression, &foo);
        let (outcome, _) = php_eval(&expression, &foo_name(), foo.as_ref());
        assert_php_returned(&outcome, Value::from("default"));
    }
}

#[test]
fn testNullCoalescingEvaluateFailsOnObjectWithoutArrayAccess() {
    let expression_language = language();
    expect_error(
        expression_language.evaluate(
            "foo[\"bar\"] ?? \"default\"",
            &array(&[("foo", object(&[]))]),
        ),
        &Class::RuntimeException,
        "Unable to get an item of non-array \"foo\".",
    );
}

#[test]
fn testLintAllowsComments() {
    for expression in [
        "1 /* comment */ + 1",
        "1 /* /* comment with spaces */",
        "1 /** extra stars **/ + 1",
        "/* multi\nline */ 'foo'",
    ] {
        let el = language();
        el.lint(expression, &none(), 0)
            .unwrap_or_else(|e| panic!("{expression}: {e}"));
    }
}

#[test]
fn testLintThrowsOnInvalidComments() {
    for expression in [
        "1 + no start */",
        "1 /* no closing",
        "1 /* double closing */ */",
    ] {
        let el = language();
        expect_error(el.lint(expression, &none(), 0), &Class::SyntaxError, "");
    }
}

#[test]
fn testLintDoesntThrowOnValidExpression() {
    let el = language();
    el.lint("1 + 1", &none(), 0).unwrap();
}

#[test]
fn testLintThrowsOnInvalidExpression() {
    let el = language();
    expect_error(
        el.lint("node.", &names(&["node"]), 0),
        &Class::SyntaxError,
        "Unexpected end of expression around position 6 for expression `node.`.",
    );
}

#[test]
fn testCommentsIgnored() {
    let expression_language = language();
    assert_eq!(
        expression_language
            .evaluate("1 /* foo */ + 2", &none())
            .unwrap(),
        Value::Int(3)
    );
    assert_eq!(
        expression_language
            .compile("1 /* foo */ + 2", &none())
            .unwrap(),
        "(1 + 2)"
    );
}

#[test]
fn testParseDoesNotReuseAnExpressionParsedWithOtherFlags() {
    let expression_language = language();
    expression_language
        .parse("a + 1", &none(), IGNORE_UNKNOWN_VARIABLES)
        .unwrap();
    expect_error(
        expression_language.parse("a + 1", &none(), 0),
        &Class::SyntaxError,
        "Variable \"a\" is not valid around position 1 for expression `a + 1`.",
    );
}

#[test]
fn testParseAlreadyParsedExpressionReturnsSameObject() {
    let el = language();
    let parsed = el.parse("1 + 1", &none(), 0).unwrap();
    assert!(Arc::ptr_eq(
        &parsed,
        &el.parse(&parsed, &none(), 0).unwrap()
    ));
}
