//! Inputs that tell symfony/yaml's fix for GHSA-9frc-8383-795m (`ReDoS` in
//! `Parser::cleanup()`) from the code before it: on v8.0.11 the old
//! patterns hit PCRE's backtrack limit (a `TypeError`, or a `---` marker
//! left in place), on the pin they do not. The fix's own tests
//! (testParseHandlesLarge*) pass on both, so these pin the difference.
//! Expected values are the pin's answers on the oracle (PHP 8.4.27,
//! symfony/yaml v8.1.9); parity/advisories.tsv has the triage.
//!
//! Upstream: none

use php_value::Value;
use symfony_yaml::{Flags, Parser};

fn reserved_indicator(yaml: &str) -> String {
    format!(
        "The reserved indicator \"%\" cannot start a plain scalar; you need to quote the scalar at line 1 (near \"{yaml}\")."
    )
}

#[test]
fn cleanup_does_not_backtrack_on_long_yaml_headers() {
    for yaml in [
        format!("%YAML:{}", "1.".repeat(50000)),
        format!("%YAML 1{}x", ".".repeat(100_000)),
    ] {
        let error = Parser::new()
            .parse(yaml.as_bytes(), Flags::NONE)
            .expect_err("a ParseException, as on the pin");
        assert_eq!(
            reserved_indicator(&yaml),
            error.get_message().to_string_lossy(),
            "{}",
            &yaml[..20]
        );
    }
}

#[test]
fn cleanup_does_not_backtrack_on_long_comments_and_markers() {
    for yaml in [
        format!("#c\n#c\n#{}", " #".repeat(50000)),
        format!("--- {}", "# ".repeat(50000)),
    ] {
        let value = Parser::new()
            .parse(yaml.as_bytes(), Flags::NONE)
            .expect("parsed, as on the pin");
        assert_eq!(Value::Null, value, "{}", &yaml[..10]);
    }
}
