//! The dumper tests' fixture containers: upstream's
//! `Tests/Fixtures/containers/*.php`, built in Rust, and the extra
//! containers whose dumps `oracle/recorders/di_dumpers.php` records.
//!
//! An `Expression` is a `\Stringable` object of its class.
//!
//! Upstream: none
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::disallowed_names
)]

use std::path::PathBuf;
use std::sync::Arc;

use php_value::object::{EnumCase, StdClass, StringableObject};
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::argument::{
    AbstractArgument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{AutowirePass, CompilerPassInterface};
use symfony_dependency_injection::definition::DefinitionError;
use symfony_dependency_injection::parameter_bag::ParameterBag;
use symfony_dependency_injection::{
    ContainerBuilder, Definition, InvalidBehavior, Parameter, Reference,
};

/// `realpath(__DIR__.'/../Fixtures')` of upstream's tests.
pub fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("upstream")
        .join("symfony")
        .join("dependency-injection")
        .join("Tests")
        .join("Fixtures")
}

/// The fixtures path as upstream's `realpath()` gives it, or as written
/// when upstream/ is not fetched (the dumps print it as `%path%`).
pub fn real_fixtures_path() -> PathBuf {
    let path = fixtures_path();
    // realpath() as the ported code computes it: std's canonicalize()
    // gives verbatim `\\?\` paths on Windows, which PHP never prints.
    symfony_config::resource::FileResource::new(path.to_string_lossy().as_bytes())
        .map(|real| PathBuf::from(String::from_utf8_lossy(real.get_resource()).into_owned()))
        .unwrap_or(path)
}

/// A PHP dump of `oracle/recorders/di_dumpers.php`
/// (`tests/data/dumpers.json`): the output, or the exception's class and
/// message.
pub fn recorded(case: &str) -> Result<Vec<u8>, (String, String)> {
    static DATA: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../data/dumpers.json")).unwrap()
    });
    let entry = DATA[case]
        .as_array()
        .unwrap_or_else(|| panic!("no recording {case}"));
    let text = |i: usize| entry[i].as_str().unwrap().to_string();
    if text(0) == "ok" {
        Ok(php_value::base64::decode(text(1).as_bytes()))
    } else {
        Err((text(1), text(2)))
    }
}

/// `$output` with the fixtures path of container9's file written as
/// `%path%`, as the recordings and upstream's fixtures write it.
pub fn with_path_placeholder(output: &[u8]) -> Vec<u8> {
    let prefix = format!(
        "{}{}",
        real_fixtures_path().join("includes").display(),
        std::path::MAIN_SEPARATOR
    );
    let text = String::from_utf8_lossy(output);
    text.replace(&prefix, "%path%").into_bytes()
}

/// A fixture file's bytes.
pub fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(fixtures_path().join(path)).unwrap()
}

pub fn reference(id: &str) -> Value {
    Value::from(Reference::new(id))
}

pub fn reference_with(id: &str, behavior: InvalidBehavior) -> Value {
    Value::from(Reference::with_behavior(id, behavior))
}

/// `new Expression($text)`.
pub fn expression(text: &str) -> Value {
    StringableObject::object("Symfony\\Component\\ExpressionLanguage\\Expression", text)
}

/// A case of a fixture enum.
pub fn enum_case(class: &str, name: &str) -> Value {
    Value::Object(EnumCase::new(class, name, None).object())
}

pub const FOO_UNIT_ENUM: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FooUnitEnum";

pub fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

/// `containers/container8.php`.
pub fn container8() -> ContainerBuilder {
    let parameters = php_array![
        "foo" => "%baz%",
        "baz" => "bar",
        "bar" => "foo is %%foo bar",
        "escape" => "@escapeme",
        "values" => Value::Array(list([
            Value::Bool(true),
            Value::Bool(false),
            Value::Null,
            Value::Int(0),
            Value::Float(1000.3),
            Value::from("true"),
            Value::from("false"),
            Value::from("null"),
        ])),
        "utf-8 valid string" => "\u{021b}\u{1b56}\ttest",
        "binary" => Value::String(b"\xf0\xf0\xf0\xf0".to_vec().into()),
        "binary-control-char" => "This is a Bell char \x07",
        "console banner" => "\x1b[37;44mHello\x1b[30;43mWorld\x1b[0m",
        "null string" => "null",
        "string of digits" => "123",
        "string of digits prefixed with minus character" => "-123",
        "true string" => "true",
        "false string" => "false",
        "binary number string" => "0b0110",
        "numeric string" => "-1.2E2",
        "hexadecimal number string" => "0xFF",
        "float string" => "10100.1",
        "positive float string" => "+10100.1",
        "negative float string" => "-10100.1",
    ];
    ContainerBuilder::with_parameter_bag(ParameterBag::new(&parameters).unwrap())
}

