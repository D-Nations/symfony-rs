//! ArgumentMetadataFactoryTest.php at symfony/http-kernel v8.1.11. Names
//! and assertions are PHPUnit's.
//!
//! PHP's factory reflects each signature; here each signature is declared
//! as the [`Parameter`]s reflection reports (with `self` written as the
//! class it names), and the tests check the metadata the factory builds
//! from them: nullability from `?T` and from a `null` default, defaults,
//! variadics, attributes and the controller's name.
//!
//! Upstream: symfony/http-kernel Tests/ControllerMetadata/ArgumentMetadataFactoryTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{PhpArray, Value, object::GenericObject, php_array};
use symfony_http_kernel::{
    controller::{Controller, ControllerResult, Parameter},
    controller_metadata::{
        ArgumentMetadata, ArgumentMetadataFactory, ArgumentMetadataFactoryInterface,
    },
};

const THIS: &str =
    "Symfony\\Component\\HttpKernel\\Tests\\ControllerMetadata\\ArgumentMetadataFactoryTest";
const FAKE: &str =
    "Symfony\\Component\\HttpKernel\\Tests\\ControllerMetadata\\FakeClassThatDoesNotExist";
const IMPORTED_AND_FAKE: &str = "Fake\\ImportedAndFake";
const FIXTURES: &str = "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Controller\\";
const FOO: &str = "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Attribute\\Foo";

fn create(name: &str, parameters: Vec<Parameter>) -> Vec<ArgumentMetadata> {
    let controller = Controller::new(|_| Ok(ControllerResult::Value(Value::Null)))
        .with_name(name)
        .with_parameters(parameters);
    ArgumentMetadataFactory.create_argument_metadata(&controller, None)
}

fn p(name: &str) -> Parameter {
    Parameter::new(name)
}

#[allow(clippy::fn_params_excessive_bools, reason = "upstream's constructor")]
fn meta(
    name: &str,
    type_name: Option<&str>,
    is_variadic: bool,
    has_default_value: bool,
    default_value: Value,
    is_nullable: bool,
    controller_name: &str,
) -> ArgumentMetadata {
    ArgumentMetadata::new(
        name,
        type_name,
        is_variadic,
        has_default_value,
        default_value,
        is_nullable,
        Vec::new(),
        controller_name,
    )
}

fn foo(value: &str) -> Value {
    Value::Object(Arc::new(GenericObject::new(
        FOO,
        php_array!["bar" => value],
    )))
}

/// `signature1(self $foo, array $bar, callable $baz)`.
fn signature1() -> Vec<Parameter> {
    vec![
        p("foo").typed(THIS),
        p("bar").typed("array"),
        p("baz").typed("callable"),
    ]
}

fn signature1_metadata() -> Vec<ArgumentMetadata> {
    let name = format!("{THIS}::signature1");
    vec![
        meta("foo", Some(THIS), false, false, Value::Null, false, &name),
        meta(
            "bar",
            Some("array"),
            false,
            false,
            Value::Null,
            false,
            &name,
        ),
        meta(
            "baz",
            Some("callable"),
            false,
            false,
            Value::Null,
            false,
            &name,
        ),
    ]
}

#[test]
fn testSignature1() {
    let arguments = create(&format!("{THIS}::signature1"), signature1());

    assert_eq!(signature1_metadata(), arguments);
}

#[test]
fn testSignature2() {
    // signature2(?self $foo = null, ?FakeClassThatDoesNotExist $bar = null,
    // ?ImportedAndFake $baz = null)
    let arguments = create(
        &format!("{THIS}::signature2"),
        vec![
            p("foo").typed(THIS).nullable().with_default(Value::Null),
            p("bar").typed(FAKE).nullable().with_default(Value::Null),
            p("baz")
                .typed(IMPORTED_AND_FAKE)
                .nullable()
                .with_default(Value::Null),
        ],
    );

    let name = format!("{THIS}::signature2");
    assert_eq!(
        vec![
            meta("foo", Some(THIS), false, true, Value::Null, true, &name),
            meta("bar", Some(FAKE), false, true, Value::Null, true, &name),
            meta(
                "baz",
                Some(IMPORTED_AND_FAKE),
                false,
                true,
                Value::Null,
                true,
                &name
            ),
        ],
        arguments
    );
}

#[test]
fn testSignature3() {
    // signature3(FakeClassThatDoesNotExist $bar, ImportedAndFake $baz)
    let arguments = create(
        &format!("{THIS}::signature3"),
        vec![p("bar").typed(FAKE), p("baz").typed(IMPORTED_AND_FAKE)],
    );

    let name = format!("{THIS}::signature3");
    assert_eq!(
        vec![
            meta("bar", Some(FAKE), false, false, Value::Null, false, &name),
            meta(
                "baz",
                Some(IMPORTED_AND_FAKE),
                false,
                false,
                Value::Null,
                false,
                &name
            ),
        ],
        arguments
    );
}

