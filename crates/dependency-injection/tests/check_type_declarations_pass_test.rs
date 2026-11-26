//! Tests/Compiler/CheckTypeDeclarationsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture classes of `Tests/Fixtures/CheckTypeDeclarationsPass/`,
//! those the test file declares and the internal classes they name
//! (`stdClass`, `Closure`, `Stringable`, `DateTimeImmutable`, `WeakMap`,
//! `Container`, the function `strlen()`) are class
//! metadata, with their types as PHP prints them.
//!
//! Pending, by reason:
//!
//! - end with `$container->get()`, which creates a service:
//!   testProcessRegisterWithClassName, testProcessVariadicSuccess,
//!   testProcessSuccessWhenNotUsingOptionalArgument,
//!   testProcessSuccessWhenUsingOptionalArgumentWithGoodType,
//!   testProcessSuccessWhenPassingNullToOptional, testProcessSuccessScalarType,
//!   testProcessSuccessOnPassingBadScalarType,
//!   testProcessSuccessPassingBadScalarTypeOptionalArgument,
//!   testProcessSuccessWhenPassingArray, testProcessFactory,
//!   testProcessFactoryCallbackSuccessOnValidType,
//!   testProcessDoesNotThrowsExceptionOnValidTypes;
//! - need `ExpressionLanguage`'s `Expression`: testProcessResolveExpressions,
//!   testProcessSuccessWhenExpressionReturnsObject,
//!   testExpressionLanguageWithSyntheticService.
//!
//! testProcessHandleExistingEnvPlaceholder and testProcessResolveParameters
//! drop their `putenv()`: the pass checks env placeholders by their
//! provided types and never reads the variable.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckTypeDeclarationsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::disallowed_names)]

use std::sync::Arc;

use php_value::{PhpArray, Value};
use symfony_dependency_injection::argument::{IteratorArgument, ServiceClosureArgument};
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, FunctionMetadata, MethodMetadata, ParameterMetadata, Visibility,
};
use symfony_dependency_injection::compiler::{
    CheckTypeDeclarationsPass, CompilerPassInterface, ResolveParameterPlaceHoldersPass,
};
use symfony_dependency_injection::parameter_bag::ParameterBag;
use symfony_dependency_injection::{
    ContainerBuilder, Definition, DefinitionError, ErrorKind, Reference, Result,
};

const NS: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\CheckTypeDeclarationsPass\\";
const TESTS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";

fn fixture(name: &str) -> String {
    format!("{NS}{name}")
}

fn p(name: &str, type_: &str) -> ParameterMetadata {
    ParameterMetadata::new(name, Some(type_))
}

fn optional(name: &str, type_: &str, default: Value) -> ParameterMetadata {
    p(name, type_).with_default(default)
}

fn variadic(name: &str, type_: &str) -> ParameterMetadata {
    let mut parameter = p(name, type_);
    parameter.is_optional = true;
    parameter.is_variadic = true;
    parameter
}

fn method(name: &str, parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    MethodMetadata::public(name).with_parameters(parameters)
}

fn static_method(name: &str, parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    method(name, parameters).with_static(true)
}

