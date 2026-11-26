//! What the emitter refuses at build time, and the scope rules, on small
//! container files (the DI trial's tests), and the same refusals from a
//! compiled `ContainerBuilder`, located by service id.
//!
//! Symfony's compiler passes already refuse a missing service or a
//! constructor cycle, so the builder cases change the compiled
//! container's definitions (shared objects) after `compile()`: the
//! emitter checks the graph again whatever its source.
//!
//! Upstream: none
#![allow(clippy::unwrap_used)]

use php_value::object::StringableObject;
use php_value::{PhpArray, Value};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior, Reference};
use symfony_di_emitter::{Config, DumperParameters, Error, Graph, Location, compile, compile_xml};

fn file(services: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container xmlns=\"http://symfony.com/schema/dic/services\">\n  <services>\n{services}\n  </services>\n</container>\n"
    )
}

fn refusal(services: &str) -> Error {
    match compile_xml(file(services).as_bytes(), &Config::default()) {
        Ok(_) => panic!("compiled"),
        Err(e) => e,
    }
}

#[test]
fn missing_service_is_a_build_error() {
    let e = refusal(
        r#"    <service id="a" class="A">
      <argument type="service" id="missing"/>
    </service>"#,
    );
    assert_eq!(
        e.message,
        "The service \"a\" has a dependency on a non-existent service \"missing\"."
    );
    assert_eq!(e.location, Location::Line(5));
}

#[test]
fn missing_optional_service_is_null() {
    let out = compile_xml(
        file(
            r#"    <service id="a" class="A">
      <argument type="service" id="missing" on-invalid="null"/>
    </service>"#,
        )
        .as_bytes(),
        &Config::default(),
    )
    .expect("compiles");
    assert_eq!(out.report.missing_optional, ["a -> missing"]);
}

#[test]
fn constructor_cycle_is_refused() {
    let e = refusal(
        r#"    <service id="a" class="A">
      <argument type="service" id="b"/>
    </service>
    <service id="b" class="B">
      <argument type="service" id="a"/>
    </service>"#,
    );
    assert_eq!(
        e.message,
        "Circular reference detected for service \"a\", path: \"a -> b -> a\"."
    );
}

#[test]
fn cycle_through_a_method_call_is_allowed() {
    let out = compile_xml(
        file(
            r#"    <service id="a" class="A">
      <call method="setB">
        <argument type="service" id="b"/>
      </call>
    </service>
    <service id="b" class="B">
      <argument type="service" id="a"/>
    </service>"#,
        )
        .as_bytes(),
        &Config::default(),
    )
    .expect("compiles");
    assert_eq!(out.report.call_cycles, [vec!["a", "b", "a"]]);
}

