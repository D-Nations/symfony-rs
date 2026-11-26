//! Shared helpers of the loader tests: the upstream fixtures directory,
//! exception class checks across symfony-config's errors, and the
//! fixture classes of Tests/Fixtures as class metadata.
//!
//! Upstream: none
#![allow(dead_code, clippy::too_many_lines, clippy::disallowed_names)]

use std::path::PathBuf;
use std::sync::Arc;

use php_value::{PhpArray, php_array};
use symfony_config::resource::FileResource;
use symfony_config::{Class, Error as ConfigError};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry,
};
use symfony_dependency_injection::loader::from_config_error;

/// `realpath(__DIR__.'/../Fixtures')`, or `None` (with a note) when
/// `upstream/` is not fetched.
pub fn fixtures() -> Option<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../upstream/symfony/dependency-injection/Tests/Fixtures");
    // realpath() as the ported code computes it (not std::fs::canonicalize,
    // whose Windows paths are verbatim `\\?\` ones).
    let real = dir
        .is_dir()
        .then(|| FileResource::new(dir.to_string_lossy().as_bytes()).ok())
        .flatten();
    if real.is_none() {
        eprintln!("skipped: upstream/ is not fetched (scripts/fetch-upstream.sh)");
    }
    real.map(|real| String::from_utf8_lossy(real.get_resource()).into_owned())
}

const DI: &str = "Symfony\\Component\\DependencyInjection\\Exception\\";

/// `$e instanceof $class`, for config's classes, the component's
/// (`Symfony\...\Exception\X`) and the SPL ones.
pub fn instance_of(e: &ConfigError, class: &str) -> bool {
    let class = class.trim_start_matches('\\');
    if e.class().name() == class {
        return true;
    }
    match e.class() {
        Class::Other { .. } => from_config_error(e).instance_of(class),
        other => match class {
            "InvalidArgumentException" => other.is_invalid_argument(),
            "LogicException" => other.is_logic(),
            "RuntimeException" => other.is_runtime(),
            "Exception" | "Throwable" => true,
            _ => false,
        },
    }
}

/// The component's exception class `short`.
pub fn di(short: &str) -> String {
    format!("{DI}{short}")
}

pub fn message(e: &ConfigError) -> String {
    String::from_utf8_lossy(e.get_message()).into_owned()
}

/// `expectException()` + `expectExceptionMessage()`.
#[track_caller]
pub fn assert_error(e: &ConfigError, class: &str, contains: &str) {
    assert!(
        instance_of(e, class),
        "expected {class}, got {}: {}",
        e.class().name(),
        message(e)
    );
    assert!(
        message(e).contains(contains),
        "message {:?} does not contain {contains:?}",
        message(e)
    );
}

pub const FIXTURES_NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\";

/// `Symfony\Component\DependencyInjection\Tests\Fixtures\<short>`.
pub fn f(short: &str) -> String {
    format!("{FIXTURES_NS}{short}")
}

pub fn attr(short: &str, arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: format!("{ATTRIBUTE}{short}"),
        arguments,
    }
}

fn class(short: &str) -> ClassMetadata {
    ClassMetadata::class(&f(short))
}

fn interface(short: &str) -> ClassMetadata {
    ClassMetadata::interface(&f(short))
}

fn with_attributes(mut class: ClassMetadata, attributes: Vec<AttributeMetadata>) -> ClassMetadata {
    class.attributes = attributes;
    class
}