/// `containers/container9.php`.
pub fn container9() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let foo_args = php_array!["%foo%" => "foo is %foo%", "foobar" => "%foo%"];
    container
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag("foo", php_array!["foo" => "foo"])
        .add_tag("foo", php_array!["bar" => "bar", "baz" => "baz"])
        .add_tag("nullable", php_array!["bar" => "bar", "baz" => Value::Null])
        .add_tag("foo", php_array!["name" => "bar", "baz" => "baz"])
        .set_factory(Value::Array(list([
            Value::from("Bar\\FooClass"),
            Value::from("getInstance"),
        ])))
        .set_arguments(list([
            Value::from("foo"),
            reference("foo.baz"),
            Value::Array(foo_args.clone()),
            Value::Bool(true),
            reference("service_container"),
        ]))
        .set_properties(php_array![
            "foo" => "bar",
            "moo" => reference("foo.baz"),
            "qux" => Value::Array(foo_args),
        ])
        .add_method_call("setBar", list([reference("bar")]), false)
        .unwrap()
        .add_method_call("initialize", PhpArray::new(), false)
        .unwrap()
        .set_configurator("sc_configure")
        .set_public(true);
    container
        .register("foo.baz", Some("%baz_class%"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from("%baz_class%"),
            Value::from("getInstance"),
        ])))
        .set_configurator(Value::Array(list([
            Value::from("%baz_class%"),
            Value::from("configureStatic1"),
        ])))
        .set_public(true);
    container
        .register("bar", Some("Bar\\FooClass"))
        .unwrap()
        .set_arguments(list([
            Value::from("foo"),
            reference("foo.baz"),
            Value::from(Parameter::new("foo_bar")),
        ]))
        .set_configurator(Value::Array(list([
            reference("foo.baz"),
            Value::from("configure"),
        ])))
        .set_public(true);
    container
        .register("foo_bar", Some("%foo_class%"))
        .unwrap()
        .add_argument(reference("deprecated_service"))
        .set_shared(false)
        .set_public(true);
    container.get_parameter_bag_mut().clear().unwrap();
    container
        .get_parameter_bag_mut()
        .add(&php_array![
            "baz_class" => "BazClass",
            "foo_class" => "Bar\\FooClass",
            "foo" => "bar",
        ])
        .unwrap();
    let file = real_fixtures_path().join("includes").join("foo.php");
    container
        .register("method_call1", Some("Bar\\FooClass"))
        .unwrap()
        .set_file(Some(file.to_str().unwrap()))
        .add_method_call("setBar", list([reference("foo")]), false)
        .unwrap()
        .add_method_call(
            "setBar",
            list([reference_with("foo2", InvalidBehavior::Null)]),
            false,
        )
        .unwrap()
        .add_method_call(
            "setBar",
            list([reference_with("foo3", InvalidBehavior::Ignore)]),
            false,
        )
        .unwrap()
        .add_method_call(
            "setBar",
            list([reference_with("foobaz", InvalidBehavior::Ignore)]),
            false,
        )
        .unwrap()
        .add_method_call(
            "setBar",
            list([expression(
                "service(\"foo\").foo() ~ (container.hasParameter(\"foo\") ? parameter(\"foo\") : \"default\")",
            )]),
            false,
        )
        .unwrap()
        .set_public(true);
    container
        .register("foo_with_inline", Some("Foo"))
        .unwrap()
        .add_method_call("setBar", list([reference("inlined")]), false)
        .unwrap()
        .set_public(true);
    container
        .register("inlined", Some("Bar"))
        .unwrap()
        .set_property("pub", "pub")
        .add_method_call("setBaz", list([reference("baz")]), false)
        .unwrap();
    container
        .register("baz", Some("Baz"))
        .unwrap()
        .add_method_call("setFoo", list([reference("foo_with_inline")]), false)
        .unwrap()
        .set_public(true);
    container
        .register("request", Some("Request"))
        .unwrap()
        .set_synthetic(true)
        .set_public(true);
    container
        .register("configurator_service", Some("ConfClass"))
        .unwrap()
        .add_method_call("setFoo", list([reference("baz")]), false)
        .unwrap();
    container
        .register("configured_service", Some("stdClass"))
        .unwrap()
        .set_configurator(Value::Array(list([
            reference("configurator_service"),
            Value::from("configureStdClass"),
        ])))
        .set_public(true);
    container
        .register("configurator_service_simple", Some("ConfClass"))
        .unwrap()
        .add_argument("bar");
    container
        .register("configured_service_simple", Some("stdClass"))
        .unwrap()
        .set_configurator(Value::Array(list([
            reference("configurator_service_simple"),
            Value::from("configureStdClass"),
        ])))
        .set_public(true);
    container
        .register("decorated", Some("stdClass"))
        .unwrap()
        .set_public(true);
    container
        .register("decorator_service", Some("stdClass"))
        .unwrap()
        .set_decorated_service(Some("decorated"), None, 0, InvalidBehavior::Exception)
        .unwrap()
        .set_public(true);
    container
        .register("decorator_service_with_name", Some("stdClass"))
        .unwrap()
        .set_decorated_service(
            Some("decorated"),
            Some("decorated.pif-pouf"),
            0,
            InvalidBehavior::Exception,
        )
        .unwrap()
        .set_public(true);
    let deprecation = "The \"%service_id%\" service is deprecated. You should stop using it, as it will be removed in the future.";
    container
        .register("deprecated_service", Some("stdClass"))
        .unwrap()
        .set_deprecated("vendor/package", "1.1", deprecation)
        .unwrap()
        .set_public(true);
    container
        .register("new_factory", Some("FactoryClass"))
        .unwrap()
        .set_property("foo", "bar");
    container
        .register("factory_service", Some("Bar"))
        .unwrap()
        .set_factory(Value::Array(list([
            reference("foo.baz"),
            Value::from("getInstance"),
        ])))
        .set_public(true);
    container
        .register("new_factory_service", Some("FooBarBaz"))
        .unwrap()
        .set_property("foo", "bar")
        .set_factory(Value::Array(list([
            reference("new_factory"),
            Value::from("getInstance"),
        ])))
        .set_public(true);
    container
        .register("service_from_static_method", Some("Bar\\FooClass"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from("Bar\\FooClass"),
            Value::from("getInstance"),
        ])))
        .set_public(true);
    container
        .register("factory_simple", Some("SimpleFactoryClass"))
        .unwrap()
        .add_argument("foo")
        .set_deprecated("vendor/package", "1.1", deprecation)
        .unwrap();
    container
        .register("factory_service_simple", Some("Bar"))
        .unwrap()
        .set_factory(Value::Array(list([
            reference("factory_simple"),
            Value::from("getInstance"),
        ])))
        .set_public(true);
    container
        .register("lazy_context", Some("LazyContext"))
        .unwrap()
        .set_arguments(list([
            Value::from(IteratorArgument::new(php_array![
                "k1" => reference("foo.baz"),
                "k2" => reference("service_container"),
            ])),
            Value::from(IteratorArgument::new(PhpArray::new())),
        ]))
        .set_public(true);
    container
        .register("lazy_context_ignore_invalid_ref", Some("LazyContext"))
        .unwrap()
        .set_arguments(list([
            Value::from(IteratorArgument::new(list([
                reference("foo.baz"),
                reference_with("invalid", InvalidBehavior::Ignore),
            ]))),
            Value::from(IteratorArgument::new(PhpArray::new())),
        ]))
        .set_public(true);
    container
        .register("BAR", Some("stdClass"))
        .unwrap()
        .set_property("bar", reference("bar"))
        .set_public(true);
    container
        .register("bar2", Some("stdClass"))
        .unwrap()
        .set_public(true);
    container
        .register("BAR2", Some("stdClass"))
        .unwrap()
        .set_public(true);
    container
        .register("tagged_iterator_foo", Some("Bar"))
        .unwrap()
        .add_tag("foo", PhpArray::new());
    container
        .register("tagged_iterator", Some("Bar"))
        .unwrap()
        .add_argument(IteratorArgument::tagged(
            "foo",
            None,
            false,
            Vec::new(),
            true,
        ))
        .set_public(true);
    container
        .set_alias("alias_for_foo", "foo")
        .unwrap()
        .set_public(true);
    container
        .set_alias("alias_for_alias", "alias_for_foo")
        .unwrap()
        .set_public(true);

    container
        .register("runtime_error", Some("stdClass"))
        .unwrap()
        .add_argument(reference_with(
            "errored_definition",
            InvalidBehavior::RuntimeException,
        ))
        .set_public(true);

    container
        .register("errored_definition", Some("stdClass"))
        .unwrap()
        .add_error(DefinitionError::Text(
            "Service \"errored_definition\" is broken.".to_string(),
        ));

    container
        .register("preload_sidekick", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_tag(
            "container.preload",
            php_array!["class" => "Some\\Sidekick1"],
        )
        .add_tag(
            "container.preload",
            php_array!["class" => "Some\\Sidekick2"],
        );

    container.register("a_factory", Some("Bar")).unwrap();
    container
        .register("a_service", Some("Bar"))
        .unwrap()
        .set_factory(Value::Array(list([
            reference("a_factory"),
            Value::from("getBar"),
        ])))
        .set_public(true);
    container
        .register("b_service", Some("Bar"))
        .unwrap()
        .set_factory(Value::Array(list([
            reference("a_factory"),
            Value::from("getBar"),
        ])))
        .set_public(true);

    container
}

