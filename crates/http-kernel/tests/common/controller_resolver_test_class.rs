//! The test methods of ControllerResolverTest.php at symfony/http-kernel
//! v8.1.11, for it and the test classes that extend it
//! (`ContainerControllerResolverTest`), as PHPUnit runs inherited test
//! methods. A [`TestClass`] gives what a subclass overrides
//! (`createControllerResolver()`, `getUndefinedControllers()`) and the
//! classes its test file declares; each test file runs every method here
//! as a `#[test]` of the same name. The test file's functions and classes
//! (`some_controller_function`, `ControllerTest`, `InvokableController`,
//! `TestAbstractController`, `PrivateConstructorController`) are declared
//! in a `ClassRegistry`; `assertSame()` on objects and closures is
//! identity, `expectExceptionMessage()` a substring.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ControllerResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{Handle, PhpArray, Value, object::StdClass};
use psr_log::{Context, LogLevel, LoggerInterface};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    controller::{Controller, ControllerResult, Parameter},
    controller_resolver::{
        AS_CONTROLLER, Class, ClassKind, ClassRegistry, ClosureObject, ControllerResolver,
        Function, Method, ResolvedController, Visibility,
    },
    exception::{Throwable, instance_of, message_of},
};

pub const NS: &str = "Symfony\\Component\\HttpKernel\\Tests\\Controller";
pub const BAD_REQUEST: &str = "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException";

/// What a test class that extends `ControllerResolverTest` overrides.
pub trait TestClass {
    /// How many cases `getUndefinedControllers()` gives.
    const UNDEFINED_CONTROLLER_COUNT: usize;

    /// The classes the test file declares beyond the parent's.
    fn declare_test_file_classes(_: &mut ClassRegistry) {}

    /// `createControllerResolver($logger)` over the declared program.
    fn createControllerResolver(
        logger: Option<Arc<dyn LoggerInterface>>,
        program: Arc<ClassRegistry>,
    ) -> ControllerResolver;

    /// `getUndefinedControllers()`.
    fn getUndefinedControllers(
        resolver: &ControllerResolver,
    ) -> Vec<(Value, &'static str, String)> {
        parent_getUndefinedControllers(resolver)
    }
}

/// `$this->createControllerResolver($logger)`.
pub fn createControllerResolver<T: TestClass>(
    logger: Option<Arc<dyn LoggerInterface>>,
) -> ControllerResolver {
    T::createControllerResolver(logger, program::<T>())
}

pub fn name(short: &str) -> String {
    format!("{NS}\\{short}")
}

pub fn returns(value: Value) -> Controller {
    Controller::new(move |_| Ok(ControllerResult::Value(value.clone())))
}

/// The functions and classes the test file declares.
pub fn program<T: TestClass>() -> Arc<ClassRegistry> {
    let mut registry = ClassRegistry::new();
    registry
        .declare_function(
            Function::new(
                name("some_controller_function"),
                returns(Value::Null)
                    .with_parameters([Parameter::new("foo"), Parameter::new("foobar")]),
            )
            .with_attributes([name("DummyController")]),
        )
        .unwrap();
    registry
        .declare_class(
            Class::new(name("ControllerTest"))
                .with_method(Method::new("__construct", returns(Value::Null)))
                .with_method(Method::new("__toString", returns(Value::string(""))))
                .with_method(Method::new("publicAction", returns(Value::Null)))
                .with_method(
                    Method::new("privateAction", returns(Value::Null))
                        .with_visibility(Visibility::Private),
                )
                .with_method(
                    Method::new("protectedAction", returns(Value::Null))
                        .with_visibility(Visibility::Protected),
                )
                .with_method(Method::new("staticAction", returns(Value::Null)).into_static()),
        )
        .unwrap();
    registry
        .declare_class(
            Class::new(name("InvokableController"))
                .with_attributes([AS_CONTROLLER])
                .with_method(Method::new(
                    "__invoke",
                    returns(Value::Null).with_parameters([
                        Parameter::new("foo"),
                        Parameter::new("bar").with_default(Value::Null),
                    ]),
                )),
        )
        .unwrap();
    registry
        .declare_class(
            Class::of_kind(ClassKind::AbstractClass, name("TestAbstractController")).with_method(
                Method::new("staticAction", returns(Value::string("foo"))).into_static(),
            ),
        )
        .unwrap();
    registry
        .declare_class(
            Class::new(name("PrivateConstructorController"))
                .with_method(
                    Method::new("__construct", returns(Value::Null))
                        .with_visibility(Visibility::Private),
                )
                .with_method(
                    Method::new("staticAction", returns(Value::string("bar"))).into_static(),
                ),
        )
        .unwrap();
    T::declare_test_file_classes(&mut registry);
    Arc::new(registry)
}

