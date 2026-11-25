//! Port of Symfony\Component\DependencyInjection\Tests\ContainerTest from
//! Tests/ContainerTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's; data providers are loops.
//!
//! The `ProjectServiceContainer` fixture is a [`ContainerClass`] whose
//! factory methods are closures; `services9_compiled.php` is reduced to
//! what the two tests read (its synthetic ids and removed ids). `stdClass`
//! services are [`ServiceObject`]s, which have identity and properties.
//!
//! `testResetReleasesTheClosuresThatReferenceTheContainer` runs the
//! fallback `EnvVarProcessor` path with a stand-in processor that reads
//! `$_ENV` from a map: `EnvVarProcessor` is ported separately.
//! `testThatCloningIsNotSupported` checks that `Container` has no
//! `Clone` impl.
//!
//! Pending:
//! - `testResetDoesNotInitializeLazyServices`: needs `ContainerBuilder`
//!   (`register()`, `setLazy()`, `compile()`), ported separately.
//!
//! Upstream: symfony/dependency-injection Tests/ContainerTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::needless_pass_by_value,
    clippy::unused_self
)]

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Weak};

use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::container::{ContainerClass, EnvProcessor};
use symfony_dependency_injection::object::downcast;
use symfony_dependency_injection::service_object::RESET_INTERFACE;
use symfony_dependency_injection::{
    Container, Error, ErrorKind, InvalidBehavior, ParameterBag, ServiceFactory, ServiceObject,
};

fn std_class() -> Value {
    ServiceObject::new("stdClass").into_value()
}

fn same(a: &Value, b: &Value) -> bool {
    a.identical(b)
}

fn bag(parameters: PhpArray) -> ParameterBag {
    ParameterBag::new(&parameters).unwrap()
}

fn exception(class: &str, message: &str) -> Error {
    Error::new(ErrorKind::Other(class.to_owned()), message)
}

/// `ProjectServiceContainer` and its public `$__bar`, `$__foo_bar`,
/// `$__foo_baz` and `$__internal`.
struct Project {
    c: Arc<Container>,
    bar: Value,
    foo_bar: Value,
    foo_baz: Value,
}

fn method(
    f: impl Fn(&Container) -> symfony_dependency_injection::Result<Value> + Send + Sync + 'static,
) -> ServiceFactory {
    Arc::new(f)
}

fn project() -> Project {
    let bar = std_class();
    let foo_bar = std_class();
    let foo_baz = std_class();
    let internal = std_class();

    let get_internal_service = {
        let internal = internal.clone();
        move |c: &Container| c.store_private("internal", internal.clone())
    };

    let class = ContainerClass {
        class: Some(
            "Symfony\\Component\\DependencyInjection\\Tests\\ProjectServiceContainer".to_owned(),
        ),
        aliases: vec![("alias".to_owned(), "bar".to_owned())],
        method_map: [
            ("bar", "getBarService"),
            ("foo_bar", "getFooBarService"),
            ("foo.baz", "getFoo_BazService"),
            ("circular", "getCircularService"),
            ("throw_exception", "getThrowExceptionService"),
            (
                "throws_exception_on_service_configuration",
                "getThrowsExceptionOnServiceConfigurationService",
            ),
            (
                "throws_error_on_service_configuration",
                "getThrowsErrorOnServiceConfigurationService",
            ),
            (
                "throws_exception_on_private_service_configuration",
                "getThrowsExceptionOnPrivateServiceConfigurationService",
            ),
            ("internal_dependency", "getInternalDependencyService"),
        ]
        .into_iter()
        .map(|(id, m)| (id.to_owned(), m.to_owned()))
        .collect(),
        methods: vec![
            ("getBarService".to_owned(), {
                let bar = bar.clone();
                method(move |c| Ok(c.store_service("bar", bar.clone())))
            }),
            ("getFooBarService".to_owned(), {
                let foo_bar = foo_bar.clone();
                method(move |_| Ok(foo_bar.clone()))
            }),
            ("getFoo_BazService".to_owned(), {
                let foo_baz = foo_baz.clone();
                method(move |_| Ok(foo_baz.clone()))
            }),
            (
                "getCircularService".to_owned(),
                method(|c| c.get_service_value("circular")),
            ),
            (
                "getThrowExceptionService".to_owned(),
                method(|_| Err(exception("Exception", "Something went terribly wrong!"))),
            ),
            (
                "getThrowsExceptionOnServiceConfigurationService".to_owned(),
                method(|c| {
                    c.store_service("throws_exception_on_service_configuration", std_class());
                    Err(exception(
                        "Exception",
                        "Something was terribly wrong while trying to configure the service!",
                    ))
                }),
            ),
            (
                "getThrowsErrorOnServiceConfigurationService".to_owned(),
                method(|c| {
                    c.store_service("throws_error_on_service_configuration", std_class());
                    Err(exception(
                        "Error",
                        "Something was terribly wrong while trying to configure the service!",
                    ))
                }),
            ),
            (
                "getThrowsExceptionOnPrivateServiceConfigurationService".to_owned(),
                method(|c| {
                    c.store_private(
                        "throws_exception_on_private_service_configuration",
                        std_class(),
                    );
                    Err(exception(
                        "Exception",
                        "Something was terribly wrong while trying to configure the private service!",
                    ))
                }),
            ),
            (
                "getInternalDependencyService".to_owned(),
                method(move |c| {
                    let instance = c.store_service("internal_dependency", std_class());
                    let internal = match c.private_service("internal") {
                        Some(internal) => internal,
                        None => get_internal_service(c),
                    };
                    if let Some(object) = downcast::<ServiceObject>(&instance) {
                        object.set_property("internal", internal);
                    }
                    Ok(instance)
                }),
            ),
        ],
        ..ContainerClass::default()
    };

    Project {
        c: Container::from_class(None, class),
        bar,
        foo_bar,
        foo_baz,
    }
}