/// `containers/container10.php`.
pub fn container10() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass"))
        .unwrap()
        .add_argument(reference("bar"))
        .set_public(true);
    container
}

/// `containers/container11.php`.
pub fn container11() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let baz = Definition::with_class("BazClass");
    let bar = Definition::new(Some("BarClass"), list([Value::from(baz)]));
    container
        .register("foo", Some("FooClass"))
        .unwrap()
        .add_argument(bar)
        .set_public(true);
    container
}

/// `containers/container12.php`.
pub fn container12() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass\\Foo"))
        .unwrap()
        .add_argument("foo<>&bar")
        .add_tag("foo\"bar\\bar", php_array!["foo" => "foo\"barřž€"])
        .set_public(true);
    container
}

/// `containers/container13.php`.
pub fn container13() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass"))
        .unwrap()
        .add_argument(reference("bar"))
        .set_public(true);
    container
        .register("bar", Some("BarClass"))
        .unwrap()
        .set_public(true);
    container.compile(false).unwrap();
    container
}

/// `containers/container14.php`: an empty `ContainerBuilder` subclass.
pub fn container14() -> ContainerBuilder {
    ContainerBuilder::new()
}

/// `containers/container15.php`.
pub fn container15() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass\\Foo"))
        .unwrap()
        .set_decorated_service(
            Some("bar"),
            Some("bar.woozy"),
            0,
            InvalidBehavior::Exception,
        )
        .unwrap()
        .set_public(true);
    container
}