#[test]
fn unsupported_input_is_refused_with_its_line() {
    let e = refusal(
        r#"    <service id="a" class="A">
      <argument type="expression">service('b')</argument>
    </service>"#,
    );
    assert_eq!(e.message, "unsupported argument type=\"expression\"");
    assert_eq!(e.location, Location::Line(5));
    let e = refusal(r#"    <service id="a" class="A" decorates="b"/>"#);
    assert_eq!(
        e.message,
        "unsupported attribute decorates=\"b\" on <service>"
    );
}

#[test]
fn reaching_a_request_scoped_service_makes_a_service_request_scoped() {
    let services = r#"    <service id="stack" class="RequestStack">
      <tag name="kernel.reset" method="reset"/>
    </service>
    <service id="eager" class="A">
      <argument type="service" id="stack"/>
    </service>
    <service id="through_closure" class="B">
      <argument type="service_closure" id="eager"/>
    </service>
    <service id="free" class="C">
      <argument>1</argument>
    </service>"#;
    let config = Config {
        request_scoped_tags: vec!["kernel.reset".into()],
        request_scoped_ids: Vec::new(),
    };
    let out = compile_xml(file(services).as_bytes(), &config).expect("compiles");
    assert_eq!(out.report.request_seeds, 1);
    assert_eq!(out.report.request_scoped, 3);
    assert_eq!(out.report.app_scoped, 1);
    assert!(out.code.contains("pub fn free(&self)"));
}

/// A compiled container with public services `a` (class `A`) and `b`
/// (class `B`), changed by `edit` after compilation.
fn builder_refusal(edit: impl FnOnce(&ContainerBuilder)) -> Error {
    let mut cb = ContainerBuilder::new();
    for (id, class) in [("a", "A"), ("b", "B")] {
        cb.register(id, Some(class)).unwrap().set_public(true);
    }
    cb.compile(false).unwrap();
    edit(&cb);
    let refused = Graph::from_container_builder(&cb, &DumperParameters::from_seed(b""))
        .and_then(|graph| compile(&graph, &Config::default()));
    match refused {
        Ok(_) => panic!("compiled"),
        Err(e) => e,
    }
}

fn reference(id: &str) -> Value {
    Value::from(Reference::new(id))
}

#[test]
fn builder_missing_service_is_a_build_error() {
    let e = builder_refusal(|cb| {
        cb.get_definition("a")
            .unwrap()
            .add_argument(reference("missing"));
    });
    assert_eq!(
        e.message,
        "The service \"a\" has a dependency on a non-existent service \"missing\"."
    );
    assert_eq!(e.location, Location::Service("a".into()));
}

#[test]
fn builder_missing_optional_service_is_null() {
    let mut cb = ContainerBuilder::new();
    cb.register("a", Some("A")).unwrap().set_public(true);
    cb.compile(false).unwrap();
    cb.get_definition("a")
        .unwrap()
        .add_argument(Reference::with_behavior("missing", InvalidBehavior::Null));
    let graph = Graph::from_container_builder(&cb, &DumperParameters::from_seed(b"")).unwrap();
    let out = compile(&graph, &Config::default()).unwrap();
    assert_eq!(out.report.missing_optional, ["a -> missing"]);
}

#[test]
fn builder_constructor_cycle_is_refused() {
    let e = builder_refusal(|cb| {
        cb.get_definition("a").unwrap().add_argument(reference("b"));
        cb.get_definition("b").unwrap().add_argument(reference("a"));
    });
    assert_eq!(
        e.message,
        "Circular reference detected for service \"a\", path: \"a -> b -> a\"."
    );
    assert_eq!(e.location, Location::Service("a".into()));
}

#[test]
fn builder_cycle_through_a_method_call_is_allowed() {
    let mut cb = ContainerBuilder::new();
    let a = cb.register("a", Some("A")).unwrap();
    a.set_public(true);
    a.add_method_call("setB", PhpArray::list([reference("b")]), false)
        .unwrap();
    let b = cb.register("b", Some("B")).unwrap();
    b.set_public(true);
    b.add_argument(reference("a"));
    cb.compile(false).unwrap();
    let graph = Graph::from_container_builder(&cb, &DumperParameters::from_seed(b"")).unwrap();
    let out = compile(&graph, &Config::default()).unwrap();
    assert_eq!(out.report.call_cycles, [vec!["a", "b", "a"]]);
}

#[test]
fn builder_unsupported_input_is_refused_with_its_service() {
    let e = builder_refusal(|cb| {
        cb.get_definition("a")
            .unwrap()
            .add_argument(StringableObject::object(
                "Symfony\\Component\\ExpressionLanguage\\Expression",
                "service('b')",
            ));
    });
    assert_eq!(e.message, "unsupported argument type=\"expression\"");
    assert_eq!(e.location, Location::Service("a".into()));
    let e = builder_refusal(|cb| {
        cb.get_definition("a")
            .unwrap()
            .set_decorated_service(Some("b"), None, 0, InvalidBehavior::Exception)
            .unwrap();
    });
    assert_eq!(
        e.message,
        "unsupported attribute decorates=\"b\" on <service>"
    );
    assert_eq!(e.location, Location::Service("a".into()));
}

#[test]
fn builder_reaching_a_request_scoped_service_makes_a_service_request_scoped() {
    let mut cb = ContainerBuilder::new();
    let stack = cb.register("stack", Some("RequestStack")).unwrap();
    let mut reset = PhpArray::new();
    reset.insert("method", Value::from("reset"));
    stack.add_tag("kernel.reset", reset);
    cb.register("eager", Some("A"))
        .unwrap()
        .add_argument(reference("stack"));
    cb.register("through_closure", Some("B"))
        .unwrap()
        .add_argument(Value::from(
            symfony_dependency_injection::argument::ServiceClosureArgument::new(reference("eager")),
        ));
    cb.register("free", Some("C")).unwrap().add_argument(1_i64);
    for id in ["stack", "eager", "through_closure", "free"] {
        cb.get_definition(id).unwrap().set_public(true);
    }
    cb.compile(false).unwrap();
    let config = Config {
        request_scoped_tags: vec!["kernel.reset".into()],
        request_scoped_ids: Vec::new(),
    };
    let graph = Graph::from_container_builder(&cb, &DumperParameters::from_seed(b"")).unwrap();
    let out = compile(&graph, &config).unwrap();
    // service_container is a seed too (the XML case has none).
    assert_eq!(out.report.request_seeds, 2);
    assert_eq!(out.report.request_scoped, 4);
    assert_eq!(out.report.app_scoped, 1);
    assert!(out.code.contains("pub fn free(&self)"));
}