/// The parts of `services9_compiled.php` the tests read.
fn services9_compiled() -> Arc<Container> {
    Container::from_class(
        None,
        ContainerClass {
            class: Some("ProjectServiceContainer".to_owned()),
            synthetic_ids: vec!["request".to_owned()],
            removed_ids: [
                "a_factory",
                "configurator_service",
                "configurator_service_simple",
                "decorated.pif-pouf",
                "decorator_service.inner",
                "errored_definition",
                "factory_simple",
                "inlined",
                "new_factory",
                "tagged_iterator_foo",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            ..ContainerClass::default()
        },
    )
}

fn get(c: &Container, id: &str) -> Value {
    c.get(id, InvalidBehavior::Exception).unwrap().unwrap()
}

#[test]
fn testConstructor() {
    let sc = Container::new(None);
    assert!(
        same(&sc.to_value(), &get(&sc, "service_container")),
        "__construct() automatically registers itself as a service"
    );

    let sc = Container::new(Some(bag(php_array!["foo" => "bar"])));
    assert_eq!(
        &php_array!["foo" => "bar"],
        sc.get_parameter_bag().all(),
        "__construct() takes an array of parameters as its first argument"
    );
}

#[test]
fn testCamelize() {
    for (id, expected) in [
        ("foo_bar", "FooBar"),
        ("foo.bar", "Foo_Bar"),
        ("foo.bar_baz", "Foo_BarBaz"),
        ("foo._bar", "Foo_Bar"),
        ("foo_.bar", "Foo_Bar"),
        ("_foo", "Foo"),
        (".foo", "_Foo"),
        ("foo_", "Foo"),
        ("foo.", "Foo_"),
        ("foo\\bar", "Foo_Bar"),
    ] {
        assert_eq!(
            expected,
            Container::camelize(id),
            "Container::camelize(\"{id}\")"
        );
    }
}

#[test]
fn testUnderscore() {
    for (id, expected) in [
        ("FooBar", "foo_bar"),
        ("Foo_Bar", "foo.bar"),
        ("Foo_BarBaz", "foo.bar_baz"),
        ("FooBar_BazQux", "foo_bar.baz_qux"),
        ("_Foo", ".foo"),
        ("Foo_", "foo."),
    ] {
        assert_eq!(
            expected,
            Container::underscore(id),
            "Container::underscore(\"{id}\")"
        );
    }
}

#[test]
fn testCompile() {
    let sc = Container::new(Some(bag(php_array!["foo" => "bar"])));
    assert!(
        !sc.get_parameter_bag().is_resolved(),
        "->compile() resolves the parameter bag"
    );
    sc.compile().unwrap();
    assert!(
        sc.get_parameter_bag().is_resolved(),
        "->compile() resolves the parameter bag"
    );
    assert!(
        sc.get_parameter_bag().is_frozen(),
        "->compile() changes the parameter bag to a FrozenParameterBag instance"
    );
    assert_eq!(
        &php_array!["foo" => "bar"],
        sc.get_parameter_bag().all(),
        "->compile() copies the current parameters to the new parameter bag"
    );
}

#[test]
fn testIsCompiled() {
    let sc = Container::new(Some(bag(php_array!["foo" => "bar"])));
    assert!(
        !sc.is_compiled(),
        "->isCompiled() returns false if the container is not compiled"
    );
    sc.compile().unwrap();
    assert!(
        sc.is_compiled(),
        "->isCompiled() returns true if the container is compiled"
    );
}

#[test]
fn testIsCompiledWithFrozenParameters() {
    let sc = Container::new(Some(ParameterBag::frozen(
        php_array!["foo" => "bar"],
        Vec::new(),
        Vec::new(),
    )));
    assert!(
        !sc.is_compiled(),
        "->isCompiled() returns false if the container is not compiled but the parameter bag is already frozen"
    );
}

#[test]
fn testGetParameterBag() {
    let sc = Container::new(None);
    assert!(
        sc.get_parameter_bag().all().is_empty(),
        "->getParameterBag() returns an empty array if no parameter has been defined"
    );
}

#[test]
fn testGetSetParameter() {
    let sc = Container::new(Some(bag(php_array!["foo" => "bar"])));
    sc.set_parameter("bar", Value::from("foo")).unwrap();
    assert!(
        same(&Value::from("foo"), &sc.get_parameter("bar").unwrap()),
        "->setParameter() sets the value of a new parameter"
    );

    sc.set_parameter("foo", Value::from("baz")).unwrap();
    assert!(
        same(&Value::from("baz"), &sc.get_parameter("foo").unwrap()),
        "->setParameter() overrides previously set parameter"
    );

    let e = sc.get_parameter("baba").unwrap_err();
    assert!(
        e.instance_of("InvalidArgumentException"),
        "->getParameter() thrown an \\InvalidArgumentException if the key does not exist"
    );
    assert_eq!(
        "You have requested a non-existent parameter \"baba\".",
        e.message(),
        "->getParameter() thrown an \\InvalidArgumentException if the key does not exist"
    );
}

#[test]
fn testGetSetParameterWithMixedCase() {
    let sc = Container::new(Some(bag(php_array!["foo" => "bar"])));

    sc.set_parameter("Foo", Value::from("baz1")).unwrap();
    assert!(same(&Value::from("bar"), &sc.get_parameter("foo").unwrap()));
    assert!(same(
        &Value::from("baz1"),
        &sc.get_parameter("Foo").unwrap()
    ));
}

#[test]
fn testGetServiceIds() {
    let sc = Container::new(None);
    sc.set("foo", Some(std_class())).unwrap();
    sc.set("bar", Some(std_class())).unwrap();
    assert_eq!(
        vec!["service_container", "foo", "bar"],
        sc.get_service_ids(),
        "->getServiceIds() returns all defined service ids"
    );

    let sc = project().c;
    sc.set("foo", Some(std_class())).unwrap();
    assert_eq!(
        vec![
            "service_container",
            "bar",
            "foo_bar",
            "foo.baz",
            "circular",
            "throw_exception",
            "throws_exception_on_service_configuration",
            "throws_error_on_service_configuration",
            "throws_exception_on_private_service_configuration",
            "internal_dependency",
            "alias",
            "foo"
        ],
        sc.get_service_ids(),
        "->getServiceIds() returns defined service ids by factory methods in the method map, followed by service ids defined by set()"
    );
}

#[test]
fn testSet() {
    let sc = Container::new(None);
    let foo = std_class();
    sc.set("._. \\o/", Some(foo.clone())).unwrap();
    assert!(same(&foo, &get(&sc, "._. \\o/")), "->set() sets a service");
}

#[test]
fn testSetWithNullResetTheService() {
    let sc = Container::new(None);
    sc.set("foo", Some(std_class())).unwrap();
    sc.set("foo", None).unwrap();
    assert!(
        !sc.has("foo"),
        "->set() with null service resets the service"
    );
}

#[test]
fn testSetReplacesAlias() {
    let c = project().c;

    let foo = std_class();
    c.set("alias", Some(foo.clone())).unwrap();
    assert!(
        same(&foo, &get(&c, "alias")),
        "->set() replaces an existing alias"
    );
}

#[test]
fn testSetWithNullOnInitializedPredefinedService() {
    let sc = Container::new(None);
    sc.set("foo", Some(std_class())).unwrap();
    sc.set("foo", None).unwrap();
    assert!(
        !sc.has("foo"),
        "->set() with null service resets the service"
    );

    let sc = project().c;
    get(&sc, "bar");
    let e = sc.set("bar", None).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The \"bar\" service is already initialized, you cannot replace it.",
        e.message()
    );
}