/// `containers/container16.php`.
pub fn container16() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass\\Foo"))
        .unwrap()
        .set_decorated_service(Some("bar"), None, 0, InvalidBehavior::Exception)
        .unwrap()
        .set_public(true);
    container
}

/// `containers/container17.php`.
pub fn container17() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("%foo.class%"))
        .unwrap()
        .set_public(true);
    container
}

/// `containers/container21.php`.
pub fn container21() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let bar = Definition::with_class("Bar");
    bar.set_configurator(Value::Array(list([
        Value::from(Definition::with_class("Baz")),
        Value::from("configureBar"),
    ])));
    let foo_factory = Definition::with_class("FooFactory");
    foo_factory.set_factory(Value::Array(list([
        Value::from(Definition::with_class("Foobar")),
        Value::from("createFooFactory"),
    ])));
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from(foo_factory),
            Value::from("createFoo"),
        ])))
        .set_configurator(Value::Array(list([
            Value::from(bar),
            Value::from("configureFoo"),
        ])))
        .set_public(true);
    container
}

/// `containers/container24.php`.
pub fn container24() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .set_autowired(true)
        .set_public(true);
    container
}

/// `containers/container34.php`.
pub fn container34() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("decorator", None)
        .unwrap()
        .set_decorated_service(
            Some("decorated"),
            Some("decorated.inner"),
            1,
            InvalidBehavior::Null,
        )
        .unwrap();
    container
}

/// `containers/container_abstract.php`.
pub fn container_abstract() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .set_abstract(true)
        .set_public(true);
    container
}

