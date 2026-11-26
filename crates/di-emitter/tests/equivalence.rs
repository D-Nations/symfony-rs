//! The two readers agree: for containers built and compiled in Rust,
//! `Graph::from_container_builder(cb)` equals `Graph::from_xml()` of
//! `XmlDumper`'s dump of `cb`, and both graphs emit the same Rust (or
//! both are refused).
//!
//! The containers are the upstream YAML fixtures
//! (`Tests/Fixtures/yaml`, loaded with `YamlFileLoader`) and the upstream
//! test containers that the DI crate rebuilds in Rust
//! (`crates/dependency-injection/tests/support/containers.rs`). Before
//! compiling, every service and alias is made public (the removal passes
//! would otherwise leave little more than `service_container`), and each
//! class a definition names is registered as an empty class. Fixtures
//! that the Rust loader or compiler passes refuse are not listed: they
//! test the DI crate, not the emitter.
//!
//! As `oracle/recorders/di_container.php` does for the demo, the XML side
//! dumps the container after replacing each tagged iterator by a plain
//! iterator of its resolved values (`untag()`); the direct reader reads
//! tagged iterators that way itself.
//!
//! Tests that read `upstream/` skip, with a note, when it is not fetched.
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::doc_markdown)]

#[path = "../../dependency-injection/tests/support/containers.rs"]
mod containers;
#[path = "../../dependency-injection/tests/loader_support/mod.rs"]
mod loader_support;

use std::path::Path;
use std::sync::Arc;

use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::FileLocator;
use symfony_config::loader::LoaderInterface;
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::class_metadata::ClassMetadata;
use symfony_dependency_injection::dumper::XmlDumper;
use symfony_dependency_injection::loader::{YamlFileLoader, shared};
use symfony_dependency_injection::{ContainerBuilder, Definition, Reference};
use symfony_di_emitter::model::ArgDef;
use symfony_di_emitter::{Config, DumperParameters, Error, Graph, Location, compile};

/// What both readers make of a container.
#[derive(Debug)]
enum Outcome {
    /// Equal graphs, which emit the same code; the number of
    /// definitions (inline ones included).
    Equal(usize),
    /// Both refuse: the direct reader's error and the XML reader's.
    Refused(Error, Error),
}

fn untag_value(value: &Value) -> Value {
    if let Some(iterator) = IteratorArgument::from_value(value) {
        let values = untag_array(&iterator.values());
        if iterator.is_tagged() {
            return IteratorArgument::new(values).to_value();
        }
        iterator.set_values(values);
        return value.clone();
    }
    if let Some(d) = Definition::from_value(value) {
        untag(&d);
        return value.clone();
    }
    if let Value::Array(a) = value {
        return Value::Array(untag_array(a));
    }
    value.clone()
}

fn untag_array(a: &PhpArray) -> PhpArray {
    let mut out = PhpArray::new();
    for (k, v) in a.iter() {
        out.insert(k.clone(), untag_value(v));
    }
    out
}

/// `untag()` of `oracle/recorders/di_container.php`.
fn untag(d: &Definition) {
    d.set_arguments(untag_array(&d.arguments()));
    d.set_properties(untag_array(&d.properties()));
    let mut calls = PhpArray::new();
    for (k, call) in d.method_calls().iter() {
        let call = match call {
            Value::Array(call) => Value::Array(untag_array(call)),
            other => other.clone(),
        };
        calls.insert(k.clone(), call);
    }
    d.set_method_calls(&calls).unwrap();
    for callable in [d.factory(), d.configurator()] {
        if let Value::Array(c) = callable
            && let Some(inner) = c.get(&Key::Int(0)).and_then(Definition::from_value)
        {
            untag(&inner);
        }
    }
}

/// Every service and alias public; every class a definition names (or
/// a class-like id without a class) known to the registry.
fn prepare(cb: &mut ContainerBuilder) {
    let mut registry = Arc::unwrap_or_clone(cb.class_registry());
    let mut add = |name: &str| {
        let name = name.trim_start_matches('\\');
        if !name.is_empty() && !name.contains('%') && registry.class(name).is_none() {
            registry.add_class(ClassMetadata::class(name));
        }
    };
    for (id, d) in cb.get_definitions().iter() {
        d.set_public(true);
        match d.class() {
            Some(class) => add(&class),
            None if id.contains('\\') => add(id),
            None => {}
        }
    }
    for class in cb.get_autoconfigured_instanceof().keys() {
        add(class);
    }
    for (_, a) in cb.get_aliases().iter() {
        a.set_public(true);
    }
    cb.set_class_registry(Arc::new(registry));
}

fn dumper_parameters() -> DumperParameters {
    DumperParameters::from_seed(b"equivalence")
}

