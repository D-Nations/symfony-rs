//! ContainerControllerResolverTest.php at symfony/http-kernel v8.1.11. Names
//! and assertions are PHPUnit's. It extends `ControllerResolverTest`, so the
//! parent's test methods (`common/controller_resolver_test_class.rs`) run
//! here too, on a `ContainerControllerResolver`. The test file's
//! classes (`InvokableControllerService`, `ControllerTestService`, each with
//! a required constructor argument) are declared in the `ClassRegistry`.
//! `new Container()` with `set()` is [`Container`], a
//! `DependencyInjectionContainer` with no removed ids; the two
//! `createMock(Container::class)` cases are [`MockContainer`], which
//! counts its calls as the mock's expectations do.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ContainerControllerResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

#[path = "common/controller_resolver_test_class.rs"]
mod test_class;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use php_value::{PhpArray, Value};
use psr_log::LoggerInterface;
use symfony_http_kernel::{
    controller::Parameter,
    controller_resolver::{Class, ClassRegistry, ControllerResolver, Method, ResolvedController},
    exception::{instance_of, message_of},
};
use test_class::{
    TestClass, class_of_value, create, get_controller, name, parent_getUndefinedControllers,
    program, returns, set,
};

use psr_container::{ContainerInterface, Throwable as ContainerThrowable};
use symfony_http_kernel::container_controller_resolver::{
    ContainerControllerResolver, ControllerContainer, DependencyInjectionContainer,
};

/// `Symfony\Component\DependencyInjection\Container` as the test uses it:
/// `set($id, $service)`, then `has()` and `get()` of what was set.
#[derive(Default)]
struct Container(Mutex<HashMap<Vec<u8>, Value>>);

impl Container {
    fn set(&self, id: &str, service: Value) {
        self.0
            .lock()
            .unwrap()
            .insert(id.as_bytes().to_vec(), service);
    }
}

impl ContainerInterface for Container {
    fn get(&self, id: &[u8]) -> Result<Value, ContainerThrowable> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| Arc::new(std::io::Error::other("not found")) as ContainerThrowable)
    }

    fn has(&self, id: &[u8]) -> bool {
        self.0.lock().unwrap().contains_key(id)
    }
}

