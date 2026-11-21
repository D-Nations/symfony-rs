//! Replays tests/data/commands.json: debug:dotenv's and dotenv:dump's
//! exact output, and the .env.local.php the dump writes, recorded from
//! PHP 8.4.27 with symfony/dotenv and symfony/console v8.1.9
//! (oracle/recorders/dotenv_commands.php, `scripts/record.sh dotenv`).
//! Each case starts from empty globals, as the recorder's did, with
//! COLUMNS=80; the recorder's temporary directory in messages is this
//! test's.
//!
//! Upstream: none

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, Value};
use serde_json::Value as Json;
use symfony_console::{
    ConsoleError,
    application::Application,
    env::putenv,
    helper::{formatter_helper::FormatterHelper, helper_set::HelperSet},
    tester::command_tester::{CommandTester, ExecuteOptions},
};
use symfony_dotenv::{
    Dotenv, Globals,
    command::{debug_command::DebugCommand, dotenv_dump_command::DotenvDumpCommand},
};

const RECORDED_ROOT: &str = "/tmp/dotenv-cases";

fn root() -> String {
    format!(
        "{}/dotenv-cases",
        env!("CARGO_TARGET_TMPDIR").replace('\\', "/")
    )
}

fn set_up(name: &str, files: &Json) -> std::io::Result<String> {
    let dir = format!("{}/{name}", root());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    if let Some(files) = files.as_object() {
        for (file, content) in files {
            std::fs::write(
                format!("{dir}/{file}"),
                content.as_str().unwrap_or_default(),
            )?;
        }
    }
    Ok(dir)
}

fn dotenv_error(e: symfony_dotenv::Error) -> ConsoleError {
    ConsoleError::Other {
        class: e.class().to_owned(),
        message: String::from_utf8_lossy(&e.message()).into_owned(),
    }
}

fn shared(globals: Globals) -> Arc<Mutex<Globals>> {
    Arc::new(Mutex::new(globals))
}

fn helper_set() -> Arc<HelperSet> {
    Arc::new(HelperSet::new(vec![(None, Arc::new(FormatterHelper))]))
}

fn run_debug(case: &Json, dir: &str) -> Result<(i64, String), ConsoleError> {
    let env = case["env"].as_str().unwrap_or_default();
    let mut globals = Globals::new();
    let mut project_dir = dir.to_owned();
    match case["setup"].as_str().unwrap_or_default() {
        "boot" => {
            globals.server.insert("TEST_ENV_KEY", Value::string(env));
            Dotenv::with_keys("TEST_ENV_KEY", "APP_DEBUG")
                .boot_env(
                    &mut globals,
                    &format!("{dir}/.env"),
                    "dev",
                    &["test"],
                    false,
                )
                .map_err(dotenv_error)?;
        }
        "vars_only" => {
            globals
                .server
                .insert("SYMFONY_DOTENV_VARS", Value::string("FOO,LONG"));
            globals.server.insert("FOO", Value::string("server"));
            globals.server.insert("LONG", Value::string("x".repeat(40)));
        }
        "custom_path" => {
            let mut options = PhpArray::new();
            options.insert("dotenv_path", Value::string("nested/inner/.env"));
            globals
                .server
                .insert("APP_RUNTIME_OPTIONS", Value::Array(options));
            globals.server.insert("TEST_ENV_KEY", Value::string(env));
            Dotenv::with_keys("TEST_ENV_KEY", "APP_DEBUG")
                .boot_env(
                    &mut globals,
                    &format!("{dir}/.env"),
                    "dev",
                    &["test"],
                    false,
                )
                .map_err(dotenv_error)?;
            project_dir = root();
        }
        _ => {}
    }
    let mut command = DebugCommand::command(env, &project_dir, shared(globals))?;
    command.set_helper_set(helper_set());
    let mut tester = CommandTester::new(command);
    let mut input = PhpArray::new();
    if let Some(filter) = case["filter"].as_str() {
        input.insert("filter", Value::string(filter));
    }
    let status = tester.execute(
        input,
        ExecuteOptions {
            decorated: Some(false),
            ..ExecuteOptions::default()
        },
    )?;
    Ok((status, tester.get_display(true)?))
}

fn run_dump(case: &Json, dir: &str) -> Result<(i64, String), ConsoleError> {
    let mut application = Application::default();
    application.add_command(DotenvDumpCommand::command(
        dir,
        case["default_env"].as_str(),
        shared(Globals::new()),
    )?)?;
    let mut tester = CommandTester::new(application.find("dotenv:dump")?);
    let mut input = PhpArray::new();
    if let Some(entries) = case["input"].as_object() {
        for (key, value) in entries {
            let value = match value {
                Json::Bool(b) => Value::Bool(*b),
                other => Value::string(other.as_str().unwrap_or_default()),
            };
            input.insert(key.as_str(), value);
        }
    }
    let status = tester.execute(
        input,
        ExecuteOptions {
            decorated: Some(false),
            ..ExecuteOptions::default()
        },
    )?;
    Ok((status, tester.get_display(true)?))
}

/// The `*.local.php` file in the directory, as `[name, contents]`.
fn dumped(dir: &str) -> Json {
    let mut found: Vec<(String, String)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.ends_with(".local.php")
                .then(|| {
                    std::fs::read_to_string(entry.path())
                        .ok()
                        .map(|c| (name, c))
                })
                .flatten()
        })
        .collect();
    found.sort();
    match found.into_iter().next() {
        Some((name, contents)) => Json::from(vec![name, contents]),
        None => Json::Null,
    }
}

#[test]
fn commands_match_php() -> Result<(), Box<dyn std::error::Error>> {
    putenv("COLUMNS", Some("80"));
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/commands.json"))?;
    let mut failures = Vec::new();
    for case in &cases {
        let dir = set_up(case["project"].as_str().unwrap_or_default(), &case["files"])?;
        let is_dump = case["command"] == "dotenv:dump";
        let result = if is_dump {
            run_dump(case, &dir)
        } else {
            run_debug(case, &dir)
        };
        let ours = match result {
            Ok((status, display)) => {
                let mut out = format!("status {status}\n{display}");
                if is_dump {
                    out.push_str(&format!("\ndumped {}", dumped(&dir)));
                }
                out
            }
            Err(e) => format!(
                "throws {}\n{}",
                e.class(),
                e.message().replace(&root(), RECORDED_ROOT)
            ),
        };
        let expected = match case.get("throws") {
            Some(class) => format!(
                "throws {}\n{}",
                class.as_str().unwrap_or_default(),
                case["message"].as_str().unwrap_or_default()
            ),
            None => {
                let mut out = format!(
                    "status {}\n{}",
                    case["status"],
                    case["display"].as_str().unwrap_or_default()
                );
                if is_dump {
                    out.push_str(&format!("\ndumped {}", case["dumped"]));
                }
                out
            }
        };
        if ours != expected {
            failures.push(format!(
                "{} {} env={} filter={} setup={} input={}:\n--- ours\n{ours}\n--- PHP\n{expected}",
                case["command"],
                case["project"],
                case["env"],
                case["filter"],
                case["setup"],
                case["input"]
            ));
        }
    }
    putenv("COLUMNS", None);
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(3)].join("\n")
    );
    Ok(())
}
