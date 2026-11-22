//! XmlReferenceDumperTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; `PHP_EOL` is `"\n"`, so upstream's
//! `str_replace("\n", PHP_EOL, ...)` leaves the strings as they are.
//!
//! Upstream: symfony/config Tests/Definition/Dumper/XmlReferenceDumperTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, configuration::ExampleConfiguration};
use symfony_config::definition::{builder::TreeBuilder, dumper::XmlReferenceDumper};

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn testDumper() -> TestResult {
    let configuration = ExampleConfiguration::new();

    let mut dumper = XmlReferenceDumper::new();
    assert_eq!(
        get_configuration_as_string(),
        text(&dumper.dump(&configuration, None)?)
    );
    Ok(())
}

#[test]
fn testNamespaceDumper() -> TestResult {
    let configuration = ExampleConfiguration::new();

    let mut dumper = XmlReferenceDumper::new();
    assert_eq!(
        get_configuration_as_string().replace(
            "http://example.org/schema/dic/acme_root",
            "http://symfony.com/schema/dic/symfony"
        ),
        text(&dumper.dump(
            &configuration,
            Some(b"http://symfony.com/schema/dic/symfony")
        )?)
    );
    Ok(())
}

#[test]
fn testDumpPrototypedArrayOfArraysWithoutKeyAttribute() -> TestResult {
    let tree_builder = TreeBuilder::new("acme_root")?;
    tree_builder
        .get_root_node()
        .children()
        .array_node("codes", None)?
        .array_prototype()?
        .scalar_prototype()?
        .end()
        .end()
        .end()
        .end();

    let mut dumper = XmlReferenceDumper::new();
    assert_eq!(
        "<!-- Namespace: http://example.org/schema/dic/acme_root -->
<config>

    <!-- prototype -->
    <codes>

        <!-- prototype -->
        <>scalar value</>

    </codes>

</config>
",
        text(&dumper.dump_node(&tree_builder.build_tree()?, None)?)
    );
    Ok(())
}

fn get_configuration_as_string() -> &'static str {
    r#"<!-- Namespace: http://example.org/schema/dic/acme_root -->
<!-- scalar-required: Required -->
<!-- scalar-deprecated: Deprecated (Since vendor/package 1.1: The child node "scalar_deprecated" at path "acme_root" is deprecated.) -->
<!-- scalar-deprecated-with-message: Deprecated (Since vendor/package 1.1: Deprecation custom message for "scalar_deprecated_with_message" at "acme_root") -->
<!-- enum-with-default: One of "this"; "that" -->
<!-- enum: One of "this"; "that"; Symfony\Component\Config\Tests\Fixtures\TestEnum::Ccc -->
<!-- enum-with-class: One of "foo"; "bar baz" -->
<!-- unit-enum-with-class: One of Symfony\Component\Config\Tests\Fixtures\TestEnum::Foo; Symfony\Component\Config\Tests\Fixtures\TestEnum::Bar; Symfony\Component\Config\Tests\Fixtures\TestEnum::Ccc -->
<!-- variable: Example: foo, bar -->
<config
    boolean="true"
    scalar-empty=""
    scalar-null="null"
    scalar-true="true"
    scalar-false="false"
    scalar-default="default"
    scalar-array-empty=""
    scalar-array-defaults="elem1,elem2"
    scalar-required=""
    scalar-deprecated=""
    scalar-deprecated-with-message=""
    node-with-a-looong-name=""
    enum-with-default="this"
    enum=""
    enum-with-class=""
    unit-enum-with-class=""
    variable=""
    custom-node="true"
>

    <!-- some info -->
    <!--
        child3: this is a long
                multi-line info text
                which should be indented;
                Example: example setting
    -->
    <array
        child1=""
        child2=""
        child3=""
    />

    <!-- prototype -->
    <scalar-prototyped>scalar value</scalar-prototyped>

    <!-- prototype -->
    <string-list-default-null>value</string-list-default-null>

    <!-- prototype: Parameter name -->
    <parameter name="parameter name">scalar value</parameter>

    <!-- prototype -->
    <connection
        user=""
        pass=""
    />

    <!-- prototype -->
    <cms-page page="cms page page">

        <!-- prototype -->
        <!-- title: Required -->
        <!-- path: Required -->
        <page
            locale="page locale"
            title=""
            path=""
        />

    </cms-page>

    <!-- prototype -->
    <pipou name="pipou name">

        <!-- prototype -->
        <name didou="" />

    </pipou>

    <array-with-array-example-and-no-default-value />

</config>
"#
}
