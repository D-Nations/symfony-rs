//! ArgumentResolverTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's.
//!
//! PHP reflects a controller callable for its parameters; here each test
//! controller declares them (`ArgumentResolverTestController`'s methods,
//! `NullableController::action`, `VariadicController::action`, the closure
//! and the function), with the name `ArgumentMetadataFactory` reports. A
//! method, a closure, an invokable object and a function name all reach
//! the resolver as such a declaration.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ArgumentResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use php_value::{
    Handle, PhpArray, Value,
    object::{GenericObject, StdClass},
    php_array,
};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    argument_resolver::{
        ArgumentResolver, NamedResolvers, Resolvers, ValueResolver, ValueResolverInterface,
    },
    controller::{Controller, ControllerResult, Parameter, as_request},
    controller_metadata::{ArgumentMetadata, ArgumentMetadataFactory},
    exception::{PhpClass, PhpException, Throwable, instance_of, message_of},
    value_resolver::{DefaultValueResolver, RequestAttributeValueResolver},
};

const TEST_CONTROLLER: &str =
    "Symfony\\Component\\HttpKernel\\Tests\\Controller\\ArgumentResolverTestController";
const EXTENDING_SESSION: &str =
    "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Controller\\ExtendingSession";
const POST: &str = "Symfony\\Component\\HttpKernel\\Tests\\Controller\\Post";

fn get_resolver(
    chainable_resolvers: Resolvers,
    named_resolvers: Option<Vec<(String, Arc<dyn ValueResolverInterface>)>>,
) -> ArgumentResolver {
    ArgumentResolver::new(
        Some(Arc::new(ArgumentMetadataFactory)),
        chainable_resolvers,
        named_resolvers.map(NamedResolvers::locator),
    )
}

fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

fn set(request: &RequestHandle, name: &str, value: Value) {
    request.with_mut(|r| r.attributes_mut().set(name, value));
}

fn controller(name: &str, parameters: Vec<Parameter>) -> Controller {
    Controller::new(|_| Ok(ControllerResult::Value(Value::Null)))
        .with_name(name)
        .with_parameters(parameters)
}

fn method(method: &str, parameters: Vec<Parameter>) -> Controller {
    controller(&format!("{TEST_CONTROLLER}::{method}"), parameters)
}

fn p(name: &str) -> Parameter {
    Parameter::new(name)
}

fn arguments(
    resolver: &ArgumentResolver,
    request: &RequestHandle,
    controller: &Controller,
) -> Vec<Value> {
    resolver.get_arguments(request, controller, None).unwrap()
}

fn error(
    resolver: &ArgumentResolver,
    request: &RequestHandle,
    controller: &Controller,
) -> Throwable {
    resolver
        .get_arguments(request, controller, None)
        .unwrap_err()
}

fn s(value: &str) -> Value {
    Value::string(value)
}

/// `[new ArgumentResolverTestController(), 'controllerWithFoo']`.
fn controller_with_foo() -> Controller {
    method("controllerWithFoo", vec![p("foo")])
}

fn controller_with_foo_and_default_bar() -> Controller {
    method(
        "controllerWithFooAndDefaultBar",
        vec![p("foo"), p("bar").with_default(Value::Null)],
    )
}

fn post(title: &str) -> Value {
    Value::Object(Arc::new(GenericObject::new(
        POST,
        php_array!["title" => title],
    )))
}

/// `TestEntityValueResolver`.
struct TestEntityValueResolver;

impl ValueResolverInterface for TestEntityValueResolver {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver"
    }

    fn resolve(
        &self,
        request: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        let title = request.with(|r| {
            if r.request().has("title") {
                r.request().get("title", Value::Null).ok()
            } else {
                None
            }
        });
        Ok(match (argument.get_type(), title) {
            (Some(POST), Some(Value::String(title))) => vec![post(&title.to_string_lossy())],
            _ => Vec::new(),
        })
    }
}

