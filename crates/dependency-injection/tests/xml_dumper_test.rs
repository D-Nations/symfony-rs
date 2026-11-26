//! Tests/Dumper/XmlDumperTest.php at symfony/dependency-injection
//! 9b34cad8. The fixture containers are built in Rust
//! (support/containers.rs). Where upstream compares XML documents
//! (`assertXmlStringEqualsXmlFile`) or a fixture file, the dump is
//! compared byte for byte with what PHP's XmlDumper writes for the same
//! container (tests/data/dumpers.json, `scripts/record.sh di-dumpers`),
//! which is that fixture file wherever upstream compares bytes;
//! upstream's literal expectations are kept as they are.
//!
//! testCompiledContainerCanBeDumped compiles container9 only up to the
//! dump: compiling its `Expression` in PHP needs symfony/cache, so its
//! compiled dump is not recorded.
//!
//! The test after the upstream ones is an extra: the recorder's "edge"
//! container.
//!
//! Upstream: symfony/dependency-injection Tests/Dumper/XmlDumperTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::doc_markdown)]

#[path = "support/containers.rs"]
mod containers;

use containers::*;
use php_value::PhpArray;
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::dumper::{DumperInterface, XmlDumper};

fn dump(container: &mut ContainerBuilder) -> String {
    let out = XmlDumper::new(container).dump(&PhpArray::new()).unwrap();
    String::from_utf8(out.as_bytes().to_vec()).unwrap()
}

/// `assertXmlStringEqualsGeneratedXmlFile()`, against PHP's dump.
fn assert_recorded(case: &str, mut container: ContainerBuilder) {
    let expected = recorded(case).unwrap();
    let dumped = dump(&mut container);
    assert_eq!(
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&with_path_placeholder(dumped.as_bytes())),
        "{case}"
    );
}

const HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<container xmlns=\"http://symfony.com/schema/dic/services\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://symfony.com/schema/dic/services https://symfony.com/schema/dic/services/services-1.0.xsd\">
  <services>
    <service id=\"service_container\" class=\"Symfony\\Component\\DependencyInjection\\ContainerInterface\" public=\"true\" synthetic=\"true\"/>
";

#[test]
fn testDump() {
    // ->dump() dumps an empty container as an empty XML file
    assert_recorded("xml:empty", ContainerBuilder::new());
}

#[test]
fn testExportParameters() {
    // ->dump() dumps parameters
    assert_recorded("xml:container8", container8());
}

#[test]
fn testAddService() {
    // ->dump() dumps services
    assert_recorded("xml:container9", container9());

    let mut container = container_with_object_argument();
    let e = XmlDumper::new(&mut container)
        .dump(&PhpArray::new())
        .unwrap_err();
    assert!(e.instance_of("RuntimeException"));
    assert_eq!(
        "Unable to dump a service container if a parameter is an object or a resource, got \"stdClass\".",
        e.message()
    );
    assert_eq!(
        recorded("xml:object_argument").unwrap_err(),
        (e.class(), e.message())
    );
}

#[test]
fn testDumpAnonymousServices() {
    let mut container = container11();
    assert_eq!(
        format!(
            "{HEAD}    <service id=\"foo\" class=\"FooClass\" public=\"true\">
      <argument type=\"service\">
        <service class=\"BarClass\">
          <argument type=\"service\">
            <service class=\"BazClass\"/>
          </argument>
        </service>
      </argument>
    </service>
  </services>
</container>
"
        ),
        dump(&mut container)
    );
}

#[test]
fn testDumpEntities() {
    let mut container = container12();
    assert_eq!(
        format!(
            "{HEAD}    <service id=\"foo\" class=\"FooClass\\Foo\" public=\"true\">
      <tag name=\"foo&quot;bar\\bar\" foo=\"foo&quot;barřž€\"/>
      <argument>foo&lt;&gt;&amp;bar</argument>
    </service>
  </services>
</container>
"
        ),
        dump(&mut container)
    );
}

#[test]
fn testDumpDecoratedServices() {
    for (expected, mut container) in [
        (
            format!(
                "{HEAD}    <service id=\"foo\" class=\"FooClass\\Foo\" public=\"true\" decorates=\"bar\" decoration-inner-name=\"bar.woozy\"/>
  </services>
</container>
"
            ),
            container15(),
        ),
        (
            format!(
                "{HEAD}    <service id=\"foo\" class=\"FooClass\\Foo\" public=\"true\" decorates=\"bar\"/>
  </services>
</container>
"
            ),
            container16(),
        ),
        (
            format!(
                "{HEAD}    <service id=\"decorator\" decorates=\"decorated\" decoration-on-invalid=\"null\" decoration-inner-name=\"decorated.inner\" decoration-priority=\"1\"/>
  </services>
</container>
"
            ),
            container34(),
        ),
    ] {
        assert_eq!(expected, dump(&mut container));
    }
}

#[test]
fn testCompiledContainerCanBeDumped() {
    for (name, mut container) in [
        ("container8", container8()),
        ("container9", container9()),
        ("container11", container11()),
        ("container12", container12()),
        ("container14", container14()),
    ] {
        container.compile(false).unwrap();
        let dumped = dump(&mut container);
        if name != "container9" {
            assert_eq!(
                String::from_utf8(recorded(&format!("xml:compiled_{name}")).unwrap()).unwrap(),
                dumped,
                "{name}"
            );
        }
    }
}

#[test]
fn testDumpInlinedServices() {
    assert_recorded("xml:container21", container21());
}

#[test]
fn testDumpAutowireData() {
    assert_recorded("xml:container24", container24());
}

#[test]
fn testTaggedArguments() {
    assert_recorded("xml:tagged_arguments", tagged_arguments(false));
}

#[test]
fn testLegacyTaggedArguments() {
    assert_recorded("xml:legacy_tagged_arguments", tagged_arguments(true));
}

#[test]
fn testServiceClosure() {
    assert_recorded("xml:service_closure", service_closure());
}

#[test]
fn testEnvClosure() {
    assert_recorded("xml:env_closure", env_closure());
}

#[test]
fn testDumpAbstractServices() {
    assert_recorded("xml:container_abstract", container_abstract());
}

#[test]
fn testDumpHandlesEnumeration() {
    assert_recorded("xml:enumeration", enumeration());
}

#[test]
fn testDumpHandlesDefaultAttribute() {
    for (kind, case) in [
        ("array", "xml:default_array"),
        ("object", "xml:default_object"),
        ("enum", "xml:default_enumeration"),
    ] {
        // assertSame(file_get_contents($expectedFile), ...): the recording
        // is that file.
        assert_recorded(case, default_attribute(kind));
    }
}

#[test]
fn testDumpServiceWithAbstractArgument() {
    assert_recorded("xml:abstract_argument", abstract_argument());
}

#[test]
fn testDumpNonScalarTags() {
    assert_recorded("xml:non_scalar_tags", container_non_scalar_tags());
}

#[test]
fn edge_container_dumps_as_php_does() {
    assert_recorded("xml:edge", edge());
}