#[test]
fn testSetWithNullOnUninitializedPredefinedService() {
    let sc = Container::new(None);
    sc.set("foo", Some(std_class())).unwrap();
    get(&sc, "foo");
    sc.set("foo", None).unwrap();
    assert!(
        !sc.has("foo"),
        "->set() with null service resets the service"
    );

    let sc = project().c;
    sc.set("bar", None).unwrap();
    assert!(
        sc.has("bar"),
        "->set() with null service resets the pre-defined service"
    );
}

#[test]
fn testGet() {
    let p = project();
    let sc = &p.c;
    let foo = std_class();
    sc.set("foo", Some(foo.clone())).unwrap();
    assert!(
        same(&foo, &get(sc, "foo")),
        "->get() returns the service for the given id"
    );
    assert!(
        same(&p.bar, &get(sc, "bar")),
        "->get() returns the service for the given id"
    );
    assert!(
        same(&p.foo_bar, &get(sc, "foo_bar")),
        "->get() returns the service if a get*Method() is defined"
    );
    assert!(
        same(&p.foo_baz, &get(sc, "foo.baz")),
        "->get() returns the service if a get*Method() is defined"
    );

    let e = sc.get("", InvalidBehavior::Exception).unwrap_err();
    assert!(
        e.is(&ErrorKind::ServiceNotFound),
        "->get() throws a ServiceNotFoundException exception if the service is empty"
    );
    assert!(
        sc.get("", InvalidBehavior::Null).unwrap().is_none(),
        "->get() returns null if the service is empty"
    );
}

