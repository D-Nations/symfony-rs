//! YamlReferenceDumperTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; the data provider is a list of cases.
//!
//! Upstream: symfony/config Tests/Definition/Dumper/YamlReferenceDumperTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, configuration::ExampleConfiguration};
use php_value::{PhpArray, Value, php_array};
use symfony_config::definition::{builder::TreeBuilder, dumper::YamlReferenceDumper};

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn testDumper() -> TestResult {
    let configuration = ExampleConfiguration::new();

    let mut dumper = YamlReferenceDumper::new();

    assert_eq!(
        get_configuration_as_string(),
        text(&dumper.dump(&configuration)?)
    );
    Ok(())
}

fn provide_dump_at_path() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("Regular node", "scalar_true", "scalar_true:          true"),
        (
            "Array node",
            "array",
            "# some info
array:
    child1:               ~
    child2:               ~

    # this is a long
    # multi-line info text
    # which should be indented
    child3:               ~ # Example: 'example setting'",
        ),
        ("Regular nested", "array.child2", "child2:               ~"),
        (
            "Prototype",
            "cms_pages.page",
            "# Prototype
page:

    # Prototype
    locale:
        title:                ~ # Required
        path:                 ~ # Required",
        ),
        (
            "Nested prototype",
            "cms_pages.page.locale",
            "# Prototype
locale:
    title:                ~ # Required
    path:                 ~ # Required",
        ),
    ]
}

#[test]
fn testDumpAtPath() -> TestResult {
    for (name, path, expected) in provide_dump_at_path() {
        let configuration = ExampleConfiguration::new();

        let mut dumper = YamlReferenceDumper::new();

        assert_eq!(
            expected.trim(),
            text(&dumper.dump_at_path(&configuration, path.as_bytes())?).trim(),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn testDumpQuotesDefaultValuesOfArrays() -> TestResult {
    let tree_builder = TreeBuilder::new("acme_root")?;
    tree_builder
        .get_root_node()
        .children()
        .array_node("hosts", None)?
        .prototype("scalar")?
        .end()
        .default_value(Value::Array(PhpArray::list([
            Value::from("%env(HOSTS)%"),
            Value::from("*.example.com"),
        ])))?
        .end()
        .array_node("options", None)?
        .prototype("variable")?
        .end()
        .default_value(Value::Array(
            php_array!["enabled" => true, "path" => Value::Null, "logger" => "@logger"],
        ))?
        .end()
        .end();

    let mut dumper = YamlReferenceDumper::new();
    let reference = dumper.dump_node(&tree_builder.build_tree()?)?;

    assert_eq!(
        "acme_root:
    hosts:

        # Defaults:
        - '%env(HOSTS)%'
        - '*.example.com'
    options:

        # Defaults:
        enabled:             true
        path:                null
        logger:              '@logger'
",
        text(&reference)
    );
    assert_eq!(
        Value::Array(php_array![
            "acme_root" => Value::Array(php_array![
                "hosts" => Value::Array(PhpArray::list([
                    Value::from("%env(HOSTS)%"),
                    Value::from("*.example.com"),
                ])),
                "options" => Value::Array(
                    php_array!["enabled" => true, "path" => Value::Null, "logger" => "@logger"],
                ),
            ]),
        ]),
        symfony_yaml::parse(&reference, symfony_yaml::Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testDumpQuotesKeysAndEmptyArraysOfDefaultValues() -> TestResult {
    let defaults = || {
        php_array![
            "@logger" => true,
            "%kernel.debug%" => 1,
            "*.example.com" => Value::Array(PhpArray::new()),
            "a: b" => Value::Null,
        ]
    };
    let tree_builder = TreeBuilder::new("acme_root")?;
    tree_builder
        .get_root_node()
        .children()
        .array_node("options", None)?
        .prototype("variable")?
        .end()
        .default_value(Value::Array(defaults()))?
        .end()
        .end();

    let mut dumper = YamlReferenceDumper::new();
    let reference = dumper.dump_node(&tree_builder.build_tree()?)?;

    assert_eq!(
        "acme_root:
    options:

        # Defaults:
        '@logger':           true
        '%kernel.debug%':    1
        '*.example.com':     []
        'a: b':              null
",
        text(&reference)
    );
    assert_eq!(
        Value::Array(php_array![
            "acme_root" => Value::Array(php_array![
                "options" => Value::Array(defaults()),
            ]),
        ]),
        symfony_yaml::parse(&reference, symfony_yaml::Flags::NONE)?
    );
    Ok(())
}

fn get_configuration_as_string() -> &'static str {
    r#"acme_root:
    boolean:              true
    scalar_empty:         ~
    scalar_null:          null
    scalar_true:          true
    scalar_false:         false
    scalar_default:       default
    scalar_array_empty:   []
    scalar_array_defaults:

        # Defaults:
        - elem1
        - elem2
    scalar_required:      ~ # Required
    scalar_deprecated:    ~ # Deprecated (Since vendor/package 1.1: The child node "scalar_deprecated" at path "acme_root" is deprecated.)
    scalar_deprecated_with_message: ~ # Deprecated (Since vendor/package 1.1: Deprecation custom message for "scalar_deprecated_with_message" at "acme_root")
    node_with_a_looong_name: ~
    enum_with_default:    this # One of "this"; "that"
    enum:                 ~ # One of "this"; "that"; Symfony\Component\Config\Tests\Fixtures\TestEnum::Ccc
    enum_with_class:      ~ # One of "foo"; "bar baz"
    unit_enum_with_class: ~ # One of Symfony\Component\Config\Tests\Fixtures\TestEnum::Foo; Symfony\Component\Config\Tests\Fixtures\TestEnum::Bar; Symfony\Component\Config\Tests\Fixtures\TestEnum::Ccc

    # some info
    array:
        child1:               ~
        child2:               ~

        # this is a long
        # multi-line info text
        # which should be indented
        child3:               ~ # Example: 'example setting'
    scalar_prototyped:    []
    string_list_default_null: ~
    variable:             ~

        # Examples:
        # - foo
        # - bar
    parameters:

        # Prototype: Parameter name
        name:                 ~
    connections:

        # Prototype
        -
            user:                 ~
            pass:                 ~
    cms_pages:

        # Prototype
        page:

            # Prototype
            locale:
                title:                ~ # Required
                path:                 ~ # Required
    pipou:

        # Prototype
        name:                 []
    array_with_array_example_and_no_default_value: []

        # Examples:
        # - foo
        # - bar
    custom_node:          true
"#
}