pub fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

pub fn set(request: &RequestHandle, key: &str, value: Value) {
    request.with_mut(|r| r.attributes_mut().set(key, value));
}

pub fn get_controller(
    resolver: &ControllerResolver,
    request: &RequestHandle,
) -> ResolvedController {
    resolver.resolve(request).unwrap().unwrap()
}

pub fn new_object(resolver: &ControllerResolver, short: &str) -> Value {
    resolver
        .classes()
        .new_instance(name(short).as_bytes())
        .unwrap()
}

pub fn class_of_value(value: &Value) -> String {
    match value {
        Value::Object(o) => o.class().to_owned(),
        _ => panic!("not an object"),
    }
}

pub fn call(resolved: &ResolvedController) -> Value {
    match resolved.controller.call(&[]).unwrap() {
        ControllerResult::Value(v) => v,
        ControllerResult::Response(_) => panic!("a response"),
    }
}

/// `$this->createMock(LoggerInterface::class)` with
/// `->expects($this->once())->method('warning')->with(...)`.
#[derive(Default)]
struct Logger(Mutex<Vec<(LogLevel, String)>>);

impl LoggerInterface for Logger {
    fn log(&self, level: LogLevel, message: &str, _: Context) {
        self.0.lock().unwrap().push((level, message.to_owned()));
    }
}

pub fn testGetControllerWithoutControllerParameter<T: TestClass>() {
    let logger = Arc::new(Logger::default());
    let resolver = createControllerResolver::<T>(Some(logger.clone()));

    let request = create("/");
    assert!(
        resolver.resolve(&request).unwrap().is_none(),
        "->getController() returns false when the request has no _controller attribute"
    );
    let calls = logger.0.lock().unwrap();
    assert_eq!(1, calls.len());
    assert_eq!(LogLevel::Warning, calls[0].0);
    assert_eq!(
        "Unable to look for the controller as the \"_controller\" parameter is missing.",
        calls[0].1
    );
}

pub fn testGetControllerWithLambda<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    let lambda = ClosureObject::anonymous(returns(Value::Null), Vec::<String>::new()).object();
    set(&request, "_controller", lambda.clone());
    let controller = get_controller(&resolver, &request);
    assert_eq!(lambda, controller.callable);
}

pub fn testGetControllerWithObjectAndInvokeMethod<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);
    let object = new_object(&resolver, "InvokableController");

    let request = create("/");
    set(&request, "_controller", object.clone());
    let controller = get_controller(&resolver, &request);
    assert_eq!(object, controller.callable);
}

pub fn testGetControllerWithObjectAndMethod<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);
    let object = new_object(&resolver, "ControllerTest");

    let request = create("/");
    let callable = Value::Array(PhpArray::list([object, Value::string("publicAction")]));
    set(&request, "_controller", callable.clone());
    let controller = get_controller(&resolver, &request);
    assert_eq!(callable, controller.callable);
}

fn assert_class_and_method(controller: &ResolvedController, class: &str, method: &str) {
    let Value::Array(array) = &controller.callable else {
        panic!("an array callable")
    };
    let values: Vec<&Value> = array.values().collect();
    assert_eq!(name(class), class_of_value(values[0]));
    assert_eq!(&Value::string(method), values[1]);
}

pub fn testGetControllerWithClassAndMethodAsArray<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    set(
        &request,
        "_controller",
        Value::Array(PhpArray::list([
            Value::string(name("ControllerTest")),
            Value::string("publicAction"),
        ])),
    );
    let controller = get_controller(&resolver, &request);
    assert_class_and_method(&controller, "ControllerTest", "publicAction");
}

pub fn testGetControllerWithClassAndMethodAsString<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    set(
        &request,
        "_controller",
        Value::string(format!("{}::publicAction", name("ControllerTest"))),
    );
    let controller = get_controller(&resolver, &request);
    assert_class_and_method(&controller, "ControllerTest", "publicAction");
}

