//! Tests/Compiler/AutowirePassTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; a data provider's cases
//! run in one test, in order. `assertEquals()` is PHP's `==`
//! (`loose_equals`); `assertEquals($class, $reference)` compares the
//! reference's id. The fixture classes of `autowiring_classes.php` (and
//! the files it includes) and of `Tests/Fixtures` are given as class
//! metadata; an object written in an attribute's arguments is the object
//! `new_autowire_attribute()` builds.
//!
//! Not portable:
//! - testParentClassNotFoundThrowsExceptionWithoutConfigComponent mocks
//!   `class_exists(ClassExistenceResource::class)` to false; the port
//!   always has the config component.
//!
//! Pending:
//! - creating services (`$container->get()`), which the build-time port
//!   refuses: testDecorationWithServiceAndAliasedInterface,
//!   testLegacyAutowireUnderscoreNamedArgument,
//!   testAutowireUnderscoreNamedArgument,
//!   testAutowireAttributeKeepsPercentEscaping,
//!   testLazyProxyForInterfaceWithFinalImplementation,
//!   testLazyProxyWithClassInheritance, testLazyProxyForDecoratedService;
//! - testAutowireAttribute: `Expression` values (no `ExpressionLanguage`),
//!   a user `Autowire` subclass (`CustomAutowire`, whose constructor is
//!   PHP code) and `$container->get()`.
//!
//! The tests after the upstream ones are extras, not upstream's: they
//! check what `AutowireAsDecoratorPass` alone leaves on the fixtures of
//! testAsDecoratorAttribute, testMultipleAsDecoratorAttribute and
//! testAsTagDecoratorAttribute.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AutowirePassTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]

use std::sync::Arc;

use php_value::compare::loose_equals;
use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::{
    IteratorArgument, ServiceClosureArgument, ServiceLocatorArgument,
};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata, Visibility,
};
use symfony_dependency_injection::compiler::{
    AutowireAsDecoratorPass, AutowirePass, AutowireRequiredMethodsPass, CompilerPassInterface,
    DecoratorServicePass, ResolveClassPass, TagDecoratorPass, new_autowire_attribute,
};
use symfony_dependency_injection::container_builder::EnvFormat;
use symfony_dependency_injection::{
    ContainerBuilder, ErrorKind, InvalidBehavior, Reference, Result,
};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\";
const REQUIRED: &str = "Symfony\\Contracts\\Service\\Attribute\\Required";
const LOGGER_INTERFACE: &str = "Psr\\Log\\LoggerInterface";
const NULL_LOGGER: &str = "Psr\\Log\\NullLogger";

fn c(short: &str) -> String {
    format!("{NS}{short}")
}

fn fx(short: &str) -> String {
    format!("{FIXTURES}{short}")
}

fn at(short: &str) -> String {
    format!("{ATTRIBUTE}{short}")
}

fn attribute(short: &str, arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: at(short),
        arguments,
    }
}

/// A parameter of the type `type_` (`""`: none).
fn p(name: &str, type_: &str) -> ParameterMetadata {
    ParameterMetadata::new(name, (!type_.is_empty()).then_some(type_))
}

fn with(mut parameter: ParameterMetadata, attribute: AttributeMetadata) -> ParameterMetadata {
    parameter.attributes.push(attribute);
    parameter
}

fn ctor(parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    MethodMetadata::public("__construct").with_parameters(parameters)
}

fn method(name: &str, parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    MethodMetadata::public(name).with_parameters(parameters)
}

fn required(mut method: MethodMetadata) -> MethodMetadata {
    method.attributes.push(AttributeMetadata {
        class: REQUIRED.to_string(),
        arguments: PhpArray::new(),
    });
    method
}

fn class(name: &str) -> ClassMetadata {
    ClassMetadata::class(name)
}