/// `containers/container_non_scalar_tags.php`.
pub fn container_non_scalar_tags() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag(
            "foo_tag",
            php_array![
                "name" => "attributeName",
                "foo" => "bar",
                "bar" => Value::Array(php_array!["foo" => "bar", "bar" => "foo"]),
            ],
        );
    container
}

/// A container with a `stdClass` argument, which the dumpers refuse.
pub fn container_with_object_argument() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("FooClass"))
        .unwrap()
        .add_argument(StdClass::object(PhpArray::new()))
        .set_public(true);
    container
}

/// XmlDumperTest::testTaggedArguments (and the legacy shape).
pub fn tagged_arguments(legacy: bool) -> ContainerBuilder {
    let (tagged, tagged2, tagged3) = if legacy {
        (
            IteratorArgument::tagged_legacy(
                "foo_tag",
                Some("barfoo"),
                Some("foobar"),
                false,
                Some("getPriority"),
                Vec::new(),
                true,
            ),
            IteratorArgument::tagged_legacy(
                "foo_tag",
                None,
                None,
                false,
                None,
                vec!["baz".to_string()],
                true,
            ),
            IteratorArgument::tagged_legacy(
                "foo_tag",
                None,
                None,
                false,
                None,
                vec!["baz".to_string(), "qux".to_string()],
                false,
            ),
        )
    } else {
        (
            IteratorArgument::tagged("foo_tag", Some("barfoo"), false, Vec::new(), true),
            IteratorArgument::tagged("foo_tag", None, false, vec!["baz".to_string()], true),
            IteratorArgument::tagged(
                "foo_tag",
                None,
                false,
                vec!["baz".to_string(), "qux".to_string()],
                false,
            ),
        )
    };

    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .add_tag("foo_tag", PhpArray::new());
    container
        .register("baz", Some("Baz"))
        .unwrap()
        .add_tag("foo_tag", PhpArray::new());
    container
        .register("qux", Some("Qux"))
        .unwrap()
        .add_tag("foo_tag", PhpArray::new());

    for (id, argument) in [
        ("foo_tagged_iterator", Value::from(tagged.clone())),
        ("foo2_tagged_iterator", Value::from(tagged2.clone())),
        ("foo3_tagged_iterator", Value::from(tagged3.clone())),
        (
            "foo_tagged_locator",
            Value::from(ServiceLocatorArgument::from_tagged(tagged)),
        ),
        (
            "foo2_tagged_locator",
            Value::from(ServiceLocatorArgument::from_tagged(tagged2)),
        ),
        (
            "foo3_tagged_locator",
            Value::from(ServiceLocatorArgument::from_tagged(tagged3)),
        ),
    ] {
        container
            .register(id, Some("Bar"))
            .unwrap()
            .set_public(true)
            .add_argument(argument);
    }
    container
}

/// testServiceClosure.
pub fn service_closure() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .add_argument(ServiceClosureArgument::new(reference_with(
            "bar",
            InvalidBehavior::Ignore,
        )));
    container
}

/// testEnvClosure.
pub fn env_closure() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .add_argument(EnvClosureArgument::new("%env(FOO)%", Value::Null, false).unwrap())
        .add_argument(EnvClosureArgument::new("%env(FOO)%", Value::Null, true).unwrap())
        .add_argument(EnvClosureArgument::new("%env(BAR)%", Value::from("def"), true).unwrap())
        .add_argument(EnvClosureArgument::new("%env(FOO)%", Value::Int(42), false).unwrap());
    container
}

/// testDumpHandlesEnumeration, compiled.
pub fn enumeration() -> ContainerBuilder {
    let class =
        "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FooClassWithEnumAttribute";
    let mut container = ContainerBuilder::new();
    container
        .register(class, Some(class))
        .unwrap()
        .set_public(true)
        .add_argument(enum_case(FOO_UNIT_ENUM, "BAR"));
    container
        .set_parameter("unit_enum", enum_case(FOO_UNIT_ENUM, "BAR"))
        .unwrap();
    container
        .set_parameter(
            "enum_array",
            Value::Array(list([
                enum_case(FOO_UNIT_ENUM, "BAR"),
                enum_case(FOO_UNIT_ENUM, "FOO"),
            ])),
        )
        .unwrap();
    container.compile(false).unwrap();
    container
}