/// The fixture classes.
#[expect(clippy::too_many_lines, reason = "one fixture directory")]
fn registry() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    r.add_class(ClassMetadata::class("stdClass"));
    r.add_class(ClassMetadata::interface("Stringable"));
    r.add_function(FunctionMetadata {
        name: "strlen".to_string(),
        parameters: vec![p("string", "string")],
        return_type: Some("int".to_string()),
        ..FunctionMetadata::default()
    });
    r.add_class(ClassMetadata::class("Closure").with_method(static_method(
        "fromCallable",
        vec![p("callback", "callable")],
    )));
    r.add_class(ClassMetadata::class("DateTimeImmutable").with_interfaces(&["DateTimeInterface"]));
    r.add_class(ClassMetadata::interface("DateTimeInterface"));
    r.add_class(ClassMetadata::class("WeakMap"));
    r.add_class(
        ClassMetadata::class("Symfony\\Component\\DependencyInjection\\Container").with_method(
            MethodMetadata::public("getResetMap").with_visibility(Visibility::Protected),
        ),
    );
    r.add_class(
        ClassMetadata::class(&fixture("Bar"))
            .with_method(method("__construct", vec![p("foo", "stdClass")])),
    );
    r.add_class(
        ClassMetadata::class(&fixture("BarErroredDependency"))
            .with_method(method("__construct", vec![p("foo", "stdClass")])),
    );
    let foo = fixture("Foo");
    r.add_class(
        ClassMetadata::class(&fixture("BarMethodCall"))
            .with_method(method("setBar", vec![p("bar", &fixture("Bar"))]))
            .with_method(method("setFoo", vec![p("foo", "stdClass")]))
            .with_method(method(
                "setFoosVariadic",
                vec![p("foo", &foo), variadic("foos", &foo)],
            ))
            .with_method(method(
                "setFoosOptional",
                vec![
                    p("foo", &foo),
                    optional("fooOptional", &format!("?{foo}"), Value::Null),
                ],
            ))
            .with_method(method(
                "setScalars",
                vec![
                    p("int", "int"),
                    p("string", "string"),
                    optional("bool", "bool", Value::Bool(false)),
                ],
            ))
            .with_method(method("setArray", vec![p("array", "array")]))
            .with_method(method("setIterable", vec![p("iterable", "iterable")]))
            .with_method(method("setCallable", vec![p("callable", "callable")]))
            .with_method(method("setClosure", vec![p("closure", "Closure")]))
            .with_method(method("setString", vec![p("string", "string")])),
    );
    r.add_class(
        ClassMetadata::class(&fixture("BarOptionalArgument")).with_method(method(
            "__construct",
            vec![optional("foo", "?stdClass", Value::Null)],
        )),
    );
    r.add_class(
        ClassMetadata::class(&fixture("BarOptionalArgumentNotNull")).with_method(method(
            "__construct",
            vec![optional("foo", "int", Value::Int(1))],
        )),
    );
    r.add_class(ClassMetadata::class(&fixture("Deprecated")));
    r.add_class(
        ClassMetadata::class(&foo)
            .with_method(static_method("createBar", Vec::new()))
            .with_method(static_method(
                "createBarArguments",
                vec![
                    p("stdClass", "stdClass"),
                    optional("stdClassOptional", "?stdClass", Value::Null),
                ],
            ))
            .with_method(static_method("createCallable", Vec::new()).with_return_type("callable"))
            .with_method(static_method("createArray", Vec::new()).with_return_type("array"))
            .with_method(static_method("createStdClass", Vec::new()).with_return_type("stdClass")),
    );
    r.add_class(
        ClassMetadata::class(&fixture("FooObject"))
            .with_method(method("__construct", vec![p("foo", "object")])),
    );
    let waldo_interface = fixture("WaldoInterface");
    r.add_class(
        ClassMetadata::class(&fixture("IntersectionConstructor")).with_method(method(
            "__construct",
            vec![p("arg", &format!("{foo}&{waldo_interface}"))],
        )),
    );
    r.add_class(
        ClassMetadata::class(&fixture("UnionConstructor"))
            .with_method(method("__construct", vec![p("arg", &format!("{foo}|int"))]))
            .with_method(
                static_method("create", vec![p("arg", "array|false")]).with_return_type("static"),
            )
            .with_method(static_method("make", vec![p("arg", "mixed")]).with_return_type("static")),
    );
    r.add_class(
        ClassMetadata::class(&fixture("UnionConstructorPHP82"))
            .with_method(
                static_method("createTrue", vec![p("arg", "array|true")])
                    .with_return_type("static"),
            )
            .with_method(
                static_method("createNull", vec![p("arg", "null")]).with_return_type("static"),
            ),
    );
    r.add_class(ClassMetadata::interface(&waldo_interface));
    r.add_class(ClassMetadata::class(&fixture("Waldo")).with_interfaces(&[&waldo_interface]));
    r.add_class(
        ClassMetadata::class(&fixture("WaldoFoo"))
            .with_parent(&foo)
            .with_interfaces(&[&waldo_interface]),
    );
    r.add_class(
        ClassMetadata::class(&fixture("Wobble"))
            .with_method(method("__construct", vec![p("waldo", &waldo_interface)])),
    );
    // The classes of the test file.
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}CallableClass")).with_method(method(
            "__call",
            vec![
                ParameterMetadata::new("name", None),
                ParameterMetadata::new("arguments", None),
            ],
        )),
    );
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}StaticCallableClass")).with_method(static_method(
            "__callStatic",
            vec![
                ParameterMetadata::new("name", None),
                ParameterMetadata::new("arguments", None),
            ],
        )),
    );
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}ServiceWithTwoInts"))
            .with_method(method("__construct", vec![p("a", "int"), p("b", "int")])),
    );
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}StringableClass"))
            .with_interfaces(&["Stringable"])
            .with_method(method("__toString", Vec::new()).with_return_type("string")),
    );
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}StringConsumer"))
            .with_method(method("__construct", vec![p("value", "string")])),
    );
    r.add_class(
        ClassMetadata::class(&format!("{TESTS}WeakMapConsumer"))
            .with_method(method("__construct", vec![p("map", "WeakMap")])),
    );
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(registry());
    container
}

