//! `DebugCommandTest`, with upstream's names and assertions. Each test has
//! its own globals, as `#[RunInSeparateProcess]` gives upstream's (the
//! others share one process; none depends on what an earlier one left).
//! The scenarios are upstream's fixtures, copied to tests/fixtures/Command.
//!
//! Upstream: symfony/dotenv Tests/Command/DebugCommandTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, Value};
use symfony_console::{
    ConsoleError,
    application::Application,
    helper::{formatter_helper::FormatterHelper, helper_set::HelperSet},
    tester::{
        command_completion_tester::CommandCompletionTester,
        command_tester::{CommandTester, ExecuteOptions},
    },
};
use symfony_dotenv::{
    Dotenv, Globals,
    command::{SharedGlobals, debug_command::DebugCommand},
};

type R = Result<(), ConsoleError>;

/// `__DIR__.'/Fixtures'`.
fn fixtures() -> String {
    format!(
        "{}/tests/fixtures/Command",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    )
}

fn shared(globals: Globals) -> SharedGlobals {
    Arc::new(Mutex::new(globals))
}

fn helper_set() -> Arc<HelperSet> {
    Arc::new(HelperSet::new(vec![(None, Arc::new(FormatterHelper))]))
}

fn boot_env(globals: &mut Globals, path: &str) {
    Dotenv::with_keys("TEST_ENV_KEY", "APP_DEBUG")
        .boot_env(globals, path, "dev", &["test"], false)
        .expect("bootEnv");
}

/// `executeCommand($projectDirectory, $env, $input, $dotenvPath)`.
fn execute_command(
    project_directory: &str,
    env: &str,
    input: Vec<(&str, &str)>,
    dotenv_path: Option<&str>,
) -> Result<String, ConsoleError> {
    let mut globals = Globals::new();
    match dotenv_path {
        None => {
            globals.server.remove(&"APP_RUNTIME_OPTIONS".into());
        }
        Some(path) => {
            let value = path
                .strip_prefix(&format!("{project_directory}/"))
                .unwrap_or(path);
            let mut options = PhpArray::new();
            options.insert("dotenv_path", Value::string(value));
            globals
                .server
                .insert("APP_RUNTIME_OPTIONS", Value::Array(options));
        }
    }

    globals.server.insert("TEST_ENV_KEY", Value::string(env));
    let default_path = format!("{project_directory}/.env");
    boot_env(&mut globals, dotenv_path.unwrap_or(&default_path));

    let mut command = DebugCommand::command(env, project_directory, shared(globals))?;
    command.set_helper_set(helper_set());
    let mut tester = CommandTester::new(command);
    let mut parameters = PhpArray::new();
    for (key, value) in input {
        parameters.insert(key, Value::string(value));
    }
    tester.execute(parameters, ExecuteOptions::default())?;
    tester.get_display(false)
}

/// `assertStringMatchesFormat()` with `%a` (one or more of anything) and
/// `%S` (any characters but a newline, possibly none).
fn matches_format(format: &str, text: &str) -> bool {
    fn go(f: &[u8], t: &[u8]) -> bool {
        match f {
            [] => t.is_empty(),
            [b'%', b'a', rest @ ..] => (1..=t.len()).any(|n| go(rest, &t[n..])),
            [b'%', b'S', rest @ ..] => {
                let line = t.iter().take_while(|&&b| b != b'\n').count();
                (0..=line).any(|n| go(rest, &t[n..]))
            }
            [c, rest @ ..] => t.first() == Some(c) && go(rest, &t[1..]),
        }
    }
    go(format.as_bytes(), text.as_bytes())
}

#[test]
fn testErrorOnUninitializedDotenv() -> R {
    let mut command = DebugCommand::command(
        "dev",
        &format!("{}/Scenario1", fixtures()),
        shared(Globals::new()),
    )?;
    command.set_helper_set(helper_set());
    let mut tester = CommandTester::new(command);
    tester.execute(PhpArray::new(), ExecuteOptions::default())?;
    let output = tester.get_display(false)?;
    assert!(output.contains("[ERROR] Dotenv component is not initialized"));
    Ok(())
}

#[test]
fn testEmptyDotEnvVarsList() -> R {
    let mut globals = Globals::new();
    globals
        .server
        .insert("SYMFONY_DOTENV_VARS", Value::string(""));

    let mut command =
        DebugCommand::command("dev", &format!("{}/Scenario1", fixtures()), shared(globals))?;
    command.set_helper_set(helper_set());
    let mut tester = CommandTester::new(command);
    tester.execute(PhpArray::new(), ExecuteOptions::default())?;
    let expected_format = "%a
 ---------- ------- ------------ ------%S
  Variable   Value   .env.local   .env%S
 ---------- ------- ------------ ------%S
  FOO                baz          bar%S
  TEST123            n/a          true%S
 ---------- ------- ------------ ------%S

 // Note that values might be different between web and CLI.%S
%a";
    let display = tester.get_display(false)?;
    assert!(matches_format(expected_format, &display), "{display}");
    Ok(())
}