pub fn testGetControllerWithInvokableClass<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    set(
        &request,
        "_controller",
        Value::string(name("InvokableController")),
    );
    let controller = get_controller(&resolver, &request);
    assert_eq!(
        name("InvokableController"),
        class_of_value(&controller.callable)
    );
}

pub fn testGetControllerOnObjectWithoutInvokeMethod<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    set(&request, "_controller", StdClass::object(PhpArray::new()));
    let e = resolver.resolve(&request).unwrap_err();
    assert!(instance_of(e.as_ref(), "InvalidArgumentException"));
}

pub fn testGetControllerWithFunction<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    let function = "Symfony\\Component\\HttpKernel\\Tests\\Controller\\some_controller_function";
    set(&request, "_controller", Value::string(function));
    let controller = get_controller(&resolver, &request);
    assert_eq!(Value::string(function), controller.callable);
}

pub fn testGetControllerWithClosure<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let closure =
        ClosureObject::anonymous(returns(Value::string("test")), Vec::<String>::new()).object();

    let request = create("/");
    set(&request, "_controller", closure);
    let controller = get_controller(&resolver, &request);
    assert_eq!("Closure", class_of_value(&controller.callable));
    assert_eq!(Value::string("test"), call(&controller));
}

fn getStaticControllers() -> Vec<(Value, &'static str)> {
    let pair = |class: &str| {
        Value::Array(PhpArray::list([
            Value::string(name(class)),
            Value::string("staticAction"),
        ]))
    };
    vec![
        (
            Value::string(format!("{}::staticAction", name("TestAbstractController"))),
            "foo",
        ),
        (pair("TestAbstractController"), "foo"),
        (
            Value::string(format!(
                "{}::staticAction",
                name("PrivateConstructorController")
            )),
            "bar",
        ),
        (pair("PrivateConstructorController"), "bar"),
    ]
}

pub fn testGetControllerWithStaticController<T: TestClass>() {
    for (static_controller, return_value) in getStaticControllers() {
        let resolver = createControllerResolver::<T>(None);

        let request = create("/");
        set(&request, "_controller", static_controller.clone());
        let controller = get_controller(&resolver, &request);
        assert_eq!(static_controller, controller.callable);
        assert_eq!(Value::string(return_value), call(&controller));
    }
}

pub fn testGetControllerWithUndefinedController<T: TestClass>() {
    let cases = T::getUndefinedControllers(&createControllerResolver::<T>(None));
    assert_eq!(T::UNDEFINED_CONTROLLER_COUNT, cases.len());
    for (controller, exception_name, exception_message) in cases {
        let resolver = createControllerResolver::<T>(None);

        let request = create("/");
        set(&request, "_controller", controller);
        let e: Throwable = resolver.resolve(&request).unwrap_err();
        assert!(
            instance_of(e.as_ref(), exception_name),
            "{exception_name}: {e}"
        );
        let message = message_of(e.as_ref()).to_string_lossy().into_owned();
        assert!(message.contains(&exception_message), "{message}");
    }
}

pub fn testAllowedControllerTypes<T: TestClass>() {
    let mut resolver = createControllerResolver::<T>(None);

    let request = create("/");
    let controller = new_object(&resolver, "ControllerTest");
    let callable = Value::Array(PhpArray::list([
        controller.clone(),
        Value::string("publicAction"),
    ]));
    set(&request, "_controller", callable.clone());
    set(&request, "_check_controller_is_allowed", Value::Bool(true));

    let e = resolver.resolve(&request).unwrap_err();
    assert!(instance_of(e.as_ref(), BAD_REQUEST));

    resolver.allow_controllers([name("ControllerTest")], Vec::<String>::new());

    assert_eq!(callable, get_controller(&resolver, &request).callable);

    // `$controller->publicAction(...)`
    let action = resolver.classes().closure_from_callable(&callable).unwrap();
    set(&request, "_controller", action.clone());
    assert_eq!(action, get_controller(&resolver, &request).callable);
}

