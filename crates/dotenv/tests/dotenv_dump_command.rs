//! DotenvDumpCommandTest, with upstream's names and assertions. Each test
//! works in a directory of its own (upstream's share theirs, one at a
//! time), and `require` of the dumped file is `local_php::read()`.
//!
//! Upstream: symfony/dotenv Tests/Command/DotenvDumpCommandTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, Value};
use symfony_console::{
    ConsoleError,
    application::Application,
    tester::command_tester::{CommandTester, ExecuteOptions},
};
use symfony_dotenv::{
    Globals,
    command::{SharedGlobals, dotenv_dump_command::DotenvDumpCommand},
    local_php,
};

type R = Result<(), ConsoleError>;

/// `setUp()`: `.env` and `.env.local` in the test's directory.
struct Dir(String);

impl Dir {
    fn set_up(name: &str) -> Self {
        let dir = format!(
            "{}/dotenv-dump-{name}",
            env!("CARGO_TARGET_TMPDIR").replace('\\', "/")
        );
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("test directory");
        std::fs::write(format!("{dir}/.env"), "APP_ENV=dev\nAPP_SECRET=abc123").expect(".env");
        std::fs::write(format!("{dir}/.env.local"), "APP_LOCAL=yes").expect(".env.local");
        Self(dir)
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.0)
    }
}

impl Drop for Dir {
    /// `tearDown()`.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `createCommand()`.
fn create_command(dir: &Dir, globals: SharedGlobals) -> Result<CommandTester, ConsoleError> {
    let mut application = Application::default();
    application.add_command(DotenvDumpCommand::command(&dir.0, None, globals)?)?;
    Ok(CommandTester::new(application.find("dotenv:dump")?))
}

fn shared() -> SharedGlobals {
    Arc::new(Mutex::new(Globals::new()))
}

fn input(entries: &[(&str, Value)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in entries {
        array.insert(*key, value.clone());
    }
    array
}

fn strings(entries: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in entries {
        array.insert(*key, Value::string(*value));
    }
    array
}

/// `require $file`.
fn require(file: &str) -> Option<PhpArray> {
    local_php::read(file).ok().flatten()
}

#[test]
fn testExecute() -> R {
    let dir = Dir::set_up("execute");
    let mut command = create_command(&dir, shared())?;
    command.execute(
        input(&[("env", Value::string("test"))]),
        ExecuteOptions::default(),
    )?;

    assert!(std::path::Path::new(&dir.path(".env.local.php")).exists());

    let vars = require(&dir.path(".env.local.php"));
    assert_eq!(
        Some(strings(&[("APP_ENV", "test"), ("APP_SECRET", "abc123")])),
        vars
    );
    Ok(())
}

#[test]
fn testExecuteEmpty() -> R {
    let dir = Dir::set_up("execute-empty");
    let mut command = create_command(&dir, shared())?;
    command.execute(
        input(&[
            ("env", Value::string("test")),
            ("--empty", Value::Bool(true)),
        ]),
        ExecuteOptions::default(),
    )?;

    assert!(std::path::Path::new(&dir.path(".env.local.php")).exists());

    let vars = require(&dir.path(".env.local.php"));
    assert_eq!(Some(strings(&[("APP_ENV", "test")])), vars);
    Ok(())
}

#[test]
fn testExecuteTestEnvs() -> R {
    let dir = Dir::set_up("execute-test-envs");
    std::fs::write(
        dir.path("composer.json"),
        r#"{"extra":{"runtime":{"test_envs":[]}}}"#,
    )
    .expect("composer.json");

    let mut command = create_command(&dir, shared())?;
    command.execute(
        input(&[("env", Value::string("test"))]),
        ExecuteOptions::default(),
    )?;

    assert!(std::path::Path::new(&dir.path(".env.local.php")).exists());

    let vars = require(&dir.path(".env.local.php"));
    assert_eq!(
        Some(strings(&[
            ("APP_ENV", "test"),
            ("APP_SECRET", "abc123"),
            ("APP_LOCAL", "yes"),
        ])),
        vars
    );
    Ok(())
}

#[test]
fn testExecuteWithRuntimeOptionsDotenvPath() -> R {
    let dir = Dir::set_up("runtime-options");
    std::fs::write(dir.path(".env.path"), "APP_ENV=test\nAPP_SECRET=newpath123")
        .expect(".env.path");
    std::fs::write(dir.path(".env.path.local"), "LOCAL_PATH=yes").expect(".env.path.local");

    let globals = shared();
    if let Ok(mut g) = globals.lock() {
        g.server.insert(
            "APP_RUNTIME_OPTIONS",
            Value::Array(strings(&[("dotenv_path", ".env.path")])),
        );
    }

    let mut command = create_command(&dir, globals)?;
    command.execute(
        input(&[("env", Value::string("dev"))]),
        ExecuteOptions::default(),
    )?;

    assert!(std::path::Path::new(&dir.path(".env.path.local.php")).exists());

    let vars = require(&dir.path(".env.path.local.php"));
    assert_eq!(
        Some(strings(&[
            ("APP_ENV", "dev"),
            ("APP_SECRET", "newpath123"),
            ("LOCAL_PATH", "yes"),
        ])),
        vars
    );
    Ok(())
}