/// Reads a compiled container both ways and compares.
#[track_caller]
fn both_ways(name: &str, cb: &mut ContainerBuilder) -> Outcome {
    let dp = dumper_parameters();
    let direct = Graph::from_container_builder(cb, &dp);
    for d in cb.get_definitions().values() {
        untag(d);
    }
    let xml = XmlDumper::new(cb).dump_xml().unwrap();
    let via_xml = Graph::from_xml(xml.as_bytes(), &dp);
    match (direct, via_xml) {
        (Ok(direct), Ok(via_xml)) => {
            assert_eq!(direct, via_xml, "{name}: the graphs differ");
            let config = Config {
                request_scoped_tags: vec!["kernel.reset".into()],
                request_scoped_ids: Vec::new(),
            };
            match (compile(&direct, &config), compile(&via_xml, &config)) {
                (Ok(a), Ok(b)) => assert!(a.code == b.code, "{name}: the emitted code differs"),
                (Err(a), Err(b)) => assert_eq!(a.message, b.message, "{name}"),
                (a, b) => panic!("{name}: one graph compiles: {:?} {:?}", a.err(), b.err()),
            }
            Outcome::Equal(direct.services.len())
        }
        (Err(direct), Err(via_xml)) => Outcome::Refused(direct, via_xml),
        (direct, via_xml) => panic!(
            "{name}: one reader refuses: direct {:?}, XML {:?}",
            direct.err(),
            via_xml.err()
        ),
    }
}

#[track_caller]
fn compiled(name: &str, mut cb: ContainerBuilder) -> Outcome {
    if !cb.is_compiled() {
        prepare(&mut cb);
        cb.compile(false)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message()));
    }
    both_ways(name, &mut cb)
}

#[track_caller]
fn assert_equal(name: &str, outcome: Outcome, services: usize) {
    match outcome {
        Outcome::Equal(n) => assert_eq!(n, services, "{name}: definitions"),
        refused @ Outcome::Refused(..) => panic!("{name}: {refused:?}"),
    }
}

#[track_caller]
fn assert_refused(
    name: &str,
    outcome: Outcome,
    message: &str,
    xml_message: Option<&str>,
    location: &Location,
) {
    let Outcome::Refused(direct, via_xml) = outcome else {
        panic!("{name}: not refused: {outcome:?}");
    };
    assert_eq!(direct.message, message, "{name}");
    assert_eq!(&direct.location, location, "{name}");
    assert_eq!(via_xml.message, xml_message.unwrap_or(message), "{name}");
    assert!(
        matches!(via_xml.location, Location::Line(_)),
        "{name}: {:?}",
        via_xml.location
    );
}

/// The YAML fixtures that load and compile in Rust, with the number of
/// definitions both readers find (`service_container` and inline ones
/// included).
const YAML_EQUAL: &[(&str, usize)] = &[
    ("class_from_id.yml", 2),
    (
        "not_singly_implemented_interface_in_multiple_resources.yml",
        3,
    ),
    (
        "not_singly_implemented_interface_in_multiple_resources_with_previously_registered_alias.yml",
        3,
    ),
    (
        "not_singly_implemented_interface_in_multiple_resources_with_previously_registered_alias2.yml",
        3,
    ),
    ("service_instanceof_factory.yml", 4),
    ("services1.yml", 1),
    ("services13.yml", 1),
    ("services14.yml", 4),
    ("services23.yml", 2),
    ("services24.yml", 2),
    ("services28.yml", 7),
    ("services29.yml", 1),
    ("services3.yml", 1),
    ("services30.yml", 1),
    ("services4_bad_import.yml", 1),
    ("services4_bad_import_file_not_found.yml", 1),
    ("services7.yml", 2),
    ("services_adawson.yml", 8),
    ("services_autoconfigure_with_parent.yml", 3),
    ("services_case.yml", 4),
    ("services_configurator_short_syntax.yml", 3),
    ("services_decorates_tag.yml", 3),
    ("services_deep_graph.yml", 7),
    ("services_defaults_with_parent.yml", 3),
    ("services_dump_load.yml", 1),
    ("services_inline.yml", 4),
    ("services_instanceof_with_parent.yml", 3),
    ("services_not_existing.yml", 2),
    ("services_prototype.yml", 4),
    ("services_prototype_namespace.yml", 5),
    ("services_resource_tags.yml", 2),
    ("services_with_array_tags.yml", 2),
    ("services_with_deprecated_tagged_argument.yml", 7),
    ("services_with_service_closure.yml", 2),
    ("services_with_tagged_argument.yml", 10),
    ("singly_implemented_interface_in_multiple_resources.yml", 2),
    ("stack.yaml", 19),
    ("stack_decorates.yaml", 4),
    ("stack_decorates_tag.yaml", 7),
    ("static_constructor.yml", 2),
    ("tag_array_arguments.yml", 2),
    ("tag_name_only.yml", 2),
    ("tagged_iterator_optional.yml", 2),
    ("when-env.yaml", 1),
];