pub fn testAllowedControllerAttributes<T: TestClass>() {
    let mut resolver = createControllerResolver::<T>(None);

    let request = create("/");
    let function = name("some_controller_function");
    // `some_controller_function(...)`
    let controller = resolver
        .classes()
        .closure_from_callable(&Value::string(function.as_str()))
        .unwrap();
    set(&request, "_controller", controller.clone());
    set(&request, "_check_controller_is_allowed", Value::Bool(true));

    let e = resolver.resolve(&request).unwrap_err();
    assert!(instance_of(e.as_ref(), BAD_REQUEST));

    resolver.allow_controllers(Vec::<String>::new(), [name("DummyController")]);

    assert_eq!(controller, get_controller(&resolver, &request).callable);

    // `some_controller_function::class`
    let controller = Value::string(function);
    set(&request, "_controller", controller.clone());
    assert_eq!(controller, get_controller(&resolver, &request).callable);
}

pub fn testAllowedAsControllerAttribute<T: TestClass>() {
    let resolver = createControllerResolver::<T>(None);

    let request = create("/");
    let controller = new_object(&resolver, "InvokableController");
    let callable = Value::Array(PhpArray::list([
        controller.clone(),
        Value::string("__invoke"),
    ]));
    set(&request, "_controller", callable.clone());
    set(&request, "_check_controller_is_allowed", Value::Bool(true));

    assert_eq!(callable, get_controller(&resolver, &request).callable);

    set(&request, "_controller", controller.clone());
    assert_eq!(controller, get_controller(&resolver, &request).callable);
}

pub fn parent_getUndefinedControllers(
    resolver: &ControllerResolver,
) -> Vec<(Value, &'static str, String)> {
    let controller = new_object(resolver, "ControllerTest");
    let s = |text: &str| Value::string(text);
    let pair = |a: Value, b: &str| Value::Array(PhpArray::list([a, Value::string(b)]));
    let class = name("ControllerTest");
    let iae = "InvalidArgumentException";
    vec![
        (s("foo"), "Error", "Class \"foo\" not found".to_owned()),
        (s("oof::bar"), "Error", "Class \"oof\" not found".to_owned()),
        (pair(s("oof"), "bar"), "Error", "Class \"oof\" not found".to_owned()),
        (s(&format!("{class}::staticsAction")), iae, format!("The controller for URI \"/\" is not callable: Expected method \"staticsAction\" on class \"{class}\", did you mean \"staticAction\"?")),
        (s(&format!("{class}::privateAction")), iae, format!("The controller for URI \"/\" is not callable: Method \"privateAction\" on class \"{class}\" should be public and non-abstract")),
        (s(&format!("{class}::protectedAction")), iae, format!("The controller for URI \"/\" is not callable: Method \"protectedAction\" on class \"{class}\" should be public and non-abstract")),
        (s(&format!("{class}::undefinedAction")), iae, format!("The controller for URI \"/\" is not callable: Expected method \"undefinedAction\" on class \"{class}\". Available methods: \"publicAction\", \"staticAction\"")),
        (s(&class), iae, format!("The controller for URI \"/\" is not callable: Controller class \"{class}\" cannot be called without a method name. You need to implement \"__invoke\" or use one of the available methods: \"publicAction\", \"staticAction\".")),
        (pair(controller.clone(), "staticsAction"), iae, format!("The controller for URI \"/\" is not callable: Expected method \"staticsAction\" on class \"{class}\", did you mean \"staticAction\"?")),
        (pair(controller.clone(), "privateAction"), iae, format!("The controller for URI \"/\" is not callable: Method \"privateAction\" on class \"{class}\" should be public and non-abstract")),
        (pair(controller.clone(), "protectedAction"), iae, format!("The controller for URI \"/\" is not callable: Method \"protectedAction\" on class \"{class}\" should be public and non-abstract")),
        (pair(controller.clone(), "undefinedAction"), iae, format!("The controller for URI \"/\" is not callable: Expected method \"undefinedAction\" on class \"{class}\". Available methods: \"publicAction\", \"staticAction\"")),
        (controller, iae, format!("The controller for URI \"/\" is not callable: Controller class \"{class}\" cannot be called without a method name. You need to implement \"__invoke\" or use one of the available methods: \"publicAction\", \"staticAction\".")),
        (
            {
                let mut a = PhpArray::new();
                a.insert("a", s("foo"));
                a.insert("b", s("bar"));
                Value::Array(a)
            },
            iae,
            "The controller for URI \"/\" is not callable: Invalid array callable, expected [controller, method].".to_owned(),
        ),
    ]
}