impl DependencyInjectionContainer for Container {
    fn get_removed_ids(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// `$this->createMock(Container::class)`: `has()` answers `false`,
/// `getRemovedIds()` the given ids; both record their calls.
struct MockContainer {
    removed_ids: PhpArray,
    has_calls: Mutex<Vec<Vec<u8>>>,
    get_removed_ids_calls: Mutex<usize>,
}

impl MockContainer {
    fn new(removed: &str) -> Self {
        let mut removed_ids = PhpArray::new();
        removed_ids.insert(removed, Value::Bool(true));
        Self {
            removed_ids,
            has_calls: Mutex::default(),
            get_removed_ids_calls: Mutex::default(),
        }
    }
}

impl ContainerInterface for MockContainer {
    fn get(&self, _: &[u8]) -> Result<Value, ContainerThrowable> {
        panic!("get() is not expected")
    }

    fn has(&self, id: &[u8]) -> bool {
        self.has_calls.lock().unwrap().push(id.to_vec());
        false
    }
}

impl DependencyInjectionContainer for MockContainer {
    fn get_removed_ids(&self) -> PhpArray {
        *self.get_removed_ids_calls.lock().unwrap() += 1;
        self.removed_ids.clone()
    }
}

fn resolver_with(
    logger: Option<Arc<dyn LoggerInterface>>,
    container: ControllerContainer,
) -> ContainerControllerResolver {
    ContainerControllerResolver::new(
        container,
        program::<ContainerControllerResolverTest>(),
        logger,
    )
}

/// `ContainerControllerResolverTest`.
struct ContainerControllerResolverTest;

impl TestClass for ContainerControllerResolverTest {
    /// The parent's 14 cases, three replaced and three added.
    const UNDEFINED_CONTROLLER_COUNT: usize = 17;

    /// The classes the test file declares.
    fn declare_test_file_classes(registry: &mut ClassRegistry) {
        // `public function __construct($bar)`: a mandatory argument to prevent
        // automatic instantiation.
        let constructor = |argument: &str| {
            Method::new(
                "__construct",
                returns(Value::Null).with_parameters([Parameter::new(argument)]),
            )
        };
        registry
            .declare_class(
                Class::new(name("InvokableControllerService"))
                    .with_method(constructor("bar"))
                    .with_method(Method::new("__invoke", returns(Value::Null))),
            )
            .unwrap();
        registry
            .declare_class(
                Class::new(name("ControllerTestService"))
                    .with_method(constructor("foo"))
                    .with_method(Method::new("action", returns(Value::Null))),
            )
            .unwrap();
    }

    /// `new ContainerControllerResolver($container ?? new Container(), $logger)`.
    fn createControllerResolver(
        logger: Option<Arc<dyn LoggerInterface>>,
        program: Arc<ClassRegistry>,
    ) -> ControllerResolver {
        ContainerControllerResolver::new(
            ControllerContainer::DependencyInjection(Arc::new(Container::default())),
            program,
            logger,
        )
        .into_inner()
    }

    fn getUndefinedControllers(
        resolver: &ControllerResolver,
    ) -> Vec<(Value, &'static str, String)> {
        let iae = "InvalidArgumentException";
        let mut tests = parent_getUndefinedControllers(resolver);
        let pair =
            |a: &str, b: &str| Value::Array(PhpArray::list([Value::string(a), Value::string(b)]));
        tests[0] = (
            Value::string("foo"),
            iae,
            "Controller \"foo\" does neither exist as service nor as class".to_owned(),
        );
        tests[1] = (
            Value::string("oof::bar"),
            iae,
            "Controller \"oof\" does neither exist as service nor as class".to_owned(),
        );
        tests[2] = (
            pair("oof", "bar"),
            iae,
            "Controller \"oof\" does neither exist as service nor as class".to_owned(),
        );
        let service = name("ControllerTestService");
        let invokable = name("InvokableControllerService");
        let required = |class: &str| {
            format!(
                "Controller \"{class}\" has required constructor arguments and does not exist in the container. Did you forget to define the controller as a service?"
            )
        };
        tests.push((pair(&service, "action"), iae, required(&service)));
        tests.push((
            Value::string(format!("{service}::action")),
            iae,
            required(&service),
        ));
        tests.push((Value::string(invokable.as_str()), iae, required(&invokable)));
        tests
    }
}

/// `new ControllerTestService('foo')` and the like: an instance whose
/// constructor is not run again.
fn service(short: &str) -> Value {
    program::<ContainerControllerResolverTest>()
        .new_instance(name(short).as_bytes())
        .unwrap()
}

fn same_object(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

fn first_and_second(controller: &ResolvedController) -> (Value, Value) {
    let Value::Array(array) = &controller.callable else {
        panic!("an array callable")
    };
    let values: Vec<&Value> = array.values().collect();
    (values[0].clone(), values[1].clone())
}

#[test]
fn testGetControllerService() {
    let service = service("ControllerTestService");

    let container = Arc::new(Container::default());
    container.set("foo", service.clone());

    let resolver = resolver_with(None, ControllerContainer::DependencyInjection(container));
    let request = create("/");
    set(&request, "_controller", Value::string("foo::action"));

    let controller = get_controller(&resolver, &request);

    let (object, method) = first_and_second(&controller);
    assert!(same_object(&service, &object));
    assert_eq!(Value::string("action"), method);
}

#[test]
fn testGetControllerInvokableService() {
    let service = service("InvokableControllerService");

    let container = Arc::new(Container::default());
    container.set("foo", service.clone());

    let resolver = resolver_with(None, ControllerContainer::DependencyInjection(container));
    let request = create("/");
    set(&request, "_controller", Value::string("foo"));

    let controller = get_controller(&resolver, &request);

    assert!(same_object(&service, &controller.callable));
}

#[test]
fn testGetControllerInvokableServiceWithClassNameAsName() {
    let service = service("InvokableControllerService");

    let container = Arc::new(Container::default());
    container.set(&name("InvokableControllerService"), service.clone());

    let resolver = resolver_with(None, ControllerContainer::DependencyInjection(container));
    let request = create("/");
    set(
        &request,
        "_controller",
        Value::string(name("InvokableControllerService")),
    );

    let controller = get_controller(&resolver, &request);

    assert!(same_object(&service, &controller.callable));
}

fn getControllers() -> Vec<String> {
    vec![format!("\\{}::action", name("ControllerTestService"))]
}

#[test]
fn testInstantiateControllerWhenControllerStartsWithABackslash() {
    for controller in getControllers() {
        let service = service("ControllerTestService");
        let class = name("ControllerTestService");

        let container = Arc::new(Container::default());
        container.set(&class, service);

        let resolver = resolver_with(None, ControllerContainer::DependencyInjection(container));
        let request = create("/");
        set(&request, "_controller", Value::string(controller));

        let controller = get_controller(&resolver, &request);

        let (object, method) = first_and_second(&controller);
        assert_eq!(class, class_of_value(&object));
        assert_eq!(Value::string("action"), method);
    }
}

/// `expectException(\InvalidArgumentException::class)` and
/// `expectExceptionMessage()` for a removed controller, with the mock's
/// `has()` called once with `id` and `getRemovedIds()` at least once.
fn assert_removed(id: &str, controller: Value) {
    let container = Arc::new(MockContainer::new(id));
    let resolver = resolver_with(
        None,
        ControllerContainer::DependencyInjection(container.clone()),
    );
    let request = create("/");
    set(&request, "_controller", controller);

    let e = resolver.resolve(&request).unwrap_err();
    assert!(instance_of(e.as_ref(), "InvalidArgumentException"), "{e}");
    let message = message_of(e.as_ref()).to_string_lossy().into_owned();
    let expected = format!(
        "Controller \"{id}\" cannot be fetched from the container because it is private. Did you forget to tag the service with \"controller.service_arguments\"?"
    );
    assert!(message.contains(&expected), "{message}");

    assert_eq!(
        vec![id.as_bytes().to_vec()],
        *container.has_calls.lock().unwrap()
    );
    assert!(*container.get_removed_ids_calls.lock().unwrap() >= 1);
}

#[test]
fn testExceptionWhenUsingRemovedControllerServiceWithClassNameAsName() {
    let class = name("ControllerTestService");
    assert_removed(
        &class,
        Value::Array(PhpArray::list([
            Value::string(class.as_str()),
            Value::string("action"),
        ])),
    );
}

#[test]
fn testExceptionWhenUsingRemovedControllerService() {
    assert_removed("app.my_controller", Value::string("app.my_controller"));
}

#[test]
fn testGetControllerWithoutControllerParameter() {
    test_class::testGetControllerWithoutControllerParameter::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithLambda() {
    test_class::testGetControllerWithLambda::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithObjectAndInvokeMethod() {
    test_class::testGetControllerWithObjectAndInvokeMethod::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithObjectAndMethod() {
    test_class::testGetControllerWithObjectAndMethod::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithClassAndMethodAsArray() {
    test_class::testGetControllerWithClassAndMethodAsArray::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithClassAndMethodAsString() {
    test_class::testGetControllerWithClassAndMethodAsString::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithInvokableClass() {
    test_class::testGetControllerWithInvokableClass::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerOnObjectWithoutInvokeMethod() {
    test_class::testGetControllerOnObjectWithoutInvokeMethod::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithFunction() {
    test_class::testGetControllerWithFunction::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithClosure() {
    test_class::testGetControllerWithClosure::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithStaticController() {
    test_class::testGetControllerWithStaticController::<ContainerControllerResolverTest>();
}

#[test]
fn testGetControllerWithUndefinedController() {
    test_class::testGetControllerWithUndefinedController::<ContainerControllerResolverTest>();
}

#[test]
fn testAllowedControllerTypes() {
    test_class::testAllowedControllerTypes::<ContainerControllerResolverTest>();
}

#[test]
fn testAllowedControllerAttributes() {
    test_class::testAllowedControllerAttributes::<ContainerControllerResolverTest>();
}

#[test]
fn testAllowedAsControllerAttribute() {
    test_class::testAllowedAsControllerAttribute::<ContainerControllerResolverTest>();
}