/// YAML fixtures both readers refuse: the message (the XML reader's, when
/// it differs) and the direct reader's location.
fn yaml_refused() -> Vec<(&'static str, &'static str, Option<&'static str>, Location)> {
    vec![
        (
            "container_with_env_placeholders.yml",
            "unsupported element <file> in a service",
            None,
            Location::Service("service".into()),
        ),
        (
            "services2.yml",
            "unsupported parameter type \"service\"",
            // XmlDumper writes the reference's id as an attribute.
            Some("unsupported attribute id=\"foo_bar\" on <parameter>"),
            Location::Parameter("foo_bar".into()),
        ),
        (
            "services8.yml",
            "unsupported parameter type \"binary\"",
            None,
            Location::Parameter("binary".into()),
        ),
        (
            "services_with_env_closure.yml",
            "unsupported argument type=\"env_closure\"",
            None,
            Location::Service("foo".into()),
        ),
        (
            "services_with_service_locator_argument.yml",
            "unsupported argument type=\"service_closure\" of a value that is not a reference",
            None,
            Location::Service("locator_dependent_inline_service".into()),
        ),
    ]
}

fn load_yaml(dir: &Path, file: &str) -> ContainerBuilder {
    let mut builder = ContainerBuilder::new();
    builder.set_class_registry(loader_support::registry());
    let container = shared(builder);
    let loader = YamlFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::new([dir.to_string_lossy().into_owned()])),
        None,
        false,
    );
    loader
        .load(&Value::from(file), None)
        .unwrap_or_else(|e| panic!("{file}: {}", String::from_utf8_lossy(e.get_message())));
    drop(loader);
    Arc::try_unwrap(container)
        .ok()
        .unwrap()
        .into_inner()
        .unwrap()
}

#[test]
fn yaml_fixtures() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let dir = Path::new(&fixtures).join("yaml");
    for &(file, services) in YAML_EQUAL {
        assert_equal(file, compiled(file, load_yaml(&dir, file)), services);
    }
    for (file, message, xml_message, location) in yaml_refused() {
        let outcome = compiled(file, load_yaml(&dir, file));
        assert_refused(file, outcome, message, xml_message, &location);
    }
}

/// A test container's constructor.
type Builder = fn() -> ContainerBuilder;

#[test]
fn upstream_test_containers() {
    use containers::*;
    let equal: Vec<(&str, Builder, usize)> = vec![
        ("container11", container11, 4),
        ("container12", container12, 2),
        ("container13", container13, 3),
        ("container14", container14, 1),
        ("container21", container21, 6),
        ("container24", container24, 2),
        ("container_abstract", container_abstract, 1),
        ("container_non_scalar_tags", container_non_scalar_tags, 2),
        ("service_closure", service_closure, 2),
        ("inline_services", inline_services, 4),
        ("tagged_with_index", tagged_with_index, 3),
        ("exclude_self_on_tag_name", exclude_self_on_tag_name, 2),
        ("inline_definition", inline_definition, 4),
        ("tagged_arguments", || tagged_arguments(false), 13),
        ("tagged_arguments (legacy)", || tagged_arguments(true), 13),
        ("yaml_tagged_arguments", || yaml_tagged_arguments(false), 10),
    ];
    for (name, container, services) in equal {
        assert_equal(name, compiled(name, container()), services);
    }
    let refused: Vec<(&str, Builder, &str, Location)> = vec![
        (
            "container8",
            container8,
            "unsupported parameter type \"binary\"",
            Location::Parameter("binary".into()),
        ),
        (
            "container9",
            container9,
            "You have requested a non-existent parameter \"foo_bar\".",
            Location::Service("bar".into()),
        ),
        (
            "env_closure",
            env_closure,
            "unsupported argument type=\"env_closure\"",
            Location::Service("foo".into()),
        ),
        (
            "enumeration",
            enumeration,
            "unsupported parameter type \"constant\"",
            Location::Parameter("unit_enum".into()),
        ),
        (
            "env_placeholders",
            env_placeholders,
            "unsupported element <file> in a service",
            Location::Service("service".into()),
        ),
    ];
    for (name, container, message, location) in refused {
        assert_refused(name, compiled(name, container()), message, None, &location);
    }
}