fn container_with_env(parameters: &[(&str, &str)]) -> ContainerBuilder {
    let mut array = PhpArray::new();
    for (k, v) in parameters {
        array.insert(*k, Value::from(*v));
    }
    let mut container =
        ContainerBuilder::with_parameter_bag(ParameterBag::env_placeholder(&array).unwrap());
    container.set_class_registry(registry());
    container
}

fn process(container: &mut ContainerBuilder) -> Result<()> {
    CheckTypeDeclarationsPass::new(true, Vec::new()).process(container)
}

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

fn r(id: &str) -> Value {
    Reference::new(id).into()
}

fn s(text: &str) -> Value {
    Value::from(text)
}

fn factory(first: Value, method: &str) -> Value {
    Value::Array(list([first, s(method)]))
}

fn call(definition: &Definition, method: &str, arguments: Vec<Value>) {
    definition
        .add_method_call(method, list(arguments), false)
        .unwrap();
}

fn assert_invalid(container: &mut ContainerBuilder, message: &str) {
    let e = process(container).unwrap_err();
    assert!(e.instance_of("InvalidArgumentException"), "{}", e.message());
    assert_eq!(message, e.message());
}

fn invalid(service: &str, argument: usize, function: &str, accepts: &str, passed: &str) -> String {
    format!(
        "Invalid definition for service \"{service}\": argument {argument} of \"{function}()\" accepts \"{accepts}\", \"{passed}\" passed."
    )
}

#[test]
fn testProcessThrowsExceptionOnInvalidTypesConstructorArguments() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(r("foo"));

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}Bar::__construct"),
            "stdClass",
            &fixture("Foo"),
        ),
    );
}

#[test]
fn testProcessThrowsExceptionOnInvalidTypesMethodCallArguments() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setFoo", vec![r("foo")]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarMethodCall::setFoo"),
            "stdClass",
            &fixture("Foo"),
        ),
    );
}

#[test]
fn testProcessFailsWhenPassingNullToRequiredArgument() {
    let mut container = container();

    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(Value::Null);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}Bar::__construct"),
            "stdClass",
            "null",
        ),
    );
}

#[test]
fn testProcessThrowsExceptionWhenMissingArgumentsInConstructor() {
    let mut container = container();

    container.register("bar", Some(&fixture("Bar"))).unwrap();

    assert_invalid(
        &mut container,
        &format!(
            "Invalid definition for service \"bar\": \"{NS}Bar::__construct()\" requires 1 arguments, 0 passed."
        ),
    );
}

#[test]
fn testProcessSuccessWhenPassingTooManyArgumentInConstructor() {
    let mut container = container();

    container.register("foo", Some("stdClass")).unwrap();
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(r("foo"))
        .add_argument(r("foo"));

    process(&mut container).unwrap();
}

#[test]
fn testProcessThrowsExceptionWhenMissingArgumentsInMethodCall() {
    let mut container = container();

    container.register("foo", Some("stdClass")).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    bar.add_argument(r("foo"));
    call(&bar, "setFoo", vec![]);

    assert_invalid(
        &mut container,
        &format!(
            "Invalid definition for service \"bar\": \"{NS}BarMethodCall::setFoo()\" requires 1 arguments, 0 passed."
        ),
    );
}

#[test]
fn testProcessVariadicFails() {
    let mut container = container();

    container.register("stdClass", Some("stdClass")).unwrap();
    container.register("foo", Some(&fixture("Foo"))).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(
        &bar,
        "setFoosVariadic",
        vec![r("foo"), r("foo"), r("stdClass")],
    );

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            2,
            &format!("{NS}BarMethodCall::setFoosVariadic"),
            &fixture("Foo"),
            "stdClass",
        ),
    );
}