#[test]
fn testSignature4() {
    // signature4($foo = 'default', $bar = 500, $baz = [])
    let arguments = create(
        &format!("{THIS}::signature4"),
        vec![
            p("foo").with_default(Value::string("default")),
            p("bar").with_default(Value::Int(500)),
            p("baz").with_default(Value::Array(PhpArray::new())),
        ],
    );

    let name = format!("{THIS}::signature4");
    assert_eq!(
        vec![
            meta(
                "foo",
                None,
                false,
                true,
                Value::string("default"),
                false,
                &name
            ),
            meta("bar", None, false, true, Value::Int(500), false, &name),
            meta(
                "baz",
                None,
                false,
                true,
                Value::Array(PhpArray::new()),
                false,
                &name
            ),
        ],
        arguments
    );
}

#[test]
fn testSignature5() {
    // signature5(?array $foo = null, $bar = null)
    let arguments = create(
        &format!("{THIS}::signature5"),
        vec![
            p("foo").typed("array").nullable().with_default(Value::Null),
            p("bar").with_default(Value::Null),
        ],
    );

    let name = format!("{THIS}::signature5");
    assert_eq!(
        vec![
            meta("foo", Some("array"), false, true, Value::Null, true, &name),
            meta("bar", None, false, true, Value::Null, true, &name),
        ],
        arguments
    );
}

#[test]
fn testVariadicSignature() {
    // VariadicController::action($foo, ...$bar)
    let name = format!("{FIXTURES}VariadicController::action");
    let arguments = create(&name, vec![p("foo"), p("bar").variadic()]);

    assert_eq!(
        vec![
            meta("foo", None, false, false, Value::Null, false, &name),
            meta("bar", None, true, false, Value::Null, false, &name),
        ],
        arguments
    );
}

#[test]
fn testBasicTypesSignature() {
    // BasicTypesController::action(string $foo, int $bar, float $baz)
    let name = format!("{FIXTURES}BasicTypesController::action");
    let arguments = create(
        &name,
        vec![
            p("foo").typed("string"),
            p("bar").typed("int"),
            p("baz").typed("float"),
        ],
    );

    assert_eq!(
        vec![
            meta(
                "foo",
                Some("string"),
                false,
                false,
                Value::Null,
                false,
                &name
            ),
            meta("bar", Some("int"), false, false, Value::Null, false, &name),
            meta(
                "baz",
                Some("float"),
                false,
                false,
                Value::Null,
                false,
                &name
            ),
        ],
        arguments
    );
}

#[test]
fn testNamedClosure() {
    // $this->signature1(...): a closure of the method keeps its name
    let arguments = create(&format!("{THIS}::signature1"), signature1());

    assert_eq!(signature1_metadata(), arguments);
}

#[test]
fn testNullableTypesSignature() {
    // NullableController::action(?string $foo, ?\stdClass $bar, ?string
    // $baz = 'value', string $last = '')
    let name = format!("{FIXTURES}NullableController::action");
    let arguments = create(
        &name,
        vec![
            p("foo").typed("string").nullable(),
            p("bar").typed("stdClass").nullable(),
            p("baz")
                .typed("string")
                .nullable()
                .with_default(Value::string("value")),
            p("last").typed("string").with_default(Value::string("")),
        ],
    );

    assert_eq!(
        vec![
            meta(
                "foo",
                Some("string"),
                false,
                false,
                Value::Null,
                true,
                &name
            ),
            meta(
                "bar",
                Some("stdClass"),
                false,
                false,
                Value::Null,
                true,
                &name
            ),
            meta(
                "baz",
                Some("string"),
                false,
                true,
                Value::string("value"),
                true,
                &name
            ),
            meta(
                "last",
                Some("string"),
                false,
                true,
                Value::string(""),
                false,
                &name
            ),
        ],
        arguments
    );
}

#[test]
fn testAttributeSignature() {
    // AttributeController::action(#[Foo('bar')] string $baz)
    let name = format!("{FIXTURES}AttributeController::action");
    let arguments = create(
        &name,
        vec![p("baz").typed("string").with_attributes([foo("bar")])],
    );

    assert_eq!(
        vec![ArgumentMetadata::new(
            "baz",
            Some("string"),
            false,
            false,
            Value::Null,
            false,
            vec![foo("bar")],
            name.as_str(),
        )],
        arguments
    );
}

#[test]
fn testIssue41478() {
    // AttributeController::issue41478(#[Foo('bar')] string $baz, string $bat)
    let name = format!("{FIXTURES}AttributeController::issue41478");
    let arguments = create(
        &name,
        vec![
            p("baz").typed("string").with_attributes([foo("bar")]),
            p("bat").typed("string"),
        ],
    );
    assert_eq!(
        vec![
            ArgumentMetadata::new(
                "baz",
                Some("string"),
                false,
                false,
                Value::Null,
                false,
                vec![foo("bar")],
                name.as_str(),
            ),
            meta(
                "bat",
                Some("string"),
                false,
                false,
                Value::Null,
                false,
                &name
            ),
        ],
        arguments
    );
}