/// A value resolver that throws a `NearMissValueResolverException`.
struct NearMiss(&'static str);

impl ValueResolverInterface for NearMiss {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\HttpKernel\\Controller\\ValueResolverInterface@anonymous"
    }

    fn resolve(&self, _: &RequestHandle, _: &ArgumentMetadata) -> Result<Vec<Value>, Throwable> {
        Err(PhpException::new(PhpClass::NearMissValueResolverException, self.0).throwable())
    }
}

/// A mock that returns `[123]` and counts its calls.
#[derive(Default)]
struct Returns123(AtomicUsize);

impl ValueResolverInterface for Returns123 {
    fn class(&self) -> &'static str {
        "MockObject_ValueResolverInterface"
    }

    fn resolve(&self, _: &RequestHandle, _: &ArgumentMetadata) -> Result<Vec<Value>, Throwable> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Value::Int(123)])
    }
}

fn resolver_classes(resolver: &ArgumentResolver) -> Vec<String> {
    resolver
        .resolvers()
        .iter()
        .map(|(_, r)| r.class().to_owned())
        .collect()
}

#[test]
fn testDefaultState() {
    assert_eq!(
        resolver_classes(&get_resolver(Vec::new(), None)),
        resolver_classes(&ArgumentResolver::default())
    );
    assert_ne!(
        resolver_classes(&get_resolver(Vec::new(), None)),
        resolver_classes(&ArgumentResolver::new(
            None,
            vec![(None, Arc::new(RequestAttributeValueResolver))],
            None
        ))
    );
}

#[test]
fn testGetArguments() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    let controller = controller_with_foo();

    assert_eq!(
        vec![s("foo")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller),
        "->getArguments() returns an array of arguments for the controller method"
    );
}

#[test]
fn testGetArgumentsReturnsEmptyArrayWhenNoArguments() {
    let request = create("/");
    let controller = method("controllerWithoutArguments", Vec::new());

    assert!(
        arguments(&get_resolver(Vec::new(), None), &request, &controller).is_empty(),
        "->getArguments() returns an empty array if the method takes no arguments"
    );
}

#[test]
fn testGetArgumentsUsesDefaultValue() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    let controller = controller_with_foo_and_default_bar();

    assert_eq!(
        vec![s("foo"), Value::Null],
        arguments(&get_resolver(Vec::new(), None), &request, &controller),
        "->getArguments() uses default values if present"
    );
}

#[test]
fn testGetArgumentsOverrideDefaultValueByRequestAttribute() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    set(&request, "bar", s("bar"));
    let controller = controller_with_foo_and_default_bar();

    assert_eq!(
        vec![s("foo"), s("bar")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller),
        "->getArguments() overrides default values if provided in the request attributes"
    );
}

#[test]
fn testGetArgumentsFromClosure() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    // static function ($foo) {}
    let controller = controller("{closure}", vec![p("foo")]);

    assert_eq!(
        vec![s("foo")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller)
    );
}

#[test]
fn testGetArgumentsUsesDefaultValueFromClosure() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    // static function ($foo, $bar = 'bar') {}
    let controller = controller("{closure}", vec![p("foo"), p("bar").with_default(s("bar"))]);

    assert_eq!(
        vec![s("foo"), s("bar")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller)
    );
}

#[test]
fn testGetArgumentsFromInvokableObject() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    // new ArgumentResolverTestController(): __invoke($foo, $bar = null)
    let controller = method(
        "__invoke",
        vec![p("foo"), p("bar").with_default(Value::Null)],
    );

    assert_eq!(
        vec![s("foo"), Value::Null],
        arguments(&get_resolver(Vec::new(), None), &request, &controller)
    );

    // Test default bar overridden by request attribute
    set(&request, "bar", s("bar"));

    assert_eq!(
        vec![s("foo"), s("bar")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller)
    );
}