#[test]
fn testCaseSensitivity() {
    let sc = Container::new(None);
    let foo1 = std_class();
    let foo2 = std_class();
    sc.set("foo", Some(foo1.clone())).unwrap();
    sc.set("Foo", Some(foo2.clone())).unwrap();

    assert_eq!(
        vec!["service_container", "foo", "Foo"],
        sc.get_service_ids()
    );
    assert!(
        same(&foo1, &get(&sc, "foo")),
        "->get() returns the service for the given id, case-sensitively"
    );
    assert!(
        same(&foo2, &get(&sc, "Foo")),
        "->get() returns the service for the given id, case-sensitively"
    );
}

#[test]
fn testGetThrowServiceNotFoundException() {
    let sc = project().c;
    sc.set("foo", Some(std_class())).unwrap();
    sc.set("baz", Some(std_class())).unwrap();

    let e = sc.get("foo1", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "You have requested a non-existent service \"foo1\". Did you mean this: \"foo\"?",
        e.message(),
        "->get() throws an Symfony\\Component\\DependencyInjection\\Exception\\ServiceNotFoundException with some advices"
    );

    let e = sc.get("bag", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "You have requested a non-existent service \"bag\". Did you mean one of these: \"bar\", \"baz\"?",
        e.message(),
        "->get() throws an Symfony\\Component\\DependencyInjection\\Exception\\ServiceNotFoundException with some advices"
    );
}

#[test]
fn testGetCircularReference() {
    let sc = project().c;
    let e = sc.get("circular", InvalidBehavior::Exception).unwrap_err();
    assert!(
        e.is(&ErrorKind::ServiceCircularReference),
        "->get() throws a ServiceCircularReferenceException if it contains circular reference"
    );
    assert!(
        e.message()
            .starts_with("Circular reference detected for service \"circular\""),
        "->get() throws a \\LogicException if it contains circular reference"
    );
}

#[test]
fn testGetSyntheticServiceThrows() {
    let container = services9_compiled();

    let e = container
        .get("request", InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "The \"request\" service is synthetic, it needs to be set at boot time before it can be used.",
        e.message()
    );
}

#[test]
fn testGetRemovedServiceThrows() {
    let container = services9_compiled();

    let e = container
        .get("inlined", InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "The \"inlined\" service or alias has been removed or inlined when the container was compiled. You should either make it public, or stop using the container directly and use dependency injection instead.",
        e.message()
    );
}