#[test]
fn testProcessVariadicFailsOnPassingBadTypeOnAnotherArgument() {
    let mut container = container();

    container.register("stdClass", Some("stdClass")).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setFoosVariadic", vec![r("stdClass")]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarMethodCall::setFoosVariadic"),
            &fixture("Foo"),
            "stdClass",
        ),
    );
}

#[test]
fn testProcessFailsWhenUsingOptionalArgumentWithBadType() {
    let mut container = container();

    container.register("stdClass", Some("stdClass")).unwrap();
    container.register("foo", Some(&fixture("Foo"))).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setFoosOptional", vec![r("foo"), r("stdClass")]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            2,
            &format!("{NS}BarMethodCall::setFoosOptional"),
            &fixture("Foo"),
            "stdClass",
        ),
    );
}

#[test]
fn testProcessSuccessWhenPassingNullToOptionalThatDoesNotAcceptNull() {
    let mut container = container();

    container
        .register("bar", Some(&fixture("BarOptionalArgumentNotNull")))
        .unwrap()
        .add_argument(Value::Null);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarOptionalArgumentNotNull::__construct"),
            "int",
            "null",
        ),
    );
}

#[test]
fn testProcessFailsWhenPassingBadTypeToOptional() {
    let mut container = container();

    container
        .register("bar", Some(&fixture("BarOptionalArgument")))
        .unwrap()
        .add_argument(s("string instead of stdClass"));

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarOptionalArgument::__construct"),
            "stdClass",
            "string",
        ),
    );
}

#[test]
fn testProcessFailsOnPassingScalarTypeToConstructorTypedWithClass() {
    let mut container = container();

    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(Value::Int(1));

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}Bar::__construct"),
            "stdClass",
            "int",
        ),
    );
}

#[test]
fn testProcessFailsOnPassingScalarTypeToMethodTypedWithClass() {
    let mut container = container();

    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setFoo", vec![s("builtin type instead of class")]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarMethodCall::setFoo"),
            "stdClass",
            "string",
        ),
    );
}

#[test]
fn testProcessFailsOnPassingClassToScalarTypedParameter() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setScalars", vec![r("foo"), r("foo")]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}BarMethodCall::setScalars"),
            "int",
            &fixture("Foo"),
        ),
    );
}

#[test]
fn testProcessSuccessWhenPassingIntegerToArrayTypedParameter() {
    let mut container = container();

    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar, "setArray", vec![Value::Int(1)]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidParameterType));
    assert_eq!(
        invalid(
            "bar",
            1,
            &format!("{NS}BarMethodCall::setArray"),
            "array",
            "int"
        ),
        e.message()
    );
}

#[test]
fn testProcessSuccessWhenPassingAnIteratorArgumentToIterable() {
    let mut container = container();

    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(
        &bar,
        "setIterable",
        vec![IteratorArgument::new(PhpArray::new()).to_value()],
    );

    process(&mut container).unwrap();
}

#[test]
fn testProcessSuccessWhenPassingDefinitionForObjectType() {
    let mut container = container();

    container
        .register("foo_object", Some(&fixture("FooObject")))
        .unwrap()
        .add_argument(Definition::with_class(&fixture("Foo")));

    process(&mut container).unwrap();
}

#[test]
fn testProcessFactoryFailsOnInvalidParameterType() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(r("foo"))
        .set_factory(factory(r("foo"), "createBarArguments"));

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            1,
            &format!("{NS}Foo::createBarArguments"),
            "stdClass",
            &fixture("Foo"),
        ),
    );
}

#[test]
fn testProcessFactoryFailsOnInvalidParameterTypeOptional() {
    let mut container = container();

    container.register("stdClass", Some("stdClass")).unwrap();
    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(r("stdClass"))
        .add_argument(r("foo"))
        .set_factory(factory(r("foo"), "createBarArguments"));

    assert_invalid(
        &mut container,
        &invalid(
            "bar",
            2,
            &format!("{NS}Foo::createBarArguments"),
            "stdClass",
            &fixture("Foo"),
        ),
    );
}

#[test]
fn testProcessFactorySuccessOnValidTypes() {
    let mut container = container();

    container.register("stdClass", Some("stdClass")).unwrap();
    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .add_argument(r("stdClass"))
        .add_argument(r("stdClass"))
        .set_factory(factory(r("foo"), "createBarArguments"));

    process(&mut container).unwrap();
}

