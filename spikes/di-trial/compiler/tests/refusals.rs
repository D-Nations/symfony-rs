//! What the compiler refuses at build time, and the scope rules, on small
//! container files.
//!
//! Upstream: none

use di_trial_compiler::{Config, Error, compile};

fn file(services: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container xmlns=\"http://symfony.com/schema/dic/services\">\n  <services>\n{services}\n  </services>\n</container>\n"
    )
}

fn refusal(services: &str) -> Error {
    match compile(file(services).as_bytes(), &Config::default()) {
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
    assert_eq!(e.line, 5);
}

#[test]
fn missing_optional_service_is_null() {
    let out = compile(
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
    let out = compile(
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
    assert_eq!(e.line, 5);
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
    let out = compile(file(services).as_bytes(), &config).expect("compiles");
    assert_eq!(out.report.request_seeds, 1);
    assert_eq!(out.report.request_scoped, 3);
    assert_eq!(out.report.app_scoped, 1);
    assert!(out.code.contains("pub fn free(&self)"));
}