#[test]
fn testHas() {
    let sc = project().c;
    sc.set("foo", Some(std_class())).unwrap();
    assert!(
        !sc.has("foo1"),
        "->has() returns false if the service does not exist"
    );
    assert!(sc.has("foo"), "->has() returns true if the service exists");
    assert!(
        sc.has("bar"),
        "->has() returns true if a get*Method() is defined"
    );
    assert!(
        sc.has("foo_bar"),
        "->has() returns true if a get*Method() is defined"
    );
    assert!(
        sc.has("foo.baz"),
        "->has() returns true if a get*Method() is defined"
    );
}

#[test]
fn testInitialized() {
    let sc = project().c;
    sc.set("foo", Some(std_class())).unwrap();
    assert!(
        sc.initialized("foo"),
        "->initialized() returns true if service is loaded"
    );
    assert!(
        !sc.initialized("foo1"),
        "->initialized() returns false if service is not loaded"
    );
    assert!(
        !sc.initialized("bar"),
        "->initialized() returns false if a service is defined, but not currently loaded"
    );
    assert!(
        !sc.initialized("alias"),
        "->initialized() returns false if an aliased service is not initialized"
    );

    get(&sc, "bar");
    assert!(
        sc.initialized("alias"),
        "->initialized() returns true for alias if aliased service is initialized"
    );
}

#[test]
fn testInitializedWithPrivateService() {
    let sc = project().c;
    get(&sc, "internal_dependency");
    assert!(!sc.initialized("internal"));
}

#[test]
fn testReset() {
    let c = Container::new(None);
    let reset_counter = Arc::new(AtomicI64::new(0));
    let bar = {
        let reset_counter = reset_counter.clone();
        ServiceObject::new("class@anonymous")
            .implements(RESET_INTERFACE)
            .method("reset", move |_, _| {
                reset_counter.fetch_add(1, Ordering::SeqCst);
                Ok(Value::Null)
            })
            .into_value()
    };
    c.set("bar", Some(bar)).unwrap();

    c.reset();

    assert!(c.get("bar", InvalidBehavior::Null).unwrap().is_none());
    assert_eq!(1, reset_counter.load(Ordering::SeqCst));
}

/// Stand-in for `EnvVarProcessor`'s `string` processing of `$_ENV`.
struct EnvStandIn(HashMap<String, String>);

impl EnvProcessor for EnvStandIn {
    fn get_env(
        &self,
        _prefix: &str,
        name: &str,
        _get_env: &dyn Fn(&str) -> symfony_dependency_injection::Result<Value>,
    ) -> symfony_dependency_injection::Result<Value> {
        self.0
            .get(name)
            .map(|v| Value::from(v.as_str()))
            .ok_or_else(|| {
                Error::env_not_found(format!("Environment variable not found: \"{name}\"."))
            })
    }

    fn get_provided_types() -> Vec<(String, String)> {
        vec![("string".to_owned(), "string".to_owned())]
    }
}

#[test]
fn testResetReleasesTheClosuresThatReferenceTheContainer() {
    let container = Container::new(None);
    let env: HashMap<String, String> =
        HashMap::from([("SF_CONTAINER_RESET_TEST".to_owned(), "foo".to_owned())]);
    let env = Arc::new(EnvStandIn(env));
    container.set_default_env_var_processor(Arc::new(move |_: &Container| {
        env.clone() as Arc<dyn EnvProcessor>
    }));

    assert!(same(
        &Value::from("foo"),
        &container.get_env("SF_CONTAINER_RESET_TEST").unwrap()
    ));

    container.reset();
    let r: Weak<Container> = Arc::downgrade(&container);
    drop(container);

    assert!(r.upgrade().is_none());
}

#[test]
fn testGetThrowsException() {
    let c = project().c;

    let _ = c.get("throw_exception", InvalidBehavior::Exception);

    // Retry, to make sure that get*Service() will be called.
    let e = c
        .get("throw_exception", InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.instance_of("Exception"));
    assert_eq!("Something went terribly wrong!", e.message());
}

#[test]
fn testGetThrowsExceptionOnServiceConfiguration() {
    let c = project().c;

    let _ = c.get(
        "throws_exception_on_service_configuration",
        InvalidBehavior::Exception,
    );

    assert!(!c.initialized("throws_exception_on_service_configuration"));

    // Retry, to make sure that get*Service() will be called.
    let _ = c.get(
        "throws_exception_on_service_configuration",
        InvalidBehavior::Exception,
    );
    assert!(!c.initialized("throws_exception_on_service_configuration"));
}