#[test]
fn testProcessDoesNotLoadCodeByDefault() {
    let mut container = container();

    container
        .register("foo", Some(&fixture("FooNotExisting")))
        .unwrap();
    let bar = container
        .register("bar", Some(&fixture("BarNotExisting")))
        .unwrap();
    bar.add_argument(r("foo"));
    call(&bar, "setFoo", vec![r("foo"), s("string"), Value::Int(1)]);

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testProcessFactoryDoesNotLoadCodeByDefault() {
    let mut container = container();

    container
        .register("foo", Some(&fixture("FooNotExisting")))
        .unwrap();
    container
        .register("bar", Some(&fixture("BarNotExisting")))
        .unwrap()
        .set_factory(factory(r("foo"), "notExistingMethod"));

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testProcessFactoryForTypeSameAsClass() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some("callable"))
        .unwrap()
        .set_factory(factory(r("foo"), "createCallable"));
    let bar_call = container
        .register("bar_call", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar_call, "setCallable", vec![r("bar")]);

    process(&mut container).unwrap();
}

#[test]
fn testProcessFactoryForIterableTypeAndArrayClass() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("bar", Some("array"))
        .unwrap()
        .set_factory(factory(r("foo"), "createArray"));
    let bar_call = container
        .register("bar_call", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar_call, "setIterable", vec![r("bar")]);

    process(&mut container).unwrap();
}

#[test]
fn testProcessFactoryOnServiceContainer() {
    let mut container = container();

    let weak_map = Definition::with_class("WeakMap");
    weak_map.set_factory(factory(r("service_container"), "getResetMap"));
    container
        .register("bar", Some(&format!("{TESTS}WeakMapConsumer")))
        .unwrap()
        .add_argument(weak_map);

    process(&mut container).unwrap();
}

