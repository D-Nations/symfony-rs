//! Replays `tests/data/lint_command.json`: lint:yaml's exact output and exit
//! code recorded from PHP 8.4.27 with symfony/yaml and symfony/console
//! v8.1.9 (`oracle/recorders/yaml_lint.php`, `scripts/record.sh yaml`). The
//! files are written to the same relative directory the recorder used,
//! from this crate's directory, with COLUMNS=80 as there.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpArray, Value};
use serde_json::Value as Json;
use symfony_console::{
    application::Application,
    env::putenv,
    tester::command_tester::{CommandTester, ExecuteOptions},
};
use symfony_yaml::{Environment, command::lint_command::LintCommand};

const BASE: &str = "../../target/yl";

const CONTENTS: [(&str, &str); 11] = [
    ("valid", "foo: bar\n"),
    ("empty", ""),
    ("incorrect", "\nfoo:\nbar"),
    ("tabs", "foo:\n\tbar: baz\n"),
    ("custom_tag", "foo: !my_tag {foo: bar}\n"),
    ("duplicate_key", "foo: 1\nfoo: 2\n"),
    ("unterminated", "foo: \"bar\n"),
    ("unicode", "clé: [é, ü\n"),
    ("unknown_constant", "foo: !php/const NOT_A_CONSTANT\n"),
    ("slash_path", "a/b: {c: d\n"),
    ("sequence_in_mapping", "foo: bar\n- baz\n"),
];

fn write_files() -> std::io::Result<()> {
    std::fs::create_dir_all(format!("{BASE}/dir"))?;
    for (name, content) in CONTENTS {
        std::fs::write(format!("{BASE}/{name}.yaml"), content)?;
    }
    std::fs::write(format!("{BASE}/dir/only.yml"), "a: b\n")?;
    std::fs::write(format!("{BASE}/dir/skipped.txt"), "not: [yaml\n")
}

fn value(json: &Json) -> Value {
    match json {
        Json::Bool(b) => Value::Bool(*b),
        Json::String(s) => Value::string(s.replace("BASE", BASE)),
        Json::Array(items) => Value::Array(PhpArray::list(items.iter().map(value))),
        _ => Value::Null,
    }
}

#[test]
fn lint_output_matches_php() -> Result<(), Box<dyn std::error::Error>> {
    write_files()?;
    putenv("GITHUB_ACTIONS", None);
    putenv("COLUMNS", Some("80"));
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/lint_command.json"))?;
    let mut failures = Vec::new();
    for case in &cases {
        let mut input = PhpArray::new();
        for (key, v) in case["input"].as_object().ok_or("input")? {
            input.insert(key.as_str(), value(v));
        }
        let mut application = Application::default();
        application.add_command(LintCommand::command(
            None,
            Arc::new(Environment::new()),
            None,
            None,
        )?)?;
        let mut tester = CommandTester::new(application.find("lint:yaml")?);
        let options = ExecuteOptions {
            verbosity: case["verbosity"].as_i64(),
            decorated: Some(false),
            interactive: None,
        };
        let ours = match tester.execute(input, options) {
            Ok(status) => format!("status {status}\n{}", tester.get_display(true)?),
            Err(e) => format!("throws {}\n{}", e.class(), e.message()),
        };
        let expected = match case.get("throws") {
            Some(class) => format!(
                "throws {}\n{}",
                class.as_str().unwrap_or_default(),
                case["message"].as_str().unwrap_or_default()
            ),
            None => format!(
                "status {}\n{}",
                case["status"],
                case["display"].as_str().unwrap_or_default()
            ),
        };
        if ours != expected {
            failures.push(format!(
                "{} (verbosity {}):\n--- ours\n{ours}\n--- PHP\n{expected}",
                case["input"], case["verbosity"]
            ));
        }
    }
    putenv("COLUMNS", None);
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(4)].join("\n")
    );
    Ok(())
}