#[test]
fn testGetArgumentsFromFunctionName() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    set(&request, "foobar", s("foobar"));
    let controller = controller(
        "Symfony\\Component\\HttpKernel\\Tests\\Controller\\controller_function",
        vec![p("foo"), p("foobar")],
    );

    assert_eq!(
        vec![s("foo"), s("foobar")],
        arguments(&get_resolver(Vec::new(), None), &request, &controller)
    );
}

#[test]
fn testGetArgumentsFailsOnUnresolvedValue() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    set(&request, "foobar", s("foobar"));
    let controller = method(
        "controllerWithFooBarFoobar",
        vec![p("foo"), p("bar"), p("foobar")],
    );

    let e = error(&get_resolver(Vec::new(), None), &request, &controller);
    assert!(
        instance_of(e.as_ref(), "RuntimeException"),
        "->getArguments() throws a \\RuntimeException exception if it cannot determine the argument value"
    );
}

#[test]
fn testGetArgumentsInjectsRequest() {
    let request = create("/");
    let controller = method(
        "controllerWithRequest",
        vec![p("request").typed("Symfony\\Component\\HttpFoundation\\Request")],
    );

    let arguments = arguments(&get_resolver(Vec::new(), None), &request, &controller);
    assert_eq!(1, arguments.len(), "->getArguments() injects the request");
    assert!(as_request(&arguments[0]).unwrap().same(&request));
}

fn variadic_controller() -> Controller {
    controller(
        "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Controller\\VariadicController::action",
        vec![p("foo"), p("bar").variadic()],
    )
}

#[test]
fn testGetVariadicArguments() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    set(&request, "bar", Value::Array(php_array!["foo", "bar"]));

    assert_eq!(
        vec![s("foo"), s("foo"), s("bar")],
        arguments(
            &get_resolver(Vec::new(), None),
            &request,
            &variadic_controller()
        )
    );
}

#[test]
fn testGetVariadicArgumentsWithoutArrayInRequest() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    set(&request, "bar", s("foo"));

    let e = error(
        &get_resolver(Vec::new(), None),
        &request,
        &variadic_controller(),
    );
    assert!(instance_of(e.as_ref(), "InvalidArgumentException"));
}

#[test]
fn testIfExceptionIsThrownWhenMissingAnArgument() {
    let request = create("/");
    let controller = controller_with_foo();

    let e = error(&get_resolver(Vec::new(), None), &request, &controller);
    assert!(instance_of(e.as_ref(), "RuntimeException"));
    assert!(message_of(e.as_ref()).to_string_lossy().contains(&format!(
        "Controller \"{TEST_CONTROLLER}::controllerWithFoo\" requires the \"$foo\" argument that could not be resolved. Either the argument is nullable and no null value has been provided, no default value has been provided or there is a non-optional argument after this one."
    )));
}

/// `NullableController::action(?string $foo, ?\stdClass $bar, ?string
/// $baz = 'value', string $last = '')`.
fn nullable_controller() -> Controller {
    controller(
        "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Controller\\NullableController::action",
        vec![
            p("foo").typed("string").nullable(),
            p("bar").typed("stdClass").nullable(),
            p("baz").typed("string").nullable().with_default(s("value")),
            p("last").typed("string").with_default(s("")),
        ],
    )
}

#[test]
fn testGetNullableArguments() {
    let request = create("/");
    set(&request, "foo", s("foo"));
    let bar = StdClass::object(PhpArray::new());
    set(&request, "bar", bar);
    set(&request, "last", s("last"));

    let arguments = arguments(
        &get_resolver(Vec::new(), None),
        &request,
        &nullable_controller(),
    );
    assert_eq!(4, arguments.len());
    assert_eq!(s("foo"), arguments[0]);
    assert!(matches!(&arguments[1], Value::Object(o) if o.class() == "stdClass"));
    assert_eq!(s("value"), arguments[2]);
    assert_eq!(s("last"), arguments[3]);
}

#[test]
fn testGetNullableArgumentsWithDefaults() {
    let request = create("/");
    set(&request, "last", s("last"));

    assert_eq!(
        vec![Value::Null, Value::Null, s("value"), s("last")],
        arguments(
            &get_resolver(Vec::new(), None),
            &request,
            &nullable_controller()
        )
    );
}