/// testDumpServiceWithAbstractArgument.
pub fn abstract_argument() -> ContainerBuilder {
    let class = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FooWithAbstractArgument";
    let mut container = ContainerBuilder::new();
    container
        .register(class, Some(class))
        .unwrap()
        .set_argument("$baz", AbstractArgument::new("should be defined by Pass"))
        .set_argument("$bar", "test");
    container
}

/// testDumpHandlesDefaultAttribute: `FooClassWithDefault*Attribute`
/// (`array`, `object` or `enum`) autowired with `[2 => true]`.
pub fn default_attribute(kind: &str) -> ContainerBuilder {
    let fixtures = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
    let (class, first) = match kind {
        "array" => (
            format!("{fixtures}FooClassWithDefaultArrayAttribute"),
            ParameterMetadata::new("array", Some("array")).with_default(Value::Array(list([
                Value::from("a"),
                Value::from("b"),
                Value::from("c"),
            ]))),
        ),
        "object" => (
            format!("{fixtures}FooClassWithDefaultObjectAttribute"),
            ParameterMetadata::new("object", Some("object"))
                .with_default(StdClass::object(PhpArray::new())),
        ),
        _ => (
            format!("{fixtures}FooClassWithDefaultEnumAttribute"),
            ParameterMetadata::new("enum", Some(FOO_UNIT_ENUM))
                .with_default(enum_case(FOO_UNIT_ENUM, "FOO")),
        ),
    };
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class(&class).with_method(
        MethodMetadata::public("__construct").with_parameters(vec![
            first,
            ParameterMetadata::new("firstOptional", Some("bool")).with_default(Value::Bool(false)),
            ParameterMetadata::new("secondOptional", Some("bool")).with_default(Value::Bool(false)),
        ]),
    ));
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    let mut arguments = PhpArray::new();
    arguments.insert(2, Value::Bool(true));
    container
        .register("foo", Some(&class))
        .unwrap()
        .set_public(true)
        .set_autowired(true)
        .set_arguments(arguments);
    AutowirePass::new(true).process(&mut container).unwrap();
    container
}

/// YamlDumperTest::testTaggedArguments and testDeprecatedTaggedArguments.
pub fn yaml_tagged_arguments(legacy: bool) -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    for (id, class) in [
        ("foo_service", "Foo"),
        ("baz_service", "Baz"),
        ("qux_service", "Qux"),
    ] {
        container
            .register(id, Some(class))
            .unwrap()
            .add_tag("foo", PhpArray::new());
    }
    if legacy {
        let tagged = IteratorArgument::tagged_legacy(
            "foo",
            Some("barfoo"),
            Some("foobar"),
            false,
            Some("getPriority"),
            Vec::new(),
            true,
        );
        container
            .register("foo_service_tagged_iterator", Some("Bar"))
            .unwrap()
            .add_argument(tagged.clone());
        container
            .register("foo_service_tagged_locator", Some("Bar"))
            .unwrap()
            .add_argument(ServiceLocatorArgument::from_tagged(tagged));
        return container;
    }
    let tagged = IteratorArgument::tagged("foo", None, false, vec!["baz".to_string()], true);
    let tagged2 = IteratorArgument::tagged(
        "foo",
        None,
        false,
        vec!["baz".to_string(), "qux".to_string()],
        false,
    );
    container
        .register("foo_service_tagged_iterator", Some("Bar"))
        .unwrap()
        .add_argument(tagged.clone());
    container
        .register("foo2_service_tagged_iterator", Some("Bar"))
        .unwrap()
        .add_argument(tagged2.clone());
    container
        .register("foo_service_tagged_locator", Some("Bar"))
        .unwrap()
        .add_argument(ServiceLocatorArgument::from_tagged(tagged));
    container
        .register("foo2_service_tagged_locator", Some("Bar"))
        .unwrap()
        .add_argument(ServiceLocatorArgument::from_tagged(tagged2));
    container
        .register("bar_service_tagged_locator", Some("Bar"))
        .unwrap()
        .add_argument(ServiceLocatorArgument::from_tagged(
            IteratorArgument::tagged("foo", None, false, Vec::new(), true),
        ));
    container
}

/// YamlDumperTest::testInlineServices.
pub fn inline_services() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let inner = Definition::with_class("Class2");
    inner.add_argument(Definition::with_class("Class2"));
    container
        .register("foo", Some("Class1"))
        .unwrap()
        .set_public(true)
        .add_argument(inner);
    container
}