#[test]
fn testProcessPassingBuiltinTypeDoesNotLoadCodeByDefault() {
    let mut container = container();

    container
        .register("bar", Some(&fixture("BarNotExisting")))
        .unwrap()
        .add_argument(Value::Int(1));

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testProcessThrowsOnIterableTypeWhenScalarPassed() {
    let mut container = container();

    let bar_call = container
        .register("bar_call", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&bar_call, "setIterable", vec![Value::Int(2)]);

    assert_invalid(
        &mut container,
        &invalid(
            "bar_call",
            1,
            &format!("{NS}BarMethodCall::setIterable"),
            "iterable",
            "int",
        ),
    );
}

#[test]
fn testProcessHandleMixedEnvPlaceholder() {
    let mut container = container_with_env(&[("ccc", "%env(FOO)%")]);

    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setArray", vec![s("foo%ccc%")]);

    assert_invalid(
        &mut container,
        &invalid(
            "foobar",
            1,
            &format!("{NS}BarMethodCall::setArray"),
            "array",
            "string",
        ),
    );
}

#[test]
fn testProcessHandleMultipleEnvPlaceholder() {
    let mut container = container_with_env(&[("ccc", "%env(FOO)%"), ("fcy", "%env(int:BAR)%")]);

    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setArray", vec![s("%ccc%%fcy%")]);

    assert_invalid(
        &mut container,
        &invalid(
            "foobar",
            1,
            &format!("{NS}BarMethodCall::setArray"),
            "array",
            "string",
        ),
    );
}

#[test]
fn testProcessHandleExistingEnvPlaceholder() {
    let mut container = container_with_env(&[("ccc", "%env(json:ARRAY)%")]);

    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setArray", vec![s("%ccc%")]);

    ResolveParameterPlaceHoldersPass::default()
        .process(&mut container)
        .unwrap();
    process(&mut container).unwrap();
}

#[test]
fn testProcessHandleNotFoundEnvPlaceholder() {
    let mut container = container_with_env(&[("ccc", "%env(json:ARRAY)%")]);

    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setArray", vec![s("%ccc%")]);

    ResolveParameterPlaceHoldersPass::default()
        .process(&mut container)
        .unwrap();
    process(&mut container).unwrap();
}

#[test]
fn testProcessSkipSkippedIds() {
    let mut container = container();
    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setArray", vec![s("a string")]);

    CheckTypeDeclarationsPass::new(true, vec!["foobar".to_string()])
        .process(&mut container)
        .unwrap();
}

#[test]
fn testProcessSkipsDeprecatedDefinitions() {
    let mut container = container();
    container
        .register("foobar", Some(&fixture("Deprecated")))
        .unwrap()
        .set_deprecated("foo/bar", "1.2.3", "")
        .unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessHandleClosureForCallable() {
    let closure_definition = Definition::with_class("Closure");
    closure_definition.set_factory(factory(s("Closure"), "fromCallable"));
    closure_definition.set_arguments(list([s("strlen")]));

    let mut container = container();
    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setCallable", vec![closure_definition.into()]);

    process(&mut container).unwrap();
}

#[test]
fn testProcessSuccessWhenPassingServiceClosureArgumentToCallable() {
    let mut container = container();

    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(
        &bar,
        "setCallable",
        vec![ServiceClosureArgument::new(r("foo")).to_value()],
    );

    process(&mut container).unwrap();
}

#[test]
fn testProcessSuccessWhenPassingServiceClosureArgumentToClosure() {
    let mut container = container();

    let bar = container
        .register("bar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(
        &bar,
        "setClosure",
        vec![ServiceClosureArgument::new(r("foo")).to_value()],
    );

    process(&mut container).unwrap();
}

#[test]
fn testProcessResolveParameters() {
    let mut container = container_with_env(&[("env_array_param", "%env(json:ARRAY)%")]);
    container
        .set_parameter("array_param", Value::Array(list([s("foobar")])))
        .unwrap();
    container.set_parameter("string_param", s("ccc")).unwrap();

    let definition = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&definition, "setArray", vec![s("%array_param%")]);
    call(&definition, "setString", vec![s("%string_param%")]);

    ResolveParameterPlaceHoldersPass::default()
        .process(&mut container)
        .unwrap();

    call(&definition, "setArray", vec![s("%env_array_param%")]);

    process(&mut container).unwrap();
}

#[test]
fn testProcessDoesNotResolvePercentSpansWithWhitespace() {
    let mut container = container();

    let foobar = container
        .register("foobar", Some(&fixture("BarMethodCall")))
        .unwrap();
    call(&foobar, "setString", vec![s("%foo bar%")]);
    call(&foobar, "setString", vec![s("%foo\nbar%")]);

    process(&mut container).unwrap();
}

#[test]
fn testUnionTypePassesWithReference() {
    let mut container = container();

    container.register("foo", Some(&fixture("Foo"))).unwrap();
    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_arguments(list([r("foo")]));

    process(&mut container).unwrap();
}

#[test]
fn testUnionTypePassesWithBuiltin() {
    let mut container = container();

    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_arguments(list([Value::Int(42)]));

    process(&mut container).unwrap();
}

#[test]
fn testUnionTypePassesWithFalse() {
    let mut container = container();

    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_factory(factory(s(&fixture("UnionConstructor")), "create"))
        .set_arguments(list([Value::Bool(false)]));

    process(&mut container).unwrap();
}

#[test]
fn testUnionTypePassesWithTrue() {
    let mut container = container();

    let class = fixture("UnionConstructorPHP82");
    container
        .register("unionTrue", Some(&class))
        .unwrap()
        .set_factory(factory(s(&class), "createTrue"))
        .set_arguments(list([Value::Bool(true)]));

    container
        .register("unionNull", Some(&class))
        .unwrap()
        .set_factory(factory(s(&class), "createNull"))
        .set_arguments(list([Value::Null]));

    process(&mut container).unwrap();
}

#[test]
fn testUnionTypeFailsWithReference() {
    let mut container = container();

    container
        .register("waldo", Some(&fixture("Waldo")))
        .unwrap();
    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_arguments(list([r("waldo")]));

    assert_invalid(
        &mut container,
        &invalid(
            "union",
            1,
            &format!("{NS}UnionConstructor::__construct"),
            &format!("{NS}Foo|int"),
            &fixture("Waldo"),
        ),
    );
}

#[test]
fn testUnionTypeFailsWithBuiltin() {
    let mut container = container();

    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_arguments(list([Value::Array(list([
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]))]));

    assert_invalid(
        &mut container,
        &invalid(
            "union",
            1,
            &format!("{NS}UnionConstructor::__construct"),
            &format!("{NS}Foo|int"),
            "array",
        ),
    );
}

#[test]
fn testUnionTypeWithFalseFailsWithReference() {
    let mut container = container();

    container
        .register("waldo", Some(&fixture("Waldo")))
        .unwrap();
    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_factory(factory(s(&fixture("UnionConstructor")), "create"))
        .set_arguments(list([r("waldo")]));

    assert_invalid(
        &mut container,
        &invalid(
            "union",
            1,
            &format!("{NS}UnionConstructor::create"),
            "array|false",
            &fixture("Waldo"),
        ),
    );
}

#[test]
fn testUnionTypeWithFalseFailsWithTrue() {
    let mut container = container();

    container
        .register("waldo", Some(&fixture("Waldo")))
        .unwrap();
    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_factory(factory(s(&fixture("UnionConstructor")), "create"))
        .set_arguments(list([Value::Bool(true)]));

    assert_invalid(
        &mut container,
        &invalid(
            "union",
            1,
            &format!("{NS}UnionConstructor::create"),
            "array|false",
            "bool",
        ),
    );
}

#[test]
fn testReferencePassesMixed() {
    let mut container = container();

    container
        .register("waldo", Some(&fixture("Waldo")))
        .unwrap();
    container
        .register("union", Some(&fixture("UnionConstructor")))
        .unwrap()
        .set_factory(factory(s(&fixture("UnionConstructor")), "make"))
        .set_arguments(list([r("waldo")]));

    process(&mut container).unwrap();
}

#[test]
fn testIntersectionTypePassesWithReference() {
    let mut container = container();

    container
        .register("foo", Some(&fixture("WaldoFoo")))
        .unwrap();
    container
        .register("intersection", Some(&fixture("IntersectionConstructor")))
        .unwrap()
        .set_arguments(list([r("foo")]));

    process(&mut container).unwrap();
}

#[test]
fn testIntersectionTypeFailsWithReference() {
    let mut container = container();

    container
        .register("waldo", Some(&fixture("Waldo")))
        .unwrap();
    container
        .register("intersection", Some(&fixture("IntersectionConstructor")))
        .unwrap()
        .set_arguments(list([r("waldo")]));

    assert_invalid(
        &mut container,
        &invalid(
            "intersection",
            1,
            &format!("{NS}IntersectionConstructor::__construct"),
            &format!("{NS}Foo&{NS}WaldoInterface"),
            &fixture("Waldo"),
        ),
    );
}

#[test]
fn testCallableClass() {
    let mut container = container();
    let definition = container
        .register("foo", Some(&format!("{TESTS}CallableClass")))
        .unwrap();
    call(&definition, "callMethod", vec![Value::Int(123)]);

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testStaticCallableClass() {
    let mut container = container();
    let class = format!("{TESTS}StaticCallableClass");
    container
        .register("foo", Some(&class))
        .unwrap()
        .set_factory(factory(s(&class), "staticMethodCall"));

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testIgnoreDefinitionFactoryArgument() {
    let mut container = container();
    let foo = Definition::with_class(&fixture("Foo"));
    foo.set_factory(factory(s(&fixture("Foo")), "createStdClass"));
    container
        .register("bar", Some(&fixture("Bar")))
        .unwrap()
        .set_arguments(list([foo.into()]));

    CheckTypeDeclarationsPass::default()
        .process(&mut container)
        .unwrap();
}

#[test]
fn testErroredDefinitionsAreNotChecked() {
    let mut container = container();
    let foo = Definition::with_class(&fixture("Foo"));
    foo.add_error(DefinitionError::Text("error".to_string()));
    container
        .register("errored_dependency", Some(&fixture("BarErroredDependency")))
        .unwrap()
        .set_arguments(list([foo.into()]));

    process(&mut container).unwrap();
}

#[test]
fn testCheckTypeDeclarationsSkipsSubsequentNamedArguments() {
    let mut container = container();
    let mut arguments = PhpArray::new();
    arguments.insert("a", Value::Int(1));
    arguments.insert("b", Value::Array(PhpArray::new()));
    container
        .register("service", Some(&format!("{TESTS}ServiceWithTwoInts")))
        .unwrap()
        .set_arguments(arguments);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidParameterType));
    assert!(e.message().contains(&format!(
        "argument 2 of \"{TESTS}ServiceWithTwoInts::__construct()\" accepts \"int\", \"array\" passed"
    )));
}

#[test]
fn testStringableCanBePassedToStringTypeParameter() {
    let mut container = container();
    container
        .register("stringable", Some(&format!("{TESTS}StringableClass")))
        .unwrap();
    container
        .register("consumer", Some(&format!("{TESTS}StringConsumer")))
        .unwrap()
        .set_arguments(list([r("stringable")]));

    process(&mut container).unwrap();
}