#[test]
fn testScenario1InDevEnv() -> R {
    let output = execute_command(&format!("{}/Scenario1", fixtures()), "dev", vec![], None)?;

    // Scanned Files
    assert!(output.contains("⨯ .env.local.php"));
    assert!(output.contains("⨯ .env.dev.local"));
    assert!(output.contains("⨯ .env.dev"));
    assert!(output.contains("✓ .env.local"));
    assert!(output.contains("✓ .env\n"));

    // Skipped Files
    assert!(!output.contains(".env.prod"));
    assert!(!output.contains(".env.test"));
    assert!(!output.contains(".env.dist"));

    // Variables
    assert!(output.contains("Variable   Value   .env.local   .env"));
    assert!(output.contains("FOO        baz     baz          bar"));
    assert!(output.contains("TEST123    true    n/a          true"));
    Ok(())
}

#[test]
fn testScenario1InTestEnv() -> R {
    let output = execute_command(&format!("{}/Scenario1", fixtures()), "test", vec![], None)?;

    // Scanned Files
    assert!(output.contains("⨯ .env.local.php"));
    assert!(output.contains("⨯ .env.test.local"));
    assert!(output.contains("✓ .env.test"));
    assert!(output.contains("✓ .env\n"));

    // Skipped Files
    assert!(!output.contains(".env.prod"));
    assert!(!output.contains(".env.dev"));
    assert!(!output.contains(".env.dist"));

    // Variables
    assert!(output.contains("Variable   Value   .env.test   .env"));
    assert!(output.contains("FOO        bar     n/a         bar"));
    assert!(output.contains("TEST123    false   false       true"));
    Ok(())
}

#[test]
fn testScenario1InProdEnv() -> R {
    let output = execute_command(&format!("{}/Scenario1", fixtures()), "prod", vec![], None)?;

    // Scanned Files
    assert!(output.contains("⨯ .env.local.php"));
    assert!(output.contains("✓ .env.prod.local"));
    assert!(output.contains("⨯ .env.prod"));
    assert!(output.contains("✓ .env.local"));
    assert!(output.contains("✓ .env\n"));

    // Skipped Files
    assert!(!output.contains(".env.dev"));
    assert!(!output.contains(".env.test"));
    assert!(!output.contains(".env.dist"));

    // Variables
    assert!(output.contains("Variable   Value   .env.prod.local   .env.local   .env"));
    assert!(output.contains("FOO        baz     n/a               baz          bar"));
    assert!(output.contains("HELLO      world   world             n/a          n/a"));
    assert!(output.contains("TEST123    true    n/a               n/a          true"));
    Ok(())
}

fn assert_scenario2(output: &str, prefix: &str) {
    // Scanned Files
    assert!(output.contains(&format!("✓ {prefix}.env.local.php")));
    assert!(output.contains(&format!("⨯ {prefix}.env.prod.local")));
    assert!(output.contains(&format!("✓ {prefix}.env.prod")));
    assert!(output.contains(&format!("⨯ {prefix}.env.local")));
    assert!(output.contains(&format!("✓ {prefix}.env.dist")));

    // Skipped Files
    assert!(!output.contains(".env\n"));
    assert!(!output.contains(".env.dev"));
    assert!(!output.contains(".env.test"));
}

#[test]
fn testScenario2InProdEnv() -> R {
    let output = execute_command(&format!("{}/Scenario2", fixtures()), "prod", vec![], None)?;
    assert_scenario2(&output, "");

    // Variables
    assert!(output.contains("Variable   Value   .env.local.php   .env.prod   .env.dist"));
    assert!(output.contains("FOO        BaR     BaR              BaR         n/a"));
    assert!(output.contains("TEST       1234    1234             1234        0000"));
    Ok(())
}

#[test]
fn testScenario2WithCustomPath() -> R {
    let path = format!("{}/Scenario2/.env", fixtures());
    let output = execute_command(&fixtures(), "prod", vec![], Some(&path))?;
    assert_scenario2(&output, "Scenario2/");

    // Variables
    assert!(output.contains(
        "Variable   Value   Scenario2/.env.local.php   Scenario2/.env.prod   Scenario2/.env.dist"
    ));
    assert!(
        output.contains("FOO        BaR     BaR                        BaR                   n/a")
    );
    assert!(
        output.contains("TEST       1234    1234                       1234                  0000")
    );
    Ok(())
}