#[test]
fn testGetSessionMissMatchOnNull() {
    let request = create("/");
    let controller = method(
        "controllerWithExtendingSession",
        vec![p("session").typed(EXTENDING_SESSION)],
    );

    let e = error(&get_resolver(Vec::new(), None), &request, &controller);
    assert!(instance_of(e.as_ref(), "RuntimeException"));
}

#[allow(clippy::unnecessary_wraps, reason = "getResolver()'s nullable $namedResolvers")]
fn named(
    resolvers: Vec<(&str, Arc<dyn ValueResolverInterface>)>,
) -> Option<Vec<(String, Arc<dyn ValueResolverInterface>)>> {
    Some(
        resolvers
            .into_iter()
            .map(|(name, r)| (name.to_owned(), r))
            .collect(),
    )
}

fn targeting(resolver: &str, disabled: bool) -> Value {
    ValueResolver::new(resolver, disabled).object()
}

#[test]
fn testTargetedResolver() {
    let resolver = get_resolver(
        Vec::new(),
        named(vec![(
            DefaultValueResolver::CLASS,
            Arc::new(DefaultValueResolver),
        )]),
    );

    let request = create("/");
    set(&request, "foo", s("bar"));
    let controller = method(
        "controllerTargetingResolver",
        vec![
            p("foo")
                .typed("int")
                .with_default(Value::Int(1))
                .with_attributes([targeting(DefaultValueResolver::CLASS, false)]),
        ],
    );

    assert_eq!(
        vec![Value::Int(1)],
        arguments(&resolver, &request, &controller)
    );
}

#[test]
fn testTargetedResolverWithDefaultValue() {
    let resolver = get_resolver(
        Vec::new(),
        named(vec![(
            "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
            Arc::new(TestEntityValueResolver),
        )]),
    );

    let request = create("/");
    let controller = method(
        "controllerTargetingResolverWithDefaultValue",
        vec![
            p("foo")
                .typed(POST)
                .with_default(post("Default"))
                .with_attributes([targeting(
                    "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
                    false,
                )]),
        ],
    );

    let arguments = arguments(&resolver, &request, &controller);

    assert_eq!(1, arguments.len());
    let Value::Object(post) = &arguments[0] else {
        panic!("a Post");
    };
    assert_eq!(Some(&s("Default")), post.properties().get(&"title".into()));
}

#[test]
fn testTargetedResolverWithNullableValue() {
    let resolver = get_resolver(
        Vec::new(),
        named(vec![(
            "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
            Arc::new(TestEntityValueResolver),
        )]),
    );

    let request = create("/");
    let controller = method(
        "controllerTargetingResolverWithNullableValue",
        vec![p("foo").typed(POST).nullable().with_attributes([targeting(
            "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
            false,
        )])],
    );

    assert_eq!(
        vec![Value::Null],
        arguments(&resolver, &request, &controller)
    );
}

#[test]
fn testTargetedResolverWithRequestAttributeValue() {
    let resolver = get_resolver(
        Vec::new(),
        named(vec![(
            "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
            Arc::new(TestEntityValueResolver),
        )]),
    );

    let request = create("/");
    let object = post("Random 1700000000");
    set(&request, "foo", object.clone());
    let controller = method(
        "controllerTargetingResolverWithTestEntity",
        vec![p("foo").typed(POST).with_attributes([targeting(
            "Symfony\\Component\\HttpKernel\\Tests\\Controller\\TestEntityValueResolver",
            false,
        )])],
    );

    let arguments = arguments(&resolver, &request, &controller);
    assert_eq!(1, arguments.len());
    assert!(arguments[0].identical(&object));
}