#[test]
fn testGetThrowsErrorOnServiceConfiguration() {
    let c = project().c;

    let e = c
        .get(
            "throws_error_on_service_configuration",
            InvalidBehavior::Exception,
        )
        .expect_err("->get() should throw the error raised while configuring the service");
    assert!(e.instance_of("Error"));

    assert!(!c.initialized("throws_error_on_service_configuration"));

    // Retry, to make sure that get*Service() will be called.
    let e = c
        .get("throws_error_on_service_configuration", InvalidBehavior::Exception)
        .expect_err(
            "->get() should throw the error again instead of returning a partially-configured service",
        );
    assert!(e.instance_of("Error"));
    assert!(!c.initialized("throws_error_on_service_configuration"));
}

#[test]
fn testGetThrowsExceptionOnPrivateServiceConfiguration() {
    let c = project().c;

    let _ = c.get(
        "throws_exception_on_private_service_configuration",
        InvalidBehavior::Exception,
    );

    assert!(
        !c.private_ids()
            .contains(&"throws_exception_on_private_service_configuration".to_owned())
    );

    // Retry, to make sure that get*Service() will be called.
    let _ = c.get(
        "throws_exception_on_private_service_configuration",
        InvalidBehavior::Exception,
    );
    assert!(
        !c.private_ids()
            .contains(&"throws_exception_on_private_service_configuration".to_owned())
    );
}

#[test]
fn testAlias() {
    let c = project().c;

    assert!(c.has("alias"));
    assert!(same(&get(&c, "alias"), &get(&c, "bar")));
}

/// `Probe::<T>::is_clone()`: the inherent method exists only for a
/// `Clone` type; otherwise the trait's default answers.
struct Probe<T>(PhantomData<T>);

trait NotClone {
    fn is_clone(&self) -> bool {
        false
    }
}

impl<T> NotClone for Probe<T> {}

#[allow(dead_code, reason = "selected only for Clone types")]
impl<T: Clone> Probe<T> {
    fn is_clone(&self) -> bool {
        true
    }
}

#[test]
fn testThatCloningIsNotSupported() {
    assert!(!Probe::<Container>(PhantomData).is_clone());
    assert!(Probe::<ParameterBag>(PhantomData).is_clone());
}

#[test]
fn testCheckExistenceOfAnInternalPrivateService() {
    let c = project().c;
    get(&c, "internal_dependency");
    assert!(!c.has("internal"));
}

#[test]
fn testRequestAnInternalSharedPrivateService() {
    let c = project().c;
    get(&c, "internal_dependency");

    let e = c.get("internal", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    // expectExceptionMessage() checks a substring.
    assert!(
        e.message()
            .contains("You have requested a non-existent service \"internal\".")
    );
}

#[test]
fn testGetEnvDoesNotAutoCastNullWithDefaultEnvVarProcessor() {
    let container = Container::new(None);
    container.set_parameter("env(FOO)", Value::Null).unwrap();
    container.compile().unwrap();

    assert!(container.get_env("FOO").unwrap().is_null());
}

#[test]
fn testGetEnvDoesNotAutoCastNullWithEnvVarProcessorsLocatorReturningDefaultEnvVarProcessor() {
    use symfony_dependency_injection::container::EnvProcessorObject;
    use symfony_dependency_injection::env_var_processor::EnvVarProcessor;
    use symfony_dependency_injection::service_locator::{LocatorEntry, ServiceLocator};

    let container = Container::new(None);
    container.set_parameter("env(FOO)", Value::Null).unwrap();
    let weak: Weak<Container> = Arc::downgrade(&container);
    let locator = ServiceLocator::new(vec![(
        "string".to_owned(),
        LocatorEntry::new(move |_| {
            let container = weak.upgrade().unwrap();
            Ok(Value::Object(Arc::new(EnvProcessorObject::new(
                "Symfony\\Component\\DependencyInjection\\EnvVarProcessor",
                Arc::new(EnvVarProcessor::new(container, None)),
            ))))
        }),
    )]);
    container
        .set(
            "container.env_var_processors_locator",
            Some(Value::Object(Arc::new(locator))),
        )
        .unwrap();
    container.compile().unwrap();

    assert!(container.get_env("FOO").unwrap().is_null());
}