#[test]
fn testWarningOnEnvAndEnvDistFile() -> R {
    let output = execute_command(&format!("{}/Scenario3", fixtures()), "dev", vec![], None)?;

    // Warning
    assert!(output.contains("[WARNING] The file .env.dist gets skipped"));
    Ok(())
}

#[test]
fn testWarningOnEnvAndEnvDistFileWithCustomPath() -> R {
    let path = format!("{}/Scenario3/.env", fixtures());
    let output = execute_command(&fixtures(), "dev", vec![], Some(&path))?;

    // Warning
    assert!(output.contains("[WARNING] The file Scenario3/.env.dist gets skipped"));
    Ok(())
}

#[test]
fn testWarningOnPhpEnvFile() -> R {
    let output = execute_command(&format!("{}/Scenario2", fixtures()), "prod", vec![], None)?;

    // Warning
    assert!(output.contains("[WARNING] Due to existing dump file (.env.local.php)"));
    Ok(())
}

#[test]
fn testWarningOnPhpEnvFileWithCustomPath() -> R {
    let path = format!("{}/Scenario2/.env", fixtures());
    let output = execute_command(&fixtures(), "prod", vec![], Some(&path))?;

    // Warning
    assert!(output.contains("[WARNING] Due to existing dump file (Scenario2/.env.local.php)"));
    Ok(())
}

#[test]
fn testScenario1InDevEnvWithNameFilter() -> R {
    let output = execute_command(
        &format!("{}/Scenario1", fixtures()),
        "dev",
        vec![("filter", "FoO")],
        None,
    )?;

    // Scanned Files
    assert!(output.contains("⨯ .env.local.php"));
    assert!(output.contains("⨯ .env.dev.local"));
    assert!(output.contains("⨯ .env.dev"));
    assert!(output.contains("✓ .env.local"));
    assert!(output.contains("✓ .env\n"));

    // Skipped Files
    assert!(!output.contains(".env.prod"));
    assert!(!output.contains(".env.test"));
    assert!(!output.contains(".env.dist"));

    // Variables
    assert!(output.contains("Variable   Value   .env.local   .env"));
    assert!(output.contains("FOO        baz     baz          bar"));
    assert!(!output.contains("TEST123    true    n/a          true"));
    Ok(())
}

#[test]
fn testScenario1InProdEnvWithMissingNameFilter() -> R {
    let output = execute_command(
        &format!("{}/Scenario1", fixtures()),
        "prod",
        vec![("filter", "unknown")],
        None,
    )?;

    // Scanned Files
    assert!(output.contains("⨯ .env.local.php"));
    assert!(output.contains("✓ .env.prod.local"));
    assert!(output.contains("⨯ .env.prod"));
    assert!(output.contains("✓ .env.local"));
    assert!(output.contains("✓ .env\n"));

    // Skipped Files
    assert!(!output.contains(".env.dev"));
    assert!(!output.contains(".env.test"));
    assert!(!output.contains(".env.dist"));

    // Variables
    assert!(output.contains("[WARNING] No variables match the given filter \"unknown\"."));
    assert!(!output.contains("Variable   Value   .env.prod.local   .env.local   .env"));
    assert!(!output.contains("FOO        baz     n/a               baz          bar"));
    assert!(!output.contains("HELLO      world   world             n/a          n/a"));
    assert!(!output.contains("TEST123    true    n/a               n/a          true"));
    Ok(())
}

#[test]
fn testScenario2InProdEnvWithNameFilterPrefix() -> R {
    let output = execute_command(
        &format!("{}/Scenario2", fixtures()),
        "prod",
        vec![("filter", "tes")],
        None,
    )?;
    assert_scenario2(&output, "");

    // Variables
    assert!(output.contains("Variable   Value   .env.local.php   .env.prod   .env.dist"));
    assert!(!output.contains("FOO        BaR     BaR              BaR         n/a"));
    assert!(output.contains("TEST       1234    1234             1234        0000"));
    Ok(())
}

#[test]
fn testCompletion() -> R {
    let env = "prod";
    let project_directory = format!("{}/Scenario2", fixtures());

    let mut globals = Globals::new();
    globals.server.insert("TEST_ENV_KEY", Value::string(env));
    boot_env(&mut globals, &format!("{project_directory}/.env"));

    let command = DebugCommand::command(env, &project_directory, shared(globals))?;
    let mut application = Application::default();
    application.add_command(command)?;
    let tester = CommandCompletionTester::new(application.get("debug:dotenv")?);
    assert_eq!(vec!["FOO", "TEST"], tester.complete(&[""])?);
    Ok(())
}