/// YamlDumperTest::testTaggedArgumentsOmitsAutoDerivedDefaultMethods.
pub fn tagged_with_index() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo_service", Some("Foo"))
        .unwrap()
        .add_tag("foo", PhpArray::new());
    container
        .register("with_index", Some("Bar"))
        .unwrap()
        .add_argument(IteratorArgument::tagged(
            "foo",
            Some("barfoo"),
            false,
            Vec::new(),
            true,
        ));
    container
}

/// YamlDumperTest::testDumpResolvedEnvPlaceholders, compiled.
pub fn env_placeholders() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .set_parameter("%env(PARAMETER_NAME)%", "%env(PARAMETER_VALUE)%")
        .unwrap();
    container
        .register("service", Some("%env(SERVICE_CLASS)%"))
        .unwrap()
        .set_file(Some("%env(SERVICE_FILE)%"))
        .add_argument("%env(SERVICE_ARGUMENT)%")
        .set_property(
            "%env(SERVICE_PROPERTY_NAME)%",
            "%env(SERVICE_PROPERTY_VALUE)%",
        )
        .add_method_call(
            "%env(SERVICE_METHOD_NAME)%",
            list([Value::from("%env(SERVICE_METHOD_ARGUMENT)%")]),
            false,
        )
        .unwrap()
        .set_factory("%env(SERVICE_FACTORY)%")
        .set_configurator("%env(SERVICE_CONFIGURATOR)%")
        .set_public(true);
    container.compile(false).unwrap();
    container
}

/// A tagged iterator with only `exclude_self: false`, which upstream's
/// YamlDumper sets on the tag name (a string).
pub fn exclude_self_on_tag_name() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .add_argument(IteratorArgument::tagged(
            "foo",
            None,
            false,
            Vec::new(),
            false,
        ));
    container
}

/// GraphvizDumperTest::testDumpWithInlineDefinition.
pub fn inline_definition() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let inline = Definition::with_class("stdClass");
    inline.add_argument(reference("bar"));
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(inline);
    container.register("bar", Some("stdClass")).unwrap();
    container
}