#[test]
fn testDisabledResolver() {
    let resolver = get_resolver(Vec::new(), named(Vec::new()));

    let request = create("/");
    set(&request, "foo", s("bar"));
    let controller = method(
        "controllerDisablingResolver",
        vec![
            p("foo")
                .typed("int")
                .with_default(Value::Int(1))
                .with_attributes([targeting(RequestAttributeValueResolver::CLASS, true)]),
        ],
    );

    assert_eq!(
        vec![Value::Int(1)],
        arguments(&resolver, &request, &controller)
    );
}

#[test]
fn testManyTargetedResolvers() {
    let resolver = get_resolver(Vec::new(), named(Vec::new()));

    let request = create("/");
    let controller = method(
        "controllerTargetingManyResolvers",
        vec![p("foo").typed("int").with_attributes([
            targeting(RequestAttributeValueResolver::CLASS, false),
            targeting(DefaultValueResolver::CLASS, false),
        ])],
    );

    let e = error(&resolver, &request, &controller);
    assert!(instance_of(e.as_ref(), "LogicException"));
}

#[test]
fn testUnknownTargetedResolver() {
    let resolver = get_resolver(Vec::new(), named(Vec::new()));

    let request = create("/");
    let controller = method(
        "controllerTargetingUnknownResolver",
        vec![
            p("bar")
                .typed("int")
                .with_attributes([targeting("foo", false)]),
        ],
    );

    let e = error(&resolver, &request, &controller);
    assert!(instance_of(
        e.as_ref(),
        "Symfony\\Component\\HttpKernel\\Exception\\ResolverNotFoundException"
    ));
}

#[test]
fn testResolversChainCompletionWhenResolverThrowsSpecialException() {
    let failing_value_resolver: Arc<dyn ValueResolverInterface> =
        Arc::new(NearMiss("This resolver throws an exception"));
    // Put failing value resolver in the beginning
    let expected_to_call_value_resolver = Arc::new(Returns123::default());

    let mut resolvers: Resolvers = vec![(None, failing_value_resolver)];
    resolvers.extend(
        ArgumentResolver::get_default_argument_value_resolvers()
            .into_iter()
            .map(|r| (None, r)),
    );
    resolvers.push((None, expected_to_call_value_resolver.clone()));
    let resolver = get_resolver(resolvers, None);
    let request = create("/");
    let controller = controller_with_foo();

    let actual_arguments = arguments(&resolver, &request, &controller);
    assert_eq!(vec![Value::Int(123)], actual_arguments);
    assert_eq!(1, expected_to_call_value_resolver.0.load(Ordering::SeqCst));
}

#[test]
fn testExceptionListSingle() {
    let resolver = get_resolver(
        vec![(
            None,
            Arc::new(NearMiss("Some reason why value could not be resolved.")),
        )],
        None,
    );
    let request = create("/");
    let controller = controller_with_foo();

    let e = error(&resolver, &request, &controller);
    assert!(instance_of(e.as_ref(), "RuntimeException"));
    assert!(message_of(e.as_ref()).to_string_lossy().contains(
        "Controller \"Symfony\\Component\\HttpKernel\\Tests\\Controller\\ArgumentResolverTestController::controllerWithFoo\" requires the \"$foo\" argument that could not be resolved. Some reason why value could not be resolved."
    ));
}

#[test]
fn testExceptionListMultiple() {
    let resolver = get_resolver(
        vec![
            (
                None,
                Arc::new(NearMiss("Some reason why value could not be resolved.")),
            ),
            (
                None,
                Arc::new(NearMiss("Another reason why value could not be resolved.")),
            ),
        ],
        None,
    );
    let request = create("/");
    let controller = controller_with_foo();

    let e = error(&resolver, &request, &controller);
    assert!(instance_of(e.as_ref(), "RuntimeException"));
    assert!(message_of(e.as_ref()).to_string_lossy().contains(
        "Controller \"Symfony\\Component\\HttpKernel\\Tests\\Controller\\ArgumentResolverTestController::controllerWithFoo\" requires the \"$foo\" argument that could not be resolved. Possible reasons: 1) Some reason why value could not be resolved. 2) Another reason why value could not be resolved."
    ));
}