/// `XmlDumper` refuses the container; the direct reader refuses the
/// service with the same message.
#[test]
fn object_argument_is_refused_with_symfonys_message() {
    let mut cb = containers::container_with_object_argument();
    prepare(&mut cb);
    cb.compile(false).unwrap();
    let e = Graph::from_container_builder(&cb, &dumper_parameters()).unwrap_err();
    let dumped = XmlDumper::new(&mut cb).dump_xml().unwrap_err();
    assert_eq!(e.message, dumped.message());
    assert_eq!(
        e.message,
        "Unable to dump a service container if a parameter is an object or a resource, got \"stdClass\"."
    );
    assert_eq!(e.location, Location::Service("foo".into()));
}

/// The XML reader refuses `type="tagged_iterator"`; the direct reader
/// reads the services the iterator resolved to, as the XML reader reads
/// the recorder's untagged iterator.
#[test]
fn tagged_iterators_are_read_as_their_services() {
    let mut cb = containers::tagged_arguments(false);
    prepare(&mut cb);
    cb.compile(false).unwrap();
    let dp = dumper_parameters();
    let direct = Graph::from_container_builder(&cb, &dp).unwrap();
    let tagged = XmlDumper::new(&mut cb).dump_xml().unwrap();
    let e = Graph::from_xml(tagged.as_bytes(), &dp).unwrap_err();
    assert!(
        e.message.starts_with("unsupported attribute tag=")
            && matches!(e.location, Location::Line(_)),
        "{e}"
    );
    assert!(
        direct
            .services
            .iter()
            .flat_map(|d| &d.arguments)
            .any(|a| matches!(a, ArgDef::Iterator(items) if !items.is_empty())),
        "no resolved iterator in {direct:?}"
    );
}

/// Env placeholders, `%%`, whole-parameter arguments, keyed collections,
/// a method call cycle and a configurator.
#[test]
fn env_placeholders_and_percent_signs() {
    let mut cb = ContainerBuilder::new();
    cb.set_parameter("rate", "100%%").unwrap();
    cb.set_parameter("dsn", "%env(DATABASE_URL)%").unwrap();
    cb.set_parameter(
        "list",
        Value::Array(php_array!["a" => "x%%y", "b" => "%env(int:PORT)%"]),
    )
    .unwrap();
    let a = cb.register("a", Some("App\\A")).unwrap();
    a.add_argument("%rate% of %env(APP_SECRET)%")
        .add_argument("%dsn%")
        .add_argument("%list%")
        .add_argument("50%% off, %%d days")
        .add_argument(Value::Array(
            php_array!["k" => Value::from(Reference::new("b")), "n" => 3],
        ));
    let b = cb.register("b", Some("App\\B")).unwrap();
    b.add_method_call(
        "setA",
        PhpArray::list([Value::from(Reference::new("a"))]),
        false,
    )
    .unwrap();
    b.set_configurator(Value::Array(PhpArray::list([
        Value::from(Reference::new("a")),
        Value::from("configure"),
    ])));
    prepare(&mut cb);
    cb.compile(false).unwrap();
    let graph = Graph::from_container_builder(&cb, &dumper_parameters()).unwrap();
    let rate = graph.parameters.iter().find(|(k, _)| k == "rate").unwrap();
    assert_eq!(rate.1, Value::string("100%"));
    let dsn = graph.parameters.iter().find(|(k, _)| k == "dsn").unwrap();
    assert_eq!(dsn.1, Value::string("%env(DATABASE_URL)%"));
    assert_equal("env", both_ways("env", &mut cb), 3);
}

/// What the XML text loses and the direct reader keeps.
#[test]
fn scalars_the_xml_text_loses() {
    let mut cb = ContainerBuilder::new();
    let a = cb.register("a", Some("App\\A")).unwrap();
    a.set_public(true);
    a.add_argument(Value::Float(1.0)).add_argument("NULL");
    prepare(&mut cb);
    cb.compile(false).unwrap();
    let dp = dumper_parameters();
    let direct = Graph::from_container_builder(&cb, &dp).unwrap();
    let xml = XmlDumper::new(&mut cb).dump_xml().unwrap();
    let via_xml = Graph::from_xml(xml.as_bytes(), &dp).unwrap();
    let arguments = |g: &Graph| {
        g.services
            .iter()
            .find(|d| d.id == "a")
            .unwrap()
            .arguments
            .clone()
    };
    assert_eq!(
        arguments(&direct),
        [
            ArgDef::Value(Value::Float(1.0)),
            ArgDef::Value(Value::string("NULL"))
        ]
    );
    assert_eq!(
        arguments(&via_xml),
        [ArgDef::Value(Value::Int(1)), ArgDef::Value(Value::Null)]
    );
}

#[test]
fn an_uncompiled_builder_is_refused() {
    let cb = ContainerBuilder::new();
    let e = Graph::from_container_builder(&cb, &dumper_parameters()).unwrap_err();
    assert_eq!(e.location, Location::Unknown);
    assert!(
        e.message.starts_with("the container is not compiled"),
        "{e}"
    );
}