fn object(attribute: AttributeMetadata) -> Value {
    new_autowire_attribute(&attribute).unwrap()
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    let mut add = |class: ClassMetadata| {
        r.add_class(class);
    };
    let i = |s: &str| ClassMetadata::interface(&c(s));
    let k = |s: &str| class(&c(s));

    // PHP and PSR classes
    let mut std_class = class("stdClass");
    std_class.is_internal = true;
    add(std_class);
    let mut spl = class("SplFileObject").with_method(ctor(vec![
        p("filename", "string"),
        p("mode", "string").with_default(Value::from("r")),
        p("useIncludePath", "bool").with_default(Value::Bool(false)),
        p("context", "").with_default(Value::Null),
    ]));
    spl.is_internal = true;
    add(spl);
    add(ClassMetadata::interface(LOGGER_INTERFACE));
    add(class(NULL_LOGGER).with_interfaces(&[LOGGER_INTERFACE]));

    // autowiring_classes.php
    add(k("Foo").with_method(required(
        method(
            "cloneFoo",
            vec![p("bar", "?stdClass").with_default(Value::Null)],
        )
        .with_return_type("static"),
    )));
    add(k("Bar").with_method(ctor(vec![p("foo", &c("Foo"))])));
    add(i("AInterface"));
    add(k("A")
        .with_interfaces(&[&c("AInterface")])
        .with_method(method("create", vec![p("foo", &c("Foo"))]).with_static(true)));
    add(k("B").with_parent(&c("A")));
    add(k("C").with_method(ctor(vec![p("a", &c("A"))])));
    add(i("DInterface"));
    add(i("EInterface").with_interfaces(&[&c("DInterface")]));
    add(i("IInterface"));
    add(k("I").with_interfaces(&[&c("IInterface")]));
    add(k("F")
        .with_parent(&c("I"))
        .with_interfaces(&[&c("EInterface")]));
    add(k("G").with_method(ctor(vec![
        p("d", &c("DInterface")),
        p("e", &c("EInterface")),
        p("i", &c("IInterface")),
    ])));
    add(k("H").with_method(ctor(vec![p("b", &c("B")), p("d", &c("DInterface"))])));
    add(k("J").with_method(ctor(vec![p("i", &c("I"))])));
    add(k("K").with_method(ctor(vec![p("i", &c("IInterface"))])));
    add(i("CollisionInterface"));
    add(k("CollisionA").with_interfaces(&[&c("CollisionInterface")]));
    add(k("CollisionB").with_interfaces(&[&c("CollisionInterface")]));
    add(k("UnionClassesWithTarget").with_method(ctor(vec![with(
        p("any", &format!("{}|{}", c("CollisionA"), c("CollisionB"))),
        attribute("Target", php_array!["collision"]),
    )])));
    add(k("CannotBeAutowired").with_method(ctor(vec![p("collision", &c("CollisionInterface"))])));
    add(k("Lille"));
    add(k("Dunglas").with_method(ctor(vec![p("l", &c("Lille"))])));
    add(k("LesTilleuls").with_method(ctor(vec![p("j", &c("Dunglas")), p("k", &c("Dunglas"))])));
    add(k("BadTypeHintedArgument").with_method(ctor(vec![
        p("k", &c("Dunglas")),
        p("r", &c("NotARealClass")),
    ])));
    add(k("BadParentTypeHintedArgument").with_method(ctor(vec![
        p("k", &c("Dunglas")),
        p("r", &c("OptionalServiceClass")),
    ])));
    add(k("OptionalServiceClass").with_parent("Symfony\\Bug\\NotExistClass"));
    add(k("NotGuessableArgument").with_method(ctor(vec![p("k", &c("Foo"))])));
    add(k("NotGuessableArgumentForSubclass").with_method(ctor(vec![p("k", &c("A"))])));
    add(k("MultipleArguments").with_method(ctor(vec![
        p("k", &c("A")),
        p("foo", ""),
        p("dunglas", &c("Dunglas")),
        p("bar", "array"),
    ])));
    add(k("MultipleArgumentsOptionalScalar").with_method(ctor(vec![
        p("a", &c("A")),
        p("foo", "").with_default(Value::from("default_val")),
        p("lille", &format!("?{}", c("Lille"))).with_default(Value::Null),
    ])));
    add(
        k("MultipleArgumentsOptionalScalarLast").with_method(ctor(vec![
            p("a", &c("A")),
            p("lille", &c("Lille")),
            p("foo", "").with_default(Value::from("some_val")),
        ])),
    );
    add(k("SetterInjectionCollision").with_method(required(method(
        "setMultipleInstancesForOneArg",
        vec![p("collision", &c("CollisionInterface"))],
    ))));
    add(
        k("SetterInjectionCollisionWithTarget").with_method(required(method(
            "setMultipleInstancesForOneArg",
            vec![with(
                p("col", &c("CollisionInterface")),
                attribute("Target", php_array!["collision"]),
            )],
        ))),
    );
    add(k("SetterInjectionParent")
        .with_method(required(method(
            "setDependencies",
            vec![p("foo", &c("Foo")), p("a", &c("A"))],
        )))
        .with_method(method("notASetter", vec![p("a", &c("A"))]))
        .with_method(required(method(
            "setWithCallsConfigured",
            vec![p("a", &c("A"))],
        )))
        .with_method(required(method(
            "setChildMethodWithoutDocBlock",
            vec![p("a", &c("A"))],
        ))));
    add(k("SetterInjection")
        .with_parent(&c("SetterInjectionParent"))
        .with_method(required(method("setFoo", vec![p("foo", &c("Foo"))])))
        .with_method(required(method(
            "setDependencies",
            vec![p("foo", &c("Foo")), p("a", &c("A"))],
        )))
        .with_method(method("setWithCallsConfigured", vec![p("a", &c("A"))]))
        .with_method(method("notASetter", vec![p("a", &c("A"))])));
    add(k("AutowireSetter").with_method(required(
        method("setFoo", vec![p("foo", &c("Foo"))]).with_return_type("void"),
    )));
    add(k("NotWireable")
        .with_method(method(
            "setNotAutowireable",
            vec![p("n", &c("NotARealClass"))],
        ))
        .with_method(method(
            "setNotAutowireableBecauseOfATypo",
            vec![p("sam", &c("lesTilleuls"))],
        ))
        .with_method(method("setBar", vec![]))
        .with_method(method(
            "setOptionalNotAutowireable",
            vec![p("n", &format!("?{}", c("NotARealClass"))).with_default(Value::Null)],
        ))
        .with_method(method("setDifferentNamespace", vec![p("n", "stdClass")]))
        .with_method(method(
            "setOptionalNoTypeHint",
            vec![p("foo", "").with_default(Value::Null)],
        ))
        .with_method(method(
            "setOptionalArgNoAutowireable",
            vec![p("other", "").with_default(Value::from("default_val"))],
        ))
        .with_method(required(
            method("setProtectedMethod", vec![p("a", &c("A"))])
                .with_visibility(Visibility::Protected),
        )));
    add(k("PrivateConstructor").with_method(ctor(vec![]).with_visibility(Visibility::Private)));
    add(i("DecoratorInterface"));
    let decorator = |s: &str| k(s).with_interfaces(&[&c("DecoratorInterface")]);
    add(decorator("DecoratorImpl"));
    add(decorator("Decorated").with_method(ctor(vec![
        p("quz", "").with_default(Value::Null),
        p("nonExistent", "?NonExistent").with_default(Value::Null),
        p("decorated", &format!("?{}", c("DecoratorInterface"))).with_default(Value::Null),
        p("foo", "array").with_default(Value::Array(PhpArray::new())),
    ])));
    add(decorator("Decorator").with_method(ctor(vec![
        p("logger", LOGGER_INTERFACE),
        p("decorated", &c("DecoratorInterface")),
    ])));
    add(decorator("DecoratedDecorator")
        .with_method(ctor(vec![p("decorator", &c("DecoratorInterface"))])));
    add(decorator("NonAutowirableDecorator").with_method(ctor(vec![
        p("logger", LOGGER_INTERFACE),
        p("decorated1", &c("DecoratorInterface")),
        p("decorated2", &c("DecoratorInterface")),
    ])));
    let mut elsa = k("ElsaAction").with_method(ctor(vec![p("notExisting", &c("NotExisting"))]));
    elsa.is_final = true;
    add(elsa);
    add(k("ParametersLikeDefaultValue").with_method(ctor(vec![
        p("parameterLike", "string").with_default(Value::from("%not%one%parameter%here%")),
        p("willBeSetToKeepFirstArgumentDefaultValue", "string").with_default(Value::from("ok")),
    ])));

    // uniontype_classes.php, intersectiontype_classes.php, compositetype_classes.php
    add(k("UnionScalars").with_method(ctor(vec![p("timeout", "int|float")])));
    add(k("UnionClasses").with_method(ctor(vec![p(
        "collision",
        &format!("{}|{}", c("CollisionA"), c("CollisionB")),
    )])));
    add(k("UnionNull").with_method(ctor(vec![p(
        "c",
        &format!("{}|null", c("CollisionInterface")),
    )])));
    add(i("AnotherInterface"));
    let intersection = format!("{}&{}", c("CollisionInterface"), c("AnotherInterface"));
    add(k("IntersectionClasses").with_method(ctor(vec![p("collision", &intersection)])));
    add(
        k("IntersectionClassesWithTarget").with_method(ctor(vec![with(
            p("collision", &intersection),
            attribute("Target", php_array!["foo"]),
        )])),
    );
    add(i("YetAnotherInterface"));
    add(k("CompositeTypeClasses").with_method(ctor(vec![p(
        "collision",
        &format!("({intersection})|{}", c("YetAnotherInterface")),
    )])));
    add(k("NullableIntersection")
        .with_method(ctor(vec![p("a", &format!("({intersection})|null"))])));

    // autowiring_classes_80.php
    add(k("AutowireAttributeNullFallback").with_method(ctor(vec![
        with(
            p("required", "string"),
            attribute("Autowire", php_array!["%required.parameter%"]),
        ),
        with(
            p("optional", "?string").with_default(Value::Null),
            attribute("Autowire", php_array!["%optional.parameter%"]),
        ),
    ])));
    add(k("AutowireAttributeEnv").with_method(ctor(vec![
        with(
            p("enabled", "bool"),
            attribute("Autowire", php_array!["env" => "ENABLED"]),
        ),
        with(
            p("optional", "?string").with_default(Value::Null),
            attribute("Autowire", php_array!["env" => "OPTIONAL"]),
        ),
    ])));
    add(i("AsDecoratorInterface"));
    let as_decorator = |s: &str, attributes: Vec<AttributeMetadata>| {
        let mut class = k(s).with_interfaces(&[&c("AsDecoratorInterface")]);
        class.attributes = attributes;
        class
    };
    let decorated_inner = |nullable: bool| {
        let parameter = if nullable {
            p("inner", &format!("?{}", c("AsDecoratorInterface"))).with_default(Value::Null)
        } else {
            p("inner", &c("AsDecoratorInterface"))
        };
        with(parameter, attribute("AutowireDecorated", PhpArray::new()))
    };
    add(as_decorator("AsDecoratorFoo", vec![]));
    for (name, priority) in [("AsDecoratorBar10", 10_i64), ("AsDecoratorBar20", 20)] {
        add(as_decorator(
            name,
            vec![attribute(
                "AsDecorator",
                php_array!["decorates" => c("AsDecoratorFoo"), "priority" => priority],
            )],
        )
        .with_method(ctor(vec![p("arg1", "string"), decorated_inner(false)])));
    }
    add(as_decorator(
        "AsDecoratorBaz",
        vec![attribute(
            "AsDecorator",
            php_array!["decorates" => "NonExistent", "onInvalid" => 2_i64],
        )],
    )
    .with_method(ctor(vec![decorated_inner(true)])));
    add(as_decorator("AsDecoratorMultipleFoo", vec![]));
    add(as_decorator("AsDecoratorMultipleBar", vec![]));
    add(as_decorator(
        "AsDecoratorMultiple",
        vec![
            attribute(
                "AsDecorator",
                php_array!["decorates" => c("AsDecoratorMultipleFoo")],
            ),
            attribute(
                "AsDecorator",
                php_array!["decorates" => c("AsDecoratorMultipleBar")],
            ),
        ],
    )
    .with_method(ctor(vec![p("arg1", "string"), decorated_inner(false)])));
    let nested = php_array![
        "decorated" => object(attribute("AutowireDecorated", PhpArray::new())),
        "iterator" => object(attribute("AutowireIterator", php_array!["foo"])),
        "locator" => object(attribute("AutowireLocator", php_array!["foo"])),
        "service" => object(attribute("Autowire", php_array!["service" => "bar"])),
    ];
    add(as_decorator(
        "AutowireNestedAttributes",
        vec![attribute(
            "AsDecorator",
            php_array!["decorates" => c("AsDecoratorFoo")],
        )],
    )
    .with_method(ctor(vec![with(
        p("options", "array"),
        attribute("Autowire", php_array![Value::Array(nested)]),
    )])));
    add(
        k("LazyServiceAttributeAutowiring").with_method(ctor(vec![with(
            p("a", &c("A")),
            attribute("Lazy", PhpArray::new()),
        )])),
    );
    add(i("LazyProxyTestInterface"));
    let mut final_impl =
        k("FinalLazyProxyImplementation").with_interfaces(&[&c("LazyProxyTestInterface")]);
    final_impl.is_final = true;
    add(final_impl);
    let lazy_interface = || attribute("Lazy", php_array![c("LazyProxyTestInterface")]);
    add(
        k("LazyServiceAttributeWithInterfaceAutowiring").with_method(ctor(vec![with(
            p("impl", &c("FinalLazyProxyImplementation")),
            lazy_interface(),
        )])),
    );
    add(
        k("LazyServiceAttributeWithInterfaceOnUnionTypeAutowiring").with_method(ctor(vec![with(
            p(
                "impl",
                &format!("{}|{}", c("LazyProxyTestInterface"), c("AInterface")),
            ),
            lazy_interface(),
        )])),
    );
    add(
        k("LazyServiceAttributeWithInterfaceOnIntersectionTypeAutowiring").with_method(ctor(vec![
            with(
                p(
                    "impl",
                    &format!("{}&{}", c("LazyProxyTestInterface"), c("AInterface")),
                ),
                lazy_interface(),
            ),
        ])),
    );
    add(
        k("LazyAutowireServiceAttributesAutowiring").with_method(ctor(vec![with(
            with(p("a", &c("A")), attribute("Lazy", PhpArray::new())),
            attribute("Autowire", php_array!["lazy" => true]),
        )])),
    );
    add(i("AsTagDecoratorInterface"));
    let as_tag = |s: &str| k(s).with_interfaces(&[&c("AsTagDecoratorInterface")]);
    add(as_tag("AsTagDecoratorFoo"));
    add(as_tag("AsTagDecoratorBar"));
    let mut tag_service = as_tag("AsTagDecoratorService").with_method(ctor(vec![with(
        p("inner", &c("AsTagDecoratorInterface")),
        attribute("AutowireDecorated", PhpArray::new()),
    )]));
    tag_service.attributes = vec![attribute("AsTagDecorator", php_array!["test.tag"])];
    add(tag_service);

    // Tests/Fixtures
    add(ClassMetadata::interface(&fx("BarInterface")));
    add(class(&fx("CaseSensitiveClass"))
        .with_method(ctor(vec![p("identifier", "").with_default(Value::Null)])));
    add(class(&fx("DeprecatedClass")));
    add(class(&fx("OptionalParameter")).with_method(ctor(vec![
        p("c", &format!("?{}", c("CollisionInterface"))),
        p("a", &c("A")),
        p("f", &format!("?{}", c("Foo"))).with_default(Value::Null),
    ])));
    add(class(&fx("WithTarget")).with_method(ctor(vec![with(
        p("bar", &fx("BarInterface")),
        attribute("Target", php_array!["image.storage"]),
    )])));
    add(
        class(&fx("WithTargetAnonymous")).with_method(ctor(vec![with(
            p("baz", &fx("BarInterface")),
            attribute("Target", PhpArray::new()),
        )])),
    );
    add(class(&fx("WithoutTarget")).with_method(ctor(vec![p("imageStorage", &fx("BarInterface"))])));
    let closure = |name: &str, service: String, target: bool| {
        let mut parameter = p(name, "Closure");
        if target {
            parameter = with(parameter, attribute("Target", php_array!["image.storage"]));
        }
        with(
            parameter,
            attribute("AutowireServiceClosure", php_array![service]),
        )
    };
    add(
        class(&fx("WithTargetAndServiceClosure")).with_method(ctor(vec![
            closure("closure", fx("BarInterface"), true),
            closure("imageStorage", fx("BarInterface"), false),
            closure("serviceId", "bar".to_string(), true),
        ])),
    );
    add(class(&fx("includes\\FooVariadic"))
        .with_method(ctor(vec![p("foo", &c("Foo"))]))
        .with_method(method(
            "bar",
            vec![ParameterMetadata {
                is_optional: true,
                is_variadic: true,
                ..p("arguments", "")
            }],
        )));
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

fn autowire(container: &mut ContainerBuilder) -> Result<()> {
    AutowirePass::default().process(container)
}

fn resolve_classes(container: &mut ContainerBuilder) {
    ResolveClassPass::new().process(container).unwrap();
}

fn required_methods(container: &mut ContainerBuilder) {
    AutowireRequiredMethodsPass::new()
        .process(container)
        .unwrap();
}

fn typed(id: &str, type_: &str) -> Value {
    Value::from(Reference::typed(
        id,
        type_,
        InvalidBehavior::Exception,
        None,
        PhpArray::new(),
    ))
}

fn typed_class(class: &str) -> Value {
    typed(class, class)
}

fn argument(container: &ContainerBuilder, id: &str, index: i64) -> Value {
    container
        .get_definition(id)
        .unwrap()
        .argument(index)
        .unwrap()
}

fn arguments(container: &ContainerBuilder, id: &str) -> PhpArray {
    container.get_definition(id).unwrap().arguments()
}

/// `(string) $value` of a reference or a string.
fn string(value: &Value) -> String {
    Reference::from_value(value).map_or_else(
        || {
            value
                .to_php_string()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        },
        |r| r.id().to_string(),
    )
}

#[track_caller]
fn assert_equals(expected: Value, actual: Value) {
    assert!(
        loose_equals(&expected, &actual),
        "{expected:?} != {actual:?}"
    );
}

#[track_caller]
fn assert_fails(container: &mut ContainerBuilder, message: &str) {
    let e = autowire(container).unwrap_err();
    assert!(e.is(&ErrorKind::AutowiringFailed), "{e}");
    assert_eq!(message, e.message());
}

/// `expectExceptionMessage()`: the message contains the text.
#[track_caller]
fn assert_fails_containing(container: &mut ContainerBuilder, message: &str) {
    let e = autowire(container).unwrap_err();
    assert!(e.is(&ErrorKind::AutowiringFailed), "{e}");
    assert!(e.message().contains(message), "{}", e.message());
}

#[test]
fn testProcess() {
    let mut container = container();

    container.register(&c("Foo"), None).unwrap();
    container
        .register("bar", Some(&c("Bar")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(1, arguments(&container, "bar").len());
    assert_eq!(c("Foo"), string(&argument(&container, "bar", 0)));
}

#[test]
fn testProcessNotExistingActionParam() {
    let mut container = container();

    container.register(&c("Foo"), None).unwrap();
    container
        .register(&c("ElsaAction"), Some(&c("ElsaAction")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\ElsaAction\": argument \"$notExisting\" of method \"__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotExisting\" but this class was not found.",
    );
}

#[test]
fn testProcessVariadic() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();
    container
        .register("fooVariadic", Some(&fx("includes\\FooVariadic")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(1, arguments(&container, "fooVariadic").len());
    assert_eq!(c("Foo"), string(&argument(&container, "fooVariadic", 0)));
}

#[test]
fn testProcessAutowireParent() {
    let mut container = container();

    container.register(&c("B"), None).unwrap();
    container
        .register("c", Some(&c("C")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"c\": argument \"$a\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\C::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\A\" but no such service exists. You should maybe alias this class to the existing \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\B\" service.",
    );
}

#[test]
fn testProcessAutowireInterface() {
    let mut container = container();

    container.register(&c("F"), None).unwrap();
    container
        .register("g", Some(&c("G")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"g\": argument \"$d\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\G::__construct()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\DInterface\" but no such service exists. You should maybe alias this interface to the existing \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\F\" service.",
    );
}

#[test]
fn testCompleteExistingDefinition() {
    let mut container = container();

    container.register("b", Some(&c("B"))).unwrap();
    container.register(&c("DInterface"), Some(&c("F"))).unwrap();
    container
        .register("h", Some(&c("H")))
        .unwrap()
        .add_argument(Reference::new("b"))
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(2, arguments(&container, "h").len());
    assert_eq!("b", string(&argument(&container, "h", 0)));
    assert_eq!(c("DInterface"), string(&argument(&container, "h", 1)));
}

#[test]
fn testCompleteExistingDefinitionWithNotDefinedArguments() {
    let mut container = container();

    container.register(&c("B"), None).unwrap();
    container.register(&c("DInterface"), Some(&c("F"))).unwrap();
    container
        .register("h", Some(&c("H")))
        .unwrap()
        .add_argument("")
        .add_argument("")
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(2, arguments(&container, "h").len());
    assert_eq!(c("B"), string(&argument(&container, "h", 0)));
    assert_eq!(c("DInterface"), string(&argument(&container, "h", 1)));
}

#[test]
fn testPrivateConstructorThrowsAutowireException() {
    let mut container = container();

    container
        .autowire("private_service", Some(&c("PrivateConstructor")))
        .unwrap();

    let e = AutowirePass::new(true).process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::AutowiringFailed));
    assert_eq!(
        "Invalid service \"private_service\": constructor of class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\PrivateConstructor\" must be public. Did you miss configuring a factory or a static constructor? Try using the \"#[Autoconfigure(constructor: ...)]\" attribute for the latter.",
        e.message()
    );
}

#[test]
fn testTypeCollision() {
    let mut container = container();

    container.register("c1", Some(&c("CollisionA"))).unwrap();
    container.register("c2", Some(&c("CollisionB"))).unwrap();
    container.register("c3", Some(&c("CollisionB"))).unwrap();
    container
        .register("a", Some(&c("CannotBeAutowired")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CannotBeAutowired::__construct()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionInterface\" but no such service exists. You should maybe alias this interface to one of these existing services: \"c1\", \"c2\", \"c3\".",
    );
}

#[test]
fn testTypeNotGuessable() {
    let mut container = container();

    container.register("a1", Some(&c("Foo"))).unwrap();
    container.register("a2", Some(&c("Foo"))).unwrap();
    container
        .register("a", Some(&c("NotGuessableArgument")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$k\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotGuessableArgument::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Foo\" but no such service exists. You should maybe alias this class to one of these existing services: \"a1\", \"a2\".",
    );
}

#[test]
fn testTypeNotGuessableWithSubclass() {
    let mut container = container();

    container.register("a1", Some(&c("B"))).unwrap();
    container.register("a2", Some(&c("B"))).unwrap();
    container
        .register("a", Some(&c("NotGuessableArgumentForSubclass")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$k\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotGuessableArgumentForSubclass::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\A\" but no such service exists. You should maybe alias this class to one of these existing services: \"a1\", \"a2\".",
    );
}

#[test]
fn testTypeNotGuessableNoServicesFound() {
    let mut container = container();

    container
        .register("a", Some(&c("CannotBeAutowired")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CannotBeAutowired::__construct()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionInterface\" but no such service exists. Did you create an instantiable class that implements this interface?",
    );
}

#[test]
fn testTypeNotGuessableUnionType() {
    let mut container = container();

    container.register(&c("CollisionA"), None).unwrap();
    container.register(&c("CollisionB"), None).unwrap();

    container
        .register("a", Some(&c("UnionClasses")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"a\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\UnionClasses::__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionA|Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionB\" but this class was not found.",
    );
}

#[test]
fn testLegacyGuessableUnionType() {
    let mut container = container();

    container.register("b", Some("stcClass")).unwrap();
    container
        .set_alias(&format!("{} $collision", c("CollisionA")), "b")
        .unwrap();
    container
        .set_alias(&format!("{} $collision", c("CollisionB")), "b")
        .unwrap();

    let a_definition = container.register("a", Some(&c("UnionClasses"))).unwrap();
    a_definition.set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!("b", string(&a_definition.argument(0).unwrap()));
}

#[test]
fn testGuessableUnionType() {
    let mut container = container();

    container.register("b", Some("stdClass")).unwrap();
    container
        .set_alias(&format!(".{} $collision", c("CollisionA")), "b")
        .unwrap();
    container
        .set_alias(&format!(".{} $collision", c("CollisionB")), "b")
        .unwrap();

    let a_definition = container
        .register("a", Some(&c("UnionClassesWithTarget")))
        .unwrap();
    a_definition.set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!("b", string(&a_definition.argument(0).unwrap()));
}

#[test]
fn testTypeNotGuessableIntersectionType() {
    let mut container = container();

    container.register(&c("CollisionInterface"), None).unwrap();
    container.register(&c("AnotherInterface"), None).unwrap();

    container
        .register("a", Some(&c("IntersectionClasses")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"a\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\IntersectionClasses::__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\AnotherInterface&Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionInterface\" but this class was not found.",
    );
}

#[test]
fn testTypeNotGuessableCompositeType() {
    let mut container = container();

    container.register(&c("CollisionInterface"), None).unwrap();
    container.register(&c("AnotherInterface"), None).unwrap();

    container
        .register("a", Some(&c("CompositeTypeClasses")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"a\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CompositeTypeClasses::__construct()\" has type \"(Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\AnotherInterface&Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionInterface)|Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\YetAnotherInterface\" but this class was not found.",
    );
}

#[test]
fn testGuessableIntersectionType() {
    let mut container = container();

    container.register("b", Some("stcClass")).unwrap();
    container.set_alias(&c("CollisionInterface"), "b").unwrap();
    container.set_alias(&c("AnotherInterface"), "b").unwrap();
    container.set_alias(&c("DummyInterface"), "b").unwrap();

    let a_definition = container
        .register("a", Some(&c("IntersectionClasses")))
        .unwrap();
    a_definition.set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!("b", string(&a_definition.argument(0).unwrap()));
}

#[test]
fn testGuessableIntersectionTypeWithTarget() {
    let mut container = container();

    container.register("b", Some("stdClass")).unwrap();
    container
        .register_alias_for_argument(
            "b",
            &c("CollisionInterface"),
            Some("foo.collision"),
            Some("foo"),
        )
        .unwrap();
    container
        .register_alias_for_argument(
            "b",
            &c("AnotherInterface"),
            Some("foo.another"),
            Some("foo"),
        )
        .unwrap();

    let a_definition = container
        .register("a", Some(&c("IntersectionClassesWithTarget")))
        .unwrap();
    a_definition.set_autowired(true);

    autowire(&mut container).unwrap();

    let found = container
        .find_definition(&string(&a_definition.argument(0).unwrap()))
        .unwrap();
    assert!(container.get_definition("b").unwrap().ptr_eq(&found));
}

#[test]
fn testTypeNotGuessableWithTypeSet() {
    let mut container = container();

    container.register("a1", Some(&c("Foo"))).unwrap();
    container.register("a2", Some(&c("Foo"))).unwrap();
    container.register(&c("Foo"), Some(&c("Foo"))).unwrap();
    container
        .register("a", Some(&c("NotGuessableArgument")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(1, arguments(&container, "a").len());
    assert_eq!(c("Foo"), string(&argument(&container, "a", 0)));
}

#[test]
fn testWithTypeSet() {
    let mut container = container();

    container.register("c1", Some(&c("CollisionA"))).unwrap();
    container.register("c2", Some(&c("CollisionB"))).unwrap();
    container.set_alias(&c("CollisionInterface"), "c2").unwrap();
    container
        .register("a", Some(&c("CannotBeAutowired")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(1, arguments(&container, "a").len());
    assert_eq!(
        c("CollisionInterface"),
        string(&argument(&container, "a", 0))
    );
}

#[test]
fn testServicesAreNotAutoCreated() {
    let mut container = container();

    container
        .register("coop_tilleuls", Some(&c("LesTilleuls")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"coop_tilleuls\": argument \"$j\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\LesTilleuls::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Dunglas\" but no such service exists.",
    );
}

#[test]
fn testResolveParameter() {
    let mut container = container();

    container.set_parameter("class_name", c("Bar")).unwrap();
    container.register(&c("Foo"), None).unwrap();
    container
        .register("bar", Some("%class_name%"))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(c("Foo"), string(&argument(&container, "bar", 0)));
}

#[test]
fn testOptionalParameter() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Foo"), None).unwrap();
    container
        .register("opt", Some(&fx("OptionalParameter")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert!(argument(&container, "opt", 0).is_null());
    assert_eq!(c("A"), string(&argument(&container, "opt", 1)));
    assert_eq!(c("Foo"), string(&argument(&container, "opt", 2)));
}

#[test]
fn testParameterWithNullUnionIsSkipped() {
    let mut container = container();

    container
        .register("opt", Some(&c("UnionNull")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert!(argument(&container, "opt", 0).is_null());
}

#[test]
fn testParameterWithNullableIntersectionIsSkippedWhenAliasDoesNotExist() {
    let mut container = container();

    container
        .register("opt", Some(&c("NullableIntersection")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert!(argument(&container, "opt", 0).is_null());
}

#[test]
fn testParameterWithNullableIntersectionIsAutowiredWhenAliasExists() {
    let mut container = container();

    container.register("b", Some("stdClass")).unwrap();
    container.set_alias(&c("CollisionInterface"), "b").unwrap();
    container.set_alias(&c("AnotherInterface"), "b").unwrap();

    container
        .register("opt", Some(&c("NullableIntersection")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!("b", string(&argument(&container, "opt", 0)));
}

#[test]
fn testParameterWithNullUnionIsAutowired() {
    let mut container = container();

    container
        .register(&c("CollisionInterface"), Some(&c("CollisionA")))
        .unwrap();

    container
        .register("opt", Some(&c("UnionNull")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(
        c("CollisionInterface"),
        string(&argument(&container, "opt", 0))
    );
}

#[test]
fn testDontTriggerAutowiring() {
    let mut container = container();

    container.register(&c("Foo"), None).unwrap();
    container.register("bar", Some(&c("Bar"))).unwrap();

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_eq!(0, arguments(&container, "bar").len());
}

#[test]
fn testClassNotFoundThrowsException() {
    let mut container = container();

    container
        .register("a", Some(&c("BadTypeHintedArgument")))
        .unwrap()
        .set_autowired(true);

    container
        .register(&c("Dunglas"), Some(&c("Dunglas")))
        .unwrap();

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$r\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\BadTypeHintedArgument::__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotARealClass\" but this class was not found.",
    );
}

#[test]
fn testParentClassNotFoundThrowsException() {
    let mut container = container();

    container
        .register("a", Some(&c("BadParentTypeHintedArgument")))
        .unwrap()
        .set_autowired(true);

    container
        .register(&c("Dunglas"), Some(&c("Dunglas")))
        .unwrap();

    let e = autowire(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::AutowiringFailed));
    // assertMatchesRegularExpression('{^Cannot autowire service "a": argument "\$r" of method "(Symfony\\\\Component\\\\DependencyInjection\\\\Tests\\\\Compiler\\\\)BadParentTypeHintedArgument::__construct\(\)" has type "\1OptionalServiceClass" but this class is missing a parent class \(Class "?Symfony\\\\Bug\\\\NotExistClass"? not found}')
    let message = e.message();
    let expected = "Cannot autowire service \"a\": argument \"$r\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\BadParentTypeHintedArgument::__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\OptionalServiceClass\" but this class is missing a parent class (Class ";
    let rest = message
        .strip_prefix(expected)
        .unwrap_or_else(|| panic!("{message}"));
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let rest = rest
        .strip_prefix("Symfony\\Bug\\NotExistClass")
        .unwrap_or_else(|| panic!("{message}"));
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    assert!(rest.starts_with(" not found"), "{message}");
}

#[test]
fn testDontUseAbstractServices() {
    let mut container = container();

    container
        .register(&c("Foo"), None)
        .unwrap()
        .set_abstract(true);
    container.register("foo", Some(&c("Foo"))).unwrap();
    container
        .register("bar", Some(&c("Bar")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"bar\": argument \"$foo\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Bar::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Foo\" but this service is abstract. You should maybe alias this class to the existing \"foo\" service.",
    );
}

#[test]
fn testSomeSpecificArgumentsAreSet() {
    let mut container = container();

    container.register("foo", Some(&c("Foo"))).unwrap();
    container.register(&c("A"), None).unwrap();
    container.register(&c("Dunglas"), None).unwrap();
    let mut arguments = PhpArray::new();
    arguments.insert(1, Value::from(Reference::new("foo")));
    arguments.insert(3, Value::Array(php_array!["bar"]));
    container
        .register("multiple", Some(&c("MultipleArguments")))
        .unwrap()
        .set_autowired(true)
        // set the 2nd (index 1) argument only: autowire the first and third
        // args are: A, Foo, Dunglas
        .set_arguments(arguments);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([
            typed_class(&c("A")),
            Value::from(Reference::new("foo")),
            typed_class(&c("Dunglas")),
            Value::Array(php_array!["bar"]),
        ])),
        Value::Array(self::arguments(&container, "multiple")),
    );
}

#[test]
fn testScalarArgsCannotBeAutowired() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Dunglas"), None).unwrap();
    let mut arguments = PhpArray::new();
    arguments.insert(1, Value::from("foo"));
    container
        .register("arg_no_type_hint", Some(&c("MultipleArguments")))
        .unwrap()
        .set_arguments(arguments)
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"arg_no_type_hint\": argument \"$bar\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\MultipleArguments::__construct()\" is type-hinted \"array\", you should configure its value explicitly.",
    );
}

#[test]
fn testUnionScalarArgsCannotBeAutowired() {
    let mut container = container();

    container
        .register("union_scalars", Some(&c("UnionScalars")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"union_scalars\": argument \"$timeout\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\UnionScalars::__construct()\" is type-hinted \"float|int\", you should configure its value explicitly.",
    );
}

#[test]
fn testNoTypeArgsCannotBeAutowired() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Dunglas"), None).unwrap();
    container
        .register("arg_no_type_hint", Some(&c("MultipleArguments")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"arg_no_type_hint\": argument \"$foo\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\MultipleArguments::__construct()\" has no type-hint, you should configure its value explicitly.",
    );
}

#[test]
fn testOptionalScalarArgsDontMessUpOrder() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Lille"), None).unwrap();
    container
        .register(
            "with_optional_scalar",
            Some(&c("MultipleArgumentsOptionalScalar")),
        )
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([
            typed_class(&c("A")),
            // use the default value
            Value::from("default_val"),
            typed_class(&c("Lille")),
        ])),
        Value::Array(arguments(&container, "with_optional_scalar")),
    );
}

#[test]
fn testOptionalScalarArgsNotPassedIfLast() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Lille"), None).unwrap();
    container
        .register(
            "with_optional_scalar_last",
            Some(&c("MultipleArgumentsOptionalScalarLast")),
        )
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([
            typed_class(&c("A")),
            typed_class(&c("Lille")),
        ])),
        Value::Array(arguments(&container, "with_optional_scalar_last")),
    );
}

#[test]
fn testOptionalArgsNoRequiredForCoreClasses() {
    let mut container = container();

    container
        .register("foo", Some("SplFileObject"))
        .unwrap()
        .add_argument("foo.txt")
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(php_array!["foo.txt"]),
        Value::Array(arguments(&container, "foo")),
    );
}

#[test]
fn testSetterInjectionWithAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register("setter_injection", Some(&c("AutowireSetter")))
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    required_methods(&mut container);
    autowire(&mut container).unwrap();

    let method_calls = container
        .get_definition("setter_injection")
        .unwrap()
        .method_calls();
    assert_eq!(1, method_calls.len());
    let call = method_calls
        .get(&Key::Int(0))
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    assert_eq!("setFoo", string(call.get(&Key::Int(0)).unwrap()));
    let arguments = call.get(&Key::Int(1)).unwrap().as_array().unwrap().clone();
    assert_eq!(c("Foo"), string(arguments.get(&Key::Int(0)).unwrap()));
}

#[test]
fn testWithNonExistingSetterAndAutowiring() {
    let mut container = container();

    container
        .register(&fx("CaseSensitiveClass"), Some(&fx("CaseSensitiveClass")))
        .unwrap()
        .set_autowired(true)
        .add_method_call("setLogger", PhpArray::new(), false)
        .unwrap();

    resolve_classes(&mut container);
    required_methods(&mut container);
    let e = autowire(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert!(
        e.message().contains(
            "Invalid service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\CaseSensitiveClass\": method \"setLogger()\" does not exist."
        ),
        "{}",
        e.message()
    );
}

#[test]
fn testExplicitMethodInjectionAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();
    container.register(&c("A"), None).unwrap();
    container.register(&c("CollisionA"), None).unwrap();
    container.register(&c("CollisionB"), None).unwrap();

    container
        .register("setter_injection", Some(&c("SetterInjection")))
        .unwrap()
        .set_autowired(true)
        .add_method_call("notASetter", PhpArray::new(), false)
        .unwrap();

    resolve_classes(&mut container);
    required_methods(&mut container);
    autowire(&mut container).unwrap();

    let method_calls = container
        .get_definition("setter_injection")
        .unwrap()
        .method_calls();

    let names: Vec<String> = method_calls
        .values()
        .map(|call| string(call.as_array().unwrap().get(&Key::Int(0)).unwrap()))
        .collect();
    assert_eq!(
        vec![
            "notASetter",
            "setFoo",
            "setDependencies",
            "setWithCallsConfigured",
            "setChildMethodWithoutDocBlock"
        ],
        names
    );
    assert_equals(
        Value::Array(PhpArray::list([typed_class(&c("A"))])),
        method_calls
            .get(&Key::Int(0))
            .unwrap()
            .as_array()
            .unwrap()
            .get(&Key::Int(1))
            .unwrap()
            .clone(),
    );
}

#[test]
fn testIgnoreServiceWithClassNotExisting() {
    let mut container = container();

    container
        .register("class_not_exist", Some(&c("OptionalServiceClass")))
        .unwrap();

    container
        .register("bar", Some(&c("Bar")))
        .unwrap()
        .set_autowired(true);

    container.register(&c("Foo"), Some(&c("Foo"))).unwrap();

    autowire(&mut container).unwrap();

    assert!(container.has_definition("bar"));
}

#[test]
fn testSetterInjectionFromAttributeCollisionThrowsException() {
    let mut container = container();

    container.register("c1", Some(&c("CollisionA"))).unwrap();
    container.register("c2", Some(&c("CollisionB"))).unwrap();
    container
        .register(
            "setter_injection_collision",
            Some(&c("SetterInjectionCollision")),
        )
        .unwrap()
        .set_autowired(true);

    required_methods(&mut container);

    assert_fails(
        &mut container,
        "Cannot autowire service \"setter_injection_collision\": argument \"$collision\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\SetterInjectionCollision::setMultipleInstancesForOneArg()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CollisionInterface\" but no such service exists. You should maybe alias this interface to one of these existing services: \"c1\", \"c2\".",
    );
}

#[test]
fn testInterfaceWithNoImplementationSuggestToWriteOne() {
    let mut container = container();

    container
        .register("my_service", Some(&c("K")))
        .unwrap()
        .set_autowired(true);

    required_methods(&mut container);

    assert_fails(
        &mut container,
        "Cannot autowire service \"my_service\": argument \"$i\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\K::__construct()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\IInterface\" but no such service exists. Did you create an instantiable class that implements this interface?",
    );
}

#[test]
fn testProcessDoesNotTriggerDeprecations() {
    let mut container = container();
    container
        .register(
            "deprecated",
            Some("Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\DeprecatedClass"),
        )
        .unwrap()
        .set_deprecated("vendor/package", "1.1", "%service_id%")
        .unwrap();
    container.register("foo", Some(&c("Foo"))).unwrap();
    container
        .register("bar", Some(&c("Bar")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"bar\": argument \"$foo\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Bar::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Foo\" but no such service exists. You should maybe alias this class to the existing \"foo\" service.",
    );

    assert!(container.has_definition("deprecated"));
    assert!(container.has_definition("foo"));
    assert!(container.has_definition("bar"));
}

#[test]
fn testEmptyStringIsKept() {
    let mut container = container();

    container.register(&c("A"), None).unwrap();
    container.register(&c("Lille"), None).unwrap();
    container
        .register("foo", Some(&c("MultipleArgumentsOptionalScalar")))
        .unwrap()
        .set_autowired(true)
        .set_arguments(php_array!["", ""]);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([
            typed_class(&c("A")),
            Value::from(""),
            typed_class(&c("Lille")),
        ])),
        Value::Array(arguments(&container, "foo")),
    );
}

#[test]
fn testWithFactory() {
    let mut container = container();

    container.register(&c("Foo"), None).unwrap();
    let definition = container.register("a", Some(&c("A"))).unwrap();
    definition
        .set_factory(Value::Array(php_array![c("A"), "create"]))
        .set_autowired(true);

    resolve_classes(&mut container);
    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([typed_class(&c("Foo"))])),
        Value::Array(definition.arguments()),
    );
}

#[test]
fn testNotWireableCalls() {
    // provideNotWireableCalls
    let cases = [
        (
            Some("setNotAutowireable"),
            "Cannot autowire service \"foo\": argument \"$n\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotWireable::setNotAutowireable()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotARealClass\" but this class was not found.",
        ),
        (
            Some("setDifferentNamespace"),
            "Cannot autowire service \"foo\": argument \"$n\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotWireable::setDifferentNamespace()\" references class \"stdClass\" but no such service exists.",
        ),
        (
            None,
            "Invalid service \"foo\": method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotWireable::setProtectedMethod()\" must be public.",
        ),
    ];
    for (method, expected_msg) in cases {
        let mut container = container();

        let foo = container.register("foo", Some(&c("NotWireable"))).unwrap();
        foo.set_autowired(true);
        for call in [
            "setBar",
            "setOptionalNotAutowireable",
            "setOptionalNoTypeHint",
            "setOptionalArgNoAutowireable",
        ] {
            foo.add_method_call(call, PhpArray::new(), false).unwrap();
        }

        if let Some(method) = method {
            foo.add_method_call(method, PhpArray::new(), false).unwrap();
        }

        resolve_classes(&mut container);
        required_methods(&mut container);
        let e = autowire(&mut container).unwrap_err();
        assert!(e.is(&ErrorKind::Runtime), "{e}");
        assert_eq!(expected_msg, e.message());
    }
}

#[test]
fn testSuggestRegisteredServicesWithSimilarCase() {
    let mut container = container();

    container
        .register(&c("LesTilleuls"), Some(&c("LesTilleuls")))
        .unwrap();
    container
        .register("foo", Some(&c("NotWireable")))
        .unwrap()
        .set_autowired(true)
        .add_method_call("setNotAutowireableBecauseOfATypo", PhpArray::new(), false)
        .unwrap();

    resolve_classes(&mut container);
    required_methods(&mut container);
    assert_fails(
        &mut container,
        "Cannot autowire service \"foo\": argument \"$sam\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotWireable::setNotAutowireableBecauseOfATypo()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\lesTilleuls\" but no such service exists. Did you mean \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\LesTilleuls\"?",
    );
}

#[test]
fn testByIdAlternative() {
    let mut container = container();

    container.set_alias(&c("IInterface"), "i").unwrap();
    container.register("i", Some(&c("I"))).unwrap();
    container
        .register("j", Some(&c("J")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"j\": argument \"$i\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\J::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\I\" but no such service exists. Try changing the type-hint to \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\IInterface\" instead.",
    );
}

#[test]
fn testExceptionWhenAliasExists() {
    let mut container = container();

    // multiple I services... but there *is* IInterface available
    container.set_alias(&c("IInterface"), "i").unwrap();
    container.register("i", Some(&c("I"))).unwrap();
    container.register("i2", Some(&c("I"))).unwrap();
    // J type-hints against I concretely
    container
        .register("j", Some(&c("J")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"j\": argument \"$i\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\J::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\I\" but no such service exists. Try changing the type-hint to \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\IInterface\" instead.",
    );
}

#[test]
fn testExceptionWhenAliasDoesNotExist() {
    let mut container = container();

    // multiple I instances... but no IInterface alias
    container.register("i", Some(&c("I"))).unwrap();
    container.register("i2", Some(&c("I"))).unwrap();
    // J type-hints against I concretely
    container
        .register("j", Some(&c("J")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"j\": argument \"$i\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\J::__construct()\" references class \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\I\" but no such service exists. You should maybe alias this class to one of these existing services: \"i\", \"i2\".",
    );
}

fn decorated_container() -> ContainerBuilder {
    let mut container = container();
    container
        .register(LOGGER_INTERFACE, Some(NULL_LOGGER))
        .unwrap();
    container
        .register(&c("Decorated"), Some(&c("Decorated")))
        .unwrap();
    container
}

fn decorate(container: &mut ContainerBuilder, id: &str, renamed: Option<&str>) {
    container
        .register(id, Some(id))
        .unwrap()
        .set_decorated_service(
            Some(&c("Decorated")),
            renamed,
            0,
            InvalidBehavior::Exception,
        )
        .unwrap()
        .set_autowired(true);
}

#[test]
fn testAutowireDecorator() {
    let mut container = decorated_container();
    decorate(&mut container, &c("Decorator"), None);

    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    assert_eq!(
        format!("{}.inner", c("Decorator")),
        string(&argument(&container, &c("Decorator"), 1))
    );
}

#[test]
fn testAutowireDecoratorChain() {
    let mut container = decorated_container();
    decorate(&mut container, &c("Decorator"), None);
    decorate(&mut container, &c("DecoratedDecorator"), None);

    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    assert_eq!(
        format!("{}.inner", c("DecoratedDecorator")),
        string(&argument(&container, &c("DecoratedDecorator"), 0))
    );
}

#[test]
fn testAutowireDecoratorRenamedId() {
    let mut container = decorated_container();
    decorate(&mut container, &c("Decorator"), Some("renamed"));

    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    assert_eq!("renamed", string(&argument(&container, &c("Decorator"), 1)));
}

#[test]
fn testDoNotAutowireDecoratorWhenSeveralArgumentOfTheType() {
    let mut container = decorated_container();
    decorate(&mut container, &c("NonAutowirableDecorator"), None);

    DecoratorServicePass::new().process(&mut container).unwrap();
    assert_fails(
        &mut container,
        "Cannot autowire service \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NonAutowirableDecorator\": argument \"$decorated1\" of method \"__construct()\" references interface \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\DecoratorInterface\" but no such service exists. You should maybe alias this interface to one of these existing services: \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NonAutowirableDecorator\", \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NonAutowirableDecorator.inner\".",
    );
}

#[test]
fn testErroredServiceLocator() {
    let mut container = container();
    container
        .register("some_locator", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::typed(
            c("MissingClass"),
            c("MissingClass"),
            InvalidBehavior::RuntimeException,
            None,
            PhpArray::new(),
        ))
        .add_tag("container.service_locator", PhpArray::new());

    autowire(&mut container).unwrap();

    assert_eq!(
        vec![
            "Cannot autowire service \"some_locator\": it has type \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\MissingClass\" but this class was not found."
                .to_string()
        ],
        container
            .get_definition(&format!(".errored.some_locator.{}", c("MissingClass")))
            .unwrap()
            .errors()
    );
}

fn named_argument_collisions(class_short: &str) {
    let mut container = container();

    container.register("c1", Some(&c("CollisionA"))).unwrap();
    container.register("c2", Some(&c("CollisionB"))).unwrap();
    container
        .set_alias(&format!("{} $collision", c("CollisionInterface")), "c2")
        .unwrap();
    container
        .register("setter_injection_collision", Some(&c(class_short)))
        .unwrap()
        .set_autowired(true);

    required_methods(&mut container);

    autowire(&mut container).unwrap();

    let expected = PhpArray::list([Value::Array(PhpArray::list([
        Value::from("setMultipleInstancesForOneArg"),
        Value::Array(PhpArray::list([typed(
            &format!("{} $collision", c("CollisionInterface")),
            &c("CollisionInterface"),
        )])),
    ]))]);
    assert_equals(
        Value::Array(expected),
        Value::Array(
            container
                .get_definition("setter_injection_collision")
                .unwrap()
                .method_calls(),
        ),
    );
}

#[test]
fn testLegacyNamedArgumentAliasResolveCollisions() {
    named_argument_collisions("SetterInjectionCollision");
}

#[test]
fn testNamedArgumentAliasResolveCollisions() {
    named_argument_collisions("SetterInjectionCollisionWithTarget");
}

#[test]
fn testArgumentWithTarget() {
    let mut container = container();

    container
        .register(&fx("BarInterface"), Some(&fx("BarInterface")))
        .unwrap();
    container
        .register(
            &format!(".{} $image.storage", fx("BarInterface")),
            Some(&fx("BarInterface")),
        )
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTarget")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(
        format!(".{} $image.storage", fx("BarInterface")),
        string(&argument(&container, "with_target", 0))
    );
}

#[test]
fn testArgumentWithParsedTarget() {
    let mut container = container();

    container
        .register(&fx("BarInterface"), Some(&fx("BarInterface")))
        .unwrap();
    container
        .register(
            &format!("{} $imageStorage", fx("BarInterface")),
            Some(&fx("BarInterface")),
        )
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTarget")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(
        format!("{} $imageStorage", fx("BarInterface")),
        string(&argument(&container, "with_target", 0))
    );
}

#[test]
fn testArgumentWithTypoTarget() {
    let mut container = container();

    container
        .register(&fx("BarInterface"), Some(&fx("BarInterface")))
        .unwrap();
    container
        .register_alias_for_argument("images.storage", &fx("BarInterface"), None, None)
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTarget")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"with_target\": argument \"$bar\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\WithTarget::__construct()\" has \"#[Target('image.storage')]\" but no such target exists. Did you mean to target \"images.storage\" instead?",
    );
}

#[test]
fn testArgumentWithTypoTargetAnonymous() {
    let mut container = container();

    container
        .register(&fx("BarInterface"), Some(&fx("BarInterface")))
        .unwrap();
    container
        .register_alias_for_argument("bar", &fx("BarInterface"), None, None)
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTargetAnonymous")))
        .unwrap()
        .set_autowired(true);

    assert_fails_containing(
        &mut container,
        "Cannot autowire service \"with_target\": argument \"$baz\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\WithTargetAnonymous::__construct()\" has \"#[Target('baz')]\" but no such target exists. Did you mean to target \"bar\" instead?",
    );
}

#[test]
fn testNamedAliasByParameterNameIsDeprecated() {
    let mut container = container();

    container
        .register("image.storage", Some(&fx("BarInterface")))
        .unwrap();
    container
        .register_alias_for_argument(
            "image.storage",
            &fx("BarInterface"),
            Some("imageStorage"),
            None,
        )
        .unwrap();
    container
        .register("without_target", Some(&fx("WithoutTarget")))
        .unwrap()
        .set_autowired(true);

    let (result, deprecations) =
        symfony_deprecation_contracts::collect(|| autowire(&mut container));
    result.unwrap();

    let expected = "Since symfony/dependency-injection 8.1: Relying solely on the name of parameter \"$imageStorage\" of \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\WithoutTarget::__construct()\" to match a named autowiring alias is deprecated; use the \"#[Target]\" attribute.";
    assert!(
        deprecations
            .iter()
            .any(|d| d.message == expected.as_bytes()),
        "{deprecations:?}"
    );
}

#[test]
fn testArgumentWithIdTarget() {
    let mut container = container();

    container
        .register("image.storage", Some(&fx("BarInterface")))
        .unwrap();
    container
        .register_alias_for_argument("image.storage", &fx("BarInterface"), Some("image"), None)
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTarget")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(
        "image.storage",
        string(&argument(&container, "with_target", 0))
    );
}

#[test]
fn testAutowireWithNamedArgs() {
    let mut container = container();

    container
        .register("foo", Some(&c("MultipleArgumentsOptionalScalar")))
        .unwrap()
        .set_arguments(php_array!["foo" => "abc"])
        .set_autowired(true)
        .set_public(true);
    container.register(&c("A"), Some(&c("A"))).unwrap();

    autowire(&mut container).unwrap();

    assert_equals(
        Value::Array(PhpArray::list([typed_class(&c("A")), Value::from("abc")])),
        Value::Array(arguments(&container, "foo")),
    );
}

#[test]
fn testAutowireDefaultValueParametersLike() {
    let mut container = container();

    container
        .register("foo", Some(&c("ParametersLikeDefaultValue")))
        .unwrap()
        .set_autowired(true)
        .set_argument(1, "ok");

    autowire(&mut container).unwrap();

    assert!(
        Value::from("%%not%%one%%parameter%%here%%").identical(&argument(&container, "foo", 0))
    );
}

#[test]
fn testAutowireAttributeNullFallbackTestRequired() {
    let mut container = container();

    container
        .register("foo", Some(&c("AutowireAttributeNullFallback")))
        .unwrap()
        .set_autowired(true)
        .set_public(true);

    assert_fails_containing(
        &mut container,
        "You have requested a non-existent parameter \"required.parameter\".",
    );
}

#[test]
fn testAutowireAttributeNullFallbackTestOptional() {
    let mut container = container();

    container
        .register("foo", Some(&c("AutowireAttributeNullFallback")))
        .unwrap()
        .set_autowired(true)
        .set_public(true);

    container
        .set_parameter("required.parameter", "foo")
        .unwrap();

    autowire(&mut container).unwrap();

    assert!(Value::Array(php_array!["foo"]).identical(&Value::Array(arguments(&container, "foo"))));
}

#[test]
fn testTargetWithAutowireServiceClosure() {
    let mut container = container();

    container
        .register(&fx("BarInterface"), Some(&fx("BarInterface")))
        .unwrap();
    container
        .register("image.storage", Some(&fx("BarInterface")))
        .unwrap();
    container
        .register_alias_for_argument("image.storage", &fx("BarInterface"), None, None)
        .unwrap();
    container
        .register("bar", Some(&fx("BarInterface")))
        .unwrap();
    container
        .register("with_target", Some(&fx("WithTargetAndServiceClosure")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let closure = |i: i64| {
        ServiceClosureArgument::from_value(&argument(&container, "with_target", i))
            .unwrap()
            .value()
    };
    let found = container.find_definition(&string(&closure(0))).unwrap();
    assert!(
        container
            .get_definition("image.storage")
            .unwrap()
            .ptr_eq(&found)
    );
    assert_equals(Value::from(Reference::new(fx("BarInterface"))), closure(1));
    assert_equals(Value::from(Reference::new("bar")), closure(2));
}

#[test]
fn testAsDecoratorAttribute() {
    let mut container = container();

    container.register(&c("AsDecoratorFoo"), None).unwrap();
    container
        .register(&c("AsDecoratorBar10"), None)
        .unwrap()
        .set_autowired(true)
        .set_argument(0, "arg1");
    container
        .register(&c("AsDecoratorBar20"), None)
        .unwrap()
        .set_autowired(true)
        .set_argument(0, "arg1");
    container
        .register(&c("AsDecoratorBaz"), None)
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    AutowireAsDecoratorPass.process(&mut container).unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    assert_eq!(
        format!("{}.inner", c("AsDecoratorBar10")),
        string(&argument(&container, &c("AsDecoratorBar10"), 1))
    );

    assert_eq!(
        format!("{}.inner", c("AsDecoratorBar20")),
        string(&argument(&container, &c("AsDecoratorBar20"), 1))
    );
    let baz = argument(&container, &c("AsDecoratorBaz"), 0);
    assert_eq!(format!("{}.inner", c("AsDecoratorBaz")), string(&baz));
    assert_eq!(
        2,
        Reference::from_value(&baz)
            .unwrap()
            .invalid_behavior()
            .as_int()
    );
}

#[test]
fn testMultipleAsDecoratorAttribute() {
    let mut container = container();

    container
        .register(&c("AsDecoratorMultipleFoo"), None)
        .unwrap();
    container
        .register(&c("AsDecoratorMultipleBar"), None)
        .unwrap();
    container
        .register(&c("AsDecoratorMultiple"), None)
        .unwrap()
        .set_autowired(true)
        .set_argument(0, "arg1");

    resolve_classes(&mut container);
    AutowireAsDecoratorPass.process(&mut container).unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    let foo_decorator_name = format!(
        ".decorator.{}.{}",
        c("AsDecoratorMultipleFoo"),
        c("AsDecoratorMultiple")
    );
    assert_eq!(
        foo_decorator_name,
        container
            .get_alias(&c("AsDecoratorMultipleFoo"))
            .unwrap()
            .id()
    );
    assert_eq!(
        format!("{foo_decorator_name}.inner"),
        string(&argument(&container, &foo_decorator_name, 1))
    );

    let bar_decorator_name = format!(
        ".decorator.{}.{}",
        c("AsDecoratorMultipleBar"),
        c("AsDecoratorMultiple")
    );
    assert_eq!(
        bar_decorator_name,
        container
            .get_alias(&c("AsDecoratorMultipleBar"))
            .unwrap()
            .id()
    );
    assert_eq!(
        format!("{bar_decorator_name}.inner"),
        string(&argument(&container, &bar_decorator_name, 1))
    );
}

#[test]
fn testAsTagDecoratorAttribute() {
    let mut container = container();

    container
        .register(&c("AsTagDecoratorFoo"), None)
        .unwrap()
        .add_tag("test.tag", PhpArray::new());
    container
        .register(&c("AsTagDecoratorBar"), None)
        .unwrap()
        .add_tag("test.tag", PhpArray::new());
    container
        .register(
            &c("AsTagDecoratorService"),
            Some(&c("AsTagDecoratorService")),
        )
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();
    AutowireAsDecoratorPass.process(&mut container).unwrap();
    TagDecoratorPass.process(&mut container).unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();

    let tag_decorator_template = format!(".tag_decorator.test.tag.{}", c("AsTagDecoratorService"));
    assert!(!container.has(&tag_decorator_template));

    let foo_decorator = format!(
        ".decorator.{}.{tag_decorator_template}",
        c("AsTagDecoratorFoo")
    );
    let bar_decorator = format!(
        ".decorator.{}.{tag_decorator_template}",
        c("AsTagDecoratorBar")
    );

    assert!(container.has(&foo_decorator));
    assert!(container.has(&bar_decorator));

    assert_eq!(
        foo_decorator,
        container.get_alias(&c("AsTagDecoratorFoo")).unwrap().id()
    );
    assert_eq!(
        bar_decorator,
        container.get_alias(&c("AsTagDecoratorBar")).unwrap().id()
    );
}

#[test]
fn testTypeSymbolExcluded() {
    let mut container = container();

    container
        .register(&c("Foo"), None)
        .unwrap()
        .add_tag("container.excluded", php_array!["source" => "for tests"]);
    container
        .register("a", Some(&c("NotGuessableArgument")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$k\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotGuessableArgument::__construct()\" needs an instance of \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Foo\" but this type has been excluded for tests.",
    );
}

#[test]
fn testTypeNamespaceExcluded() {
    let mut container = container();

    container
        .register(NS.trim_end_matches('\\'), None)
        .unwrap()
        .add_tag("container.excluded", PhpArray::new());
    container
        .register("a", Some(&c("NotGuessableArgument")))
        .unwrap()
        .set_autowired(true);

    assert_fails(
        &mut container,
        "Cannot autowire service \"a\": argument \"$k\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotGuessableArgument::__construct()\" needs an instance of \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\Foo\" but this type has been excluded from autowiring.",
    );
}

#[test]
fn testNestedAttributes() {
    let mut container = container();

    container.register(&c("AsDecoratorFoo"), None).unwrap();
    container
        .register(&c("AutowireNestedAttributes"), None)
        .unwrap()
        .set_autowired(true);

    resolve_classes(&mut container);
    AutowireAsDecoratorPass.process(&mut container).unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();
    autowire(&mut container).unwrap();

    let expected = php_array![
        "decorated" => Reference::new(format!("{}.inner", c("AutowireNestedAttributes"))),
        "iterator" => IteratorArgument::tagged("foo", None, false, vec![], true).to_value(),
        "locator" => ServiceLocatorArgument::from_tagged(IteratorArgument::tagged(
            "foo",
            None,
            true,
            vec![],
            true,
        ))
        .to_value(),
        "service" => Reference::new("bar"),
    ];
    assert_equals(
        Value::Array(expected),
        argument(&container, &c("AutowireNestedAttributes"), 0),
    );
}

#[test]
fn testLazyServiceAttribute() {
    let mut container = container();
    container
        .register("a", Some(&c("A")))
        .unwrap()
        .set_autowired(true);
    container
        .register("foo", Some(&c("LazyServiceAttributeAutowiring")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let expected = Reference::new(format!(".lazy.{}", c("A")));
    assert_equals(Value::from(expected), argument(&container, "foo", 0));
}

#[test]
fn testLazyServiceAttributeOnAlreadyLazyService() {
    let mut container = container();
    container
        .register(&c("A"), Some(&c("A")))
        .unwrap()
        .set_autowired(true)
        .set_lazy(true);
    container
        .register("foo", Some(&c("LazyServiceAttributeAutowiring")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    assert_eq!(c("A"), string(&argument(&container, "foo", 0)));
    assert!(!container.has_definition(&format!(".lazy.{}", c("A"))));
}

fn proxy_tag(interfaces: &[&str]) -> Value {
    Value::Array(PhpArray::list(
        interfaces
            .iter()
            .map(|i| Value::Array(php_array!["interface" => *i])),
    ))
}

#[test]
fn testLazyServiceAttributeWithInterface() {
    let mut container = container();
    container
        .register(
            &c("FinalLazyProxyImplementation"),
            Some(&c("FinalLazyProxyImplementation")),
        )
        .unwrap()
        .set_autowired(true);
    container
        .register(
            "foo",
            Some(&c("LazyServiceAttributeWithInterfaceAutowiring")),
        )
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let definition = container
        .get_definition(&string(&argument(&container, "foo", 0)))
        .unwrap();

    assert!(definition.is_lazy());
    assert!(
        proxy_tag(&[&c("LazyProxyTestInterface")])
            .identical(&Value::Array(definition.tag("proxy")))
    );
}

#[test]
fn testLazyServiceAttributeWithInterfaceProxiesUnionType() {
    let mut container = container();
    container
        .register("impl", Some(&c("FinalLazyProxyImplementation")))
        .unwrap();
    container
        .set_alias(&c("LazyProxyTestInterface"), "impl")
        .unwrap();
    container.set_alias(&c("AInterface"), "impl").unwrap();
    container
        .register(
            "foo",
            Some(&c("LazyServiceAttributeWithInterfaceOnUnionTypeAutowiring")),
        )
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let definition = container
        .get_definition(&string(&argument(&container, "foo", 0)))
        .unwrap();

    assert!(definition.is_lazy());
    assert!(
        proxy_tag(&[&c("LazyProxyTestInterface")])
            .identical(&Value::Array(definition.tag("proxy")))
    );
}

#[test]
fn testLazyServiceAttributeWithInterfaceProxiesWholeIntersectionType() {
    let mut container = container();
    container
        .register("impl", Some(&c("FinalLazyProxyImplementation")))
        .unwrap();
    container
        .set_alias(&c("LazyProxyTestInterface"), "impl")
        .unwrap();
    container.set_alias(&c("AInterface"), "impl").unwrap();
    container
        .register(
            "foo",
            Some(&c(
                "LazyServiceAttributeWithInterfaceOnIntersectionTypeAutowiring",
            )),
        )
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let definition = container
        .get_definition(&string(&argument(&container, "foo", 0)))
        .unwrap();

    assert!(definition.is_lazy());
    assert!(
        proxy_tag(&[&c("AInterface"), &c("LazyProxyTestInterface")])
            .identical(&Value::Array(definition.tag("proxy")))
    );
}

#[test]
fn testLazyNotCompatibleWithAutowire() {
    let mut container = container();
    container
        .register("a", Some(&c("A")))
        .unwrap()
        .set_autowired(true);
    container
        .register("foo", Some(&c("LazyAutowireServiceAttributesAutowiring")))
        .unwrap()
        .set_autowired(true);

    if let Err(e) = autowire(&mut container) {
        assert!(e.is(&ErrorKind::AutowiringFailed));
        assert_eq!(
            "Using both attributes #[Lazy] and #[Autowire] on an argument is not allowed; use the \"lazy\" parameter of #[Autowire] instead.",
            e.message()
        );
    }
}

#[test]
fn testAutowireAttributeWithEnvVar() {
    let mut container = container();

    container
        .register("foo", Some(&c("AutowireAttributeEnv")))
        .unwrap()
        .set_autowired(true);

    autowire(&mut container).unwrap();

    let arguments = arguments(&container, "foo");

    let resolved = |container: &mut ContainerBuilder, i: i64| {
        container
            .resolve_env_placeholders(
                arguments.get(&Key::Int(i)).unwrap().clone(),
                &EnvFormat::Placeholder,
                None,
            )
            .unwrap()
    };
    assert!(Value::from("%env(bool:ENABLED)%").identical(&resolved(&mut container, 0)));
    assert!(Value::from("%env(default::OPTIONAL)%").identical(&resolved(&mut container, 1)));
}

// ----- extras: what AutowireAsDecoratorPass alone leaves -----

#[test]
fn as_decorator_sets_the_decorated_service() {
    let mut container = container();
    container.register(&c("AsDecoratorFoo"), None).unwrap();
    container
        .register(&c("AsDecoratorBar10"), Some(&c("AsDecoratorBar10")))
        .unwrap()
        .set_autowired(true);
    container
        .register(&c("AsDecoratorBaz"), Some(&c("AsDecoratorBaz")))
        .unwrap()
        .set_autowired(true);

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    let bar = container.get_definition(&c("AsDecoratorBar10")).unwrap();
    let decorated = bar.decorated_service().unwrap();
    assert_eq!(c("AsDecoratorFoo"), decorated.id);
    assert_eq!(None, decorated.renamed_id);
    assert_eq!(10, decorated.priority);
    assert_eq!(None, decorated.invalid_behavior);

    let baz = container.get_definition(&c("AsDecoratorBaz")).unwrap();
    let decorated = baz.decorated_service().unwrap();
    assert_eq!("NonExistent", decorated.id);
    assert_eq!(Some(InvalidBehavior::Null), decorated.invalid_behavior);
}

#[test]
fn multiple_as_decorator_clones_the_definition() {
    let mut container = container();
    container
        .register(&c("AsDecoratorMultiple"), Some(&c("AsDecoratorMultiple")))
        .unwrap()
        .set_autowired(true)
        .set_argument(0, Value::from("arg1"));

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(!container.has_definition(&c("AsDecoratorMultiple")));
    for decorated in ["AsDecoratorMultipleFoo", "AsDecoratorMultipleBar"] {
        let id = format!(".decorator.{}.{}", c(decorated), c("AsDecoratorMultiple"));
        let definition = container.get_definition(&id).unwrap();
        assert_eq!(c(decorated), definition.decorated_service().unwrap().id);
        assert_eq!(Some(c("AsDecoratorMultiple")), definition.class());
    }
}

#[test]
fn as_tag_decorator_registers_a_tag_decorator_template() {
    let mut container = container();
    container
        .register(
            &c("AsTagDecoratorService"),
            Some(&c("AsTagDecoratorService")),
        )
        .unwrap()
        .set_autowired(true);

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(!container.has_definition(&c("AsTagDecoratorService")));
    let template = container
        .get_definition(&format!(
            ".tag_decorator.test.tag.{}",
            c("AsTagDecoratorService")
        ))
        .unwrap();
    let tags = template.tags();
    let tag = tags.get(&Key::from("container.tag_decorator")).unwrap();
    let expected = Value::Array(PhpArray::list([Value::Array(
        php_array!["decorates_tag" => "test.tag", "priority" => 0_i64],
    )]));
    assert!(expected.identical(tag), "{tag:?}");
    assert!(template.has_tag("container.excluded"));
}

#[test]
fn ignore_attributes_tag_and_non_autowired_definitions_are_skipped() {
    let mut container = container();
    container
        .register("a", Some(&c("AsDecoratorBar10")))
        .unwrap()
        .set_autowired(true)
        .add_tag("container.ignore_attributes", PhpArray::new());
    container
        .register("b", Some(&c("AsDecoratorBar10")))
        .unwrap();

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(
        container
            .get_definition("a")
            .unwrap()
            .decorated_service()
            .is_none()
    );
    assert!(
        container
            .get_definition("b")
            .unwrap()
            .decorated_service()
            .is_none()
    );
}
