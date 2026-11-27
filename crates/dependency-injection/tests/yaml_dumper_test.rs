//! Tests/Dumper/YamlDumperTest.php at symfony/dependency-injection
//! 9b34cad8. The fixture containers are built in Rust
//! (support/containers.rs). Where upstream compares with a fixture file
//! (`assertStringEqualsFile`) or parses both sides
//! (`assertEqualYamlStructure`), the dump is compared byte for byte with
//! what PHP's YamlDumper writes for the same container
//! (tests/data/dumpers.json, `scripts/record.sh di-dumpers`), which is
//! that fixture file wherever upstream compares bytes.
//!
//! testDumpHandlesEnumeration: symfony-yaml dumps an enum case as
//! `!php/enum`, so the expected file is `services_with_enumeration_enum_tag.yml`,
//! as in PHP.
//!
//! Pending:
//! - testDumpLoad loads `services_dump_load.yml` with `YamlFileLoader`,
//!   which is not ported yet.
//!
//! The tests after the upstream ones are extras: the recorder's "edge"
//! container, and a tagged iterator with only `exclude_self: false`,
//! where upstream fails with PHP's `TypeError`.
//!
//! Upstream: symfony/dependency-injection Tests/Dumper/YamlDumperTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::doc_markdown)]

#[path = "support/containers.rs"]
mod containers;

use containers::*;
use php_value::PhpArray;
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::dumper::{DumperInterface, YamlDumper};

fn dump(container: &mut ContainerBuilder) -> Vec<u8> {
    YamlDumper::new(container)
        .dump(&PhpArray::new())
        .unwrap()
        .as_bytes()
        .to_vec()
}

/// `assertStringEqualsGeneratedFile()` (and `assertEqualYamlStructure()`),
/// against PHP's dump.
fn assert_recorded(case: &str, mut container: ContainerBuilder) {
    let expected = recorded(case).unwrap();
    let dumped = with_path_placeholder(&dump(&mut container));
    assert_eq!(
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&dumped),
        "{case}"
    );
    assert_eq!(expected, dumped, "{case}");
}

#[test]
fn testDump() {
    // ->dump() dumps an empty container as an empty YAML file
    assert_recorded("yaml:empty", ContainerBuilder::new());
}

#[test]
fn testAddParameters() {
    // ->dump() dumps parameters
    assert_recorded("yaml:container8", container8());
}

/// The pinned PHP YamlDumper quotes a Windows-style absolute file path.
/// The expected bytes were recorded with the Symfony PHP oracle.
#[test]
fn dump_windows_file_path_matches_php() {
    let mut container = ContainerBuilder::new();
    container
        .register("example", Some("Example"))
        .unwrap()
        .set_public(true)
        .set_file(Some(r"C:\project\includes\foo.php"));
    assert_eq!(
        include_bytes!("data/yaml_windows_file.yml").as_slice(),
        dump(&mut container)
    );
}

#[test]
fn testAddService() {
    // The PHP recording was made with a Unix path. Use a fixed Unix-style
    // value so the host's drive letter cannot change YAML quoting before
    // the path placeholder is substituted.
    let mut container = container9();
    container
        .get_definition("method_call1")
        .unwrap()
        .set_file(Some("/__php2rust_fixture__/foo.php"));
    let dumped = dump(&mut container);
    let normalized = String::from_utf8(dumped)
        .unwrap()
        .replace("/__php2rust_fixture__/", "%path%");
    assert_eq!(recorded("yaml:container9").unwrap(), normalized.as_bytes());

    let mut container = container_with_object_argument();
    let e = YamlDumper::new(&mut container)
        .dump(&PhpArray::new())
        .unwrap_err();
    assert!(e.instance_of("RuntimeException"));
    assert_eq!(
        "Unable to dump a service container if a parameter is an object or a resource, got \"stdClass\".",
        e.message()
    );
    assert_eq!(
        recorded("yaml:object_argument").unwrap_err(),
        (e.class(), e.message())
    );
}

#[test]
fn testDumpAutowireData() {
    assert_recorded("yaml:container24", container24());
}

#[test]
fn testDumpDecoratedServices() {
    assert_recorded("yaml:container34", container34());
}

#[test]
fn testInlineServices() {
    assert_recorded("yaml:inline_services", inline_services());
}

#[test]
fn testTaggedArguments() {
    assert_recorded("yaml:tagged_arguments", yaml_tagged_arguments(false));
}

#[test]
fn testDeprecatedTaggedArguments() {
    assert_recorded(
        "yaml:deprecated_tagged_arguments",
        yaml_tagged_arguments(true),
    );
}

#[test]
fn testTaggedArgumentsOmitsAutoDerivedDefaultMethods() {
    let mut container = tagged_with_index();
    let yaml = String::from_utf8(dump(&mut container)).unwrap();

    assert!(!yaml.contains("default_index_method"));
    assert!(!yaml.contains("default_priority_method"));
    assert!(yaml.contains("index_by: barfoo"));
    assert_eq!(
        String::from_utf8(recorded("yaml:tagged_with_index").unwrap()).unwrap(),
        yaml
    );
}

#[test]
fn testServiceClosure() {
    assert_recorded("yaml:service_closure", service_closure());
}

#[test]
fn testEnvClosure() {
    assert_recorded("yaml:env_closure", env_closure());
}

#[test]
fn testDumpHandlesEnumeration() {
    assert_recorded("yaml:enumeration", enumeration());
}

#[test]
fn testDumpHandlesDefaultAttribute() {
    for (kind, case) in [
        ("array", "yaml:default_array"),
        ("object", "yaml:default_object"),
        ("enum", "yaml:default_enumeration"),
    ] {
        assert_recorded(case, default_attribute(kind));
    }
}

#[test]
fn testDumpServiceWithAbstractArgument() {
    assert_recorded("yaml:abstract_argument", abstract_argument());
}

#[test]
fn testDumpNonScalarTags() {
    assert_recorded("yaml:non_scalar_tags", container_non_scalar_tags());
}

#[test]
fn testDumpResolvedEnvPlaceholders() {
    assert_recorded("yaml:env_placeholders", env_placeholders());
}

/// An env placeholder's random part (`bin2hex(random_bytes(16))`) as
/// `<random>`.
fn without_random_placeholder_part(yaml: &[u8]) -> Vec<u8> {
    let marker = b"_int_PORT_";
    let mut out = Vec::new();
    let mut i = 0;
    while i < yaml.len() {
        if yaml[i..].starts_with(marker) {
            out.extend_from_slice(marker);
            out.extend_from_slice(b"<random>");
            i += marker.len();
            while i < yaml.len() && yaml[i].is_ascii_hexdigit() {
                i += 1;
            }
        } else {
            out.push(yaml[i]);
            i += 1;
        }
    }
    out
}

#[test]
fn edge_container_dumps_as_php_does() {
    // The dumper leaves the env placeholders of a container that is not
    // compiled in its parameters, as upstream does.
    let mut container = edge();
    let expected = without_random_placeholder_part(&recorded("yaml:edge").unwrap());
    let dumped = without_random_placeholder_part(&dump(&mut container));
    assert_eq!(
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&dumped)
    );
}

#[test]
fn exclude_self_on_a_tag_name_fails_as_php_does() {
    let mut container = exclude_self_on_tag_name();
    let e = YamlDumper::new(&mut container)
        .dump(&PhpArray::new())
        .unwrap_err();
    assert_eq!(
        recorded("yaml:exclude_self_on_tag_name").unwrap_err(),
        (e.class(), e.message())
    );
}