/// The classes of Fixtures/Prototype*, Fixtures/Utils and the attribute
/// classes they use, as their PHP files declare them.
pub fn registry() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    for short in [
        "Exclude",
        "When",
        "WhenNot",
        "AsAlias",
        "AutoconfigureTag",
        "Autoconfigure",
        "AsTaggedItem",
    ] {
        r.add_class(ClassMetadata::class(&format!("{ATTRIBUTE}{short}")));
    }

    // Prototype. An attribute subclass (`#[ProductionOnly]` extends
    // `When`) is registered as a class, and its arguments are the values
    // its constructor gives the parent (`env: 'prod'`).
    r.add_class(
        ClassMetadata::class(&f("Prototype\\ProductionOnly"))
            .with_parent(&format!("{ATTRIBUTE}When")),
    );
    r.add_class(
        ClassMetadata::class(&f("Prototype\\NeverInProduction"))
            .with_parent(&format!("{ATTRIBUTE}WhenNot")),
    );
    r.add_class(with_attributes(
        interface("Prototype\\FooInterface"),
        vec![attr(
            "Autoconfigure",
            php_array!["tags" => php_array!["foo"]],
        )],
    ));
    r.add_class(with_attributes(
        class("Prototype\\Foo").with_interfaces(&[
            &f("Prototype\\FooInterface"),
            &f("Prototype\\Sub\\BarInterface"),
        ]),
        vec![
            AttributeMetadata {
                class: f("Prototype\\ProductionOnly"),
                arguments: php_array!["env" => "prod"],
            },
            attr("When", php_array!["env" => "dev"]),
        ],
    ));
    r.add_class(with_attributes(
        class("Prototype\\NotFoo"),
        vec![
            AttributeMetadata {
                class: f("Prototype\\NeverInProduction"),
                arguments: php_array!["env" => "prod"],
            },
            attr("WhenNot", php_array!["env" => "dev"]),
        ],
    ));
    r.add_class(class("Prototype\\AbstractClass").with_abstract(true));
    r.add_class(interface("Prototype\\Sub\\BarInterface"));
    r.add_class(
        class("Prototype\\Sub\\Bar").with_interfaces(&[&f("Prototype\\Sub\\BarInterface")]),
    );
    r.add_class(class("Prototype\\OtherDir\\Baz"));
    r.add_class(class("Prototype\\OtherDir\\AnotherSub\\DeeperBaz"));
    for short in [
        "Prototype\\OtherDir\\Component1\\Dir1\\Service1",
        "Prototype\\OtherDir\\Component1\\Dir2\\Service2",
        "Prototype\\OtherDir\\Component2\\Dir1\\Service4",
        "Prototype\\OtherDir\\Component2\\Dir2\\Service5",
    ] {
        r.add_class(class(short));
    }
    r.add_class(
        class("Prototype\\BadClasses\\MissingParent")
            .with_parent(&f("Prototype\\BadClasses\\MissingClass")),
    );
    r.add_class(with_attributes(
        class("Prototype\\BadAttributes\\WhenNotWhenFoo"),
        vec![
            attr("When", php_array!["env" => "dev"]),
            attr("WhenNot", php_array!["env" => "test"]),
        ],
    ));
    r.add_class(interface(
        "Prototype\\StaticConstructor\\PrototypeStaticConstructorInterface",
    ));
    r.add_class(
        class("Prototype\\StaticConstructor\\PrototypeStaticConstructor").with_interfaces(&[&f(
            "Prototype\\StaticConstructor\\PrototypeStaticConstructorInterface",
        )]),
    );
    r.add_class(class(
        "Prototype\\StaticConstructor\\PrototypeStaticConstructorAsArgument",
    ));
    r.add_class(interface(
        "Prototype\\SinglyImplementedInterface\\Port\\PortInterface",
    ));
    r.add_class(
        class("Prototype\\SinglyImplementedInterface\\Adapter\\Adapter").with_interfaces(&[&f(
            "Prototype\\SinglyImplementedInterface\\Port\\PortInterface",
        )]),
    );
    r.add_class(
        class("Prototype\\SinglyImplementedInterface\\AnotherAdapter\\Adapter").with_interfaces(&[
            &f("Prototype\\SinglyImplementedInterface\\Port\\PortInterface"),
        ]),
    );

    // Utils
    r.add_class(with_attributes(
        class("Utils\\NotAService"),
        vec![attr("Exclude", PhpArray::new())],
    ));

    // PrototypeAsAlias
    let foo = f("PrototypeAsAlias\\AliasFooInterface");
    let bar = f("PrototypeAsAlias\\AliasBarInterface");
    r.add_class(interface("PrototypeAsAlias\\AliasFooInterface"));
    r.add_class(interface("PrototypeAsAlias\\AliasBarInterface"));
    let alias = |short: &str, interfaces: &[&str], attributes: Vec<AttributeMetadata>| {
        with_attributes(class(short).with_interfaces(interfaces), attributes)
    };
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAlias",
        &[&foo],
        vec![attr("AsAlias", php_array![foo.as_str()])],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasDuplicate",
        &[&foo],
        vec![attr("AsAlias", php_array![foo.as_str()])],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasInterface",
        &[&foo],
        vec![attr("AsAlias", PhpArray::new())],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasMultiple",
        &[&foo],
        vec![
            attr("AsAlias", php_array![foo.as_str(), true]),
            attr("AsAlias", php_array!["some-alias"]),
        ],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasIdMultipleInterface",
        &[&foo, &bar],
        vec![
            attr("AsAlias", php_array![bar.as_str()]),
            attr("AsAlias", php_array![foo.as_str()]),
        ],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasMultipleInterface",
        &[&foo, &bar],
        vec![attr("AsAlias", PhpArray::new())],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasProdEnv",
        &[&foo],
        vec![attr("AsAlias", php_array!["when" => "prod"])],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasDevEnv",
        &[&foo],
        vec![attr(
            "AsAlias",
            php_array!["id" => foo.as_str(), "when" => "dev"],
        )],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasBothEnv",
        &[&bar],
        vec![attr(
            "AsAlias",
            php_array!["id" => bar.as_str(), "when" => php_array!["dev", "prod"]],
        )],
    ));
    r.add_class(
        ClassMetadata::class(&f("PrototypeAsAlias\\AsProductionAlias"))
            .with_parent(&format!("{ATTRIBUTE}AsAlias")),
    );
    r.add_class(alias("PrototypeAsAlias\\WithCustomAsAlias", &[], vec![AttributeMetadata { class: f("PrototypeAsAlias\\AsProductionAlias"), arguments: php_array!["id" => foo.as_str(), "public" => false, "when" => php_array!["prod"]] }]));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasTargetOne",
        &[&foo],
        vec![attr("AsAlias", php_array!["target" => "one"])],
    ));
    r.add_class(alias(
        "PrototypeAsAlias\\WithAsAliasTargetTwo",
        &[&foo],
        vec![attr("AsAlias", php_array!["target" => "two"])],
    ));

    Arc::new(r)
}