/// The recorder's `edge()`: what the upstream tests leave open.
pub fn edge() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    let port = container.get_parameter_bag().get("env(int:PORT)").unwrap();
    let port = port.as_string().unwrap().to_string_lossy().into_owned();
    let mut holes = PhpArray::new();
    holes.insert(1, Value::from("a"));
    holes.insert(3, Value::from("b"));
    let parameters = php_array![
        "cr" => "a\rb",
        "quotes" => "quote\"apos'",
        "entities" => "<&>",
        "one" => Value::Float(1.0),
        "sum" => Value::Float(0.1 + 0.2),
        "negzero" => Value::Float(-0.0),
        "big" => Value::Float(1e100),
        "max" => Value::Int(i64::MAX),
        "holes" => Value::Array(holes),
        "empty_array" => Value::Array(PhpArray::new()),
        "zero" => "0",
        "empty" => "",
        "lead_space" => " 12",
        "trail_space" => "12 ",
        "upper_bin" => "0B01",
        "bare_hex" => "0x",
        "bad_hex" => "0xg",
        "bin_nl" => "0b01\n",
        "null_nl" => "null\n",
        "invalid_utf8" => Value::String(b"a\xffb".to_vec().into()),
        "tab_nl" => "a\tb\nc",
        "at" => "@foo",
        "percent" => "100%",
        "ref_param" => "edge",
        "nested" => Value::Array(php_array!["k\"ey" => Value::Array(php_array!["<k>" => "%x%"])]),
        "port" => format!("http://host:{port}"),
    ];
    container.get_parameter_bag_mut().add(&parameters).unwrap();

    let mut nested_array = php_array![
        "k\"ey" => "v",
        "<k>" => Value::Array(php_array!["x" => reference("nested_target")]),
    ];
    nested_array.insert(5, Value::from("five"));
    let inline = Definition::with_class("Inline");
    inline
        .add_argument(Value::String(b"\xfe".to_vec().into()))
        .add_argument(reference("inline_target"))
        .add_tag("inline_tag", php_array!["a" => "b"]);

    container
        .register("edge", Some("\\Leading\\Slash"))
        .unwrap()
        .set_lazy(true)
        .set_autoconfigured(true)
        .set_shared(false)
        .set_file(Some("/path/to/file.php"))
        .add_tag("int_keys", list([Value::from("x"), Value::from("y")]))
        .add_tag(
            "scalars",
            php_array![
                "t" => Value::Bool(true),
                "f" => Value::Bool(false),
                "i" => Value::Int(7),
                "fl" => Value::Float(1.5),
                "n" => Value::Null,
                "q" => "a\"b<c>",
            ],
        )
        .add_tag(
            "named",
            php_array![
                "name" => "the name",
                "nested" => Value::Array(php_array![
                    "n" => Value::Null,
                    "e" => "",
                    "v" => "v",
                    "deep" => Value::Array(php_array!["z" => Value::Int(0)]),
                ]),
            ],
        )
        .add_tag("0", PhpArray::new())
        .add_tag("cr", php_array!["v" => "x\ry"])
        .add_error(DefinitionError::Text("First \"error\".".to_string()))
        .add_error(DefinitionError::Text("Second <error>.".to_string()))
        .set_arguments(list([
            Value::from(ServiceLocatorArgument::new(php_array![
                "a" => reference("missing_a"),
                "b" => reference_with("missing_b", InvalidBehavior::Null),
            ])),
            Value::from(ServiceClosureArgument::new(Value::from(
                Definition::with_class("ClosureTarget"),
            ))),
            Value::from(ServiceClosureArgument::new(reference_with(
                "lazy_target",
                InvalidBehavior::IgnoreUninitialized,
            ))),
            Value::from(IteratorArgument::new(list([reference_with(
                "iter_target",
                InvalidBehavior::IgnoreUninitialized,
            )]))),
            Value::from(IteratorArgument::tagged(
                "app.handler",
                Some("foo-bar baz"),
                false,
                Vec::new(),
                true,
            )),
            Value::from(IteratorArgument::tagged(
                "app.handler",
                None,
                true,
                Vec::new(),
                true,
            )),
            Value::from(ServiceLocatorArgument::from_tagged(
                IteratorArgument::tagged(
                    "app.locator",
                    Some("key"),
                    false,
                    vec!["one".to_string(), "two".to_string()],
                    true,
                ),
            )),
            Value::from(Parameter::new("ref_param")),
            expression("1 + \"two\""),
            Value::from(AbstractArgument::new("to be set.")),
            enum_case(FOO_UNIT_ENUM, "FOO"),
            Value::from("\x01\x02"),
            Value::String(b"\xff".to_vec().into()),
            Value::from("a\rb"),
            Value::from(""),
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            Value::Int(0),
            Value::Float(-1.5),
            Value::from("42"),
            Value::from("null"),
            Value::Array(nested_array),
            Value::Array(PhpArray::new()),
            Value::from(format!("http://{port}/")),
            Value::from(inline),
        ]))
        .set_properties(php_array![
            "p" => reference("prop_target"),
            "q" => "quoted \"q\"",
        ])
        .add_method_call("withClone", list([Value::from("x")]), true)
        .unwrap()
        .add_method_call("noArgs", PhpArray::new(), true)
        .unwrap()
        .add_method_call("call", list([reference("call_target")]), false)
        .unwrap()
        .set_decorated_service(
            Some("decorated_one"),
            Some("inner"),
            -5,
            InvalidBehavior::Ignore,
        )
        .unwrap()
        .set_configurator("configure_function");

    container
        .register("closure_factory", Some("Closure"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from("Closure"),
            Value::from("fromCallable"),
        ])))
        .set_public(true);
    container
        .register("null_class", None)
        .unwrap()
        .set_factory(Value::Array(list([Value::Null, Value::from("create")])));
    container
        .register("invokable", Some("Invokable"))
        .unwrap()
        .set_factory(reference("edge"));
    container
        .register("function_factory", Some("Made"))
        .unwrap()
        .set_factory("make_it");
    container
        .register("we\"ird<&>", Some("Weird"))
        .unwrap()
        .set_abstract(true)
        .set_deprecated(
            "vendor/pkg",
            "2.0",
            "The \"%service_id%\" <service> is \"gone\".",
        )
        .unwrap();
    container
        .set_alias("edge_alias", "edge")
        .unwrap()
        .set_deprecated("vendor/pkg", "3.0", "The \"%alias_id%\" alias is gone.")
        .unwrap();
    container
        .set_alias("public_alias", "edge")
        .unwrap()
        .set_public(true);
    container
        .set_alias("chained_private", "public_alias")
        .unwrap();
    container.set_alias("private_alias", "invokable").unwrap();
    container
}
