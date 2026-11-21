//! LintCommandTest, with upstream's names, assertions and provider rows.
//! The test's `Foo::TEST` constant is defined in the parser's
//! environment, as PHP finds it among the loaded classes.
//!
//! Upstream: symfony/yaml Tests/Command/LintCommandTest.php
#![allow(non_snake_case)]

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use php_value::{PhpArray, Value};
use regex::Regex;
use symfony_console::{
    ConsoleError,
    application::{Application, SharedCommand},
    env::{getenv, putenv},
    output::output::VERBOSITY_VERBOSE,
    tester::{
        command_completion_tester::CommandCompletionTester,
        command_tester::{CommandTester, ExecuteOptions},
    },
};
use symfony_yaml::{Environment, command::lint_command::LintCommand};

type R = Result<(), ConsoleError>;

/// `setUp()` / `tearDown()`: files in sys_get_temp_dir()/framework-yml-lint-test.
struct Files(Vec<PathBuf>);

impl Files {
    fn new() -> Self {
        let _ = std::fs::create_dir_all(Self::dir());
        Self(Vec::new())
    }

    fn dir() -> PathBuf {
        std::env::temp_dir().join("framework-yml-lint-test")
    }

    /// `createFile($content)`: tempnam(), a new name each time.
    fn create_file(&mut self, content: &str) -> String {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = Self::dir().join(format!("sf-{}-{n}", std::process::id()));
        std::fs::write(&path, content).expect("temporary file");
        self.0.push(path.clone());
        path.to_string_lossy().replace('\\', "/")
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        for file in &self.0 {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// `createCommand()`.
fn create_command() -> Result<SharedCommand, ConsoleError> {
    let mut environment = Environment::new();
    environment.constants.define(
        r"Symfony\Component\Yaml\Tests\Command\Foo::TEST",
        Value::string("foo"),
    );
    let mut application = Application::default();
    application.add_command(LintCommand::command(
        None,
        Arc::new(environment),
        None,
        None,
    )?)?;
    application.find("lint:yaml")
}

/// `createCommandTester()`.
fn create_command_tester() -> Result<CommandTester, ConsoleError> {
    Ok(CommandTester::new(create_command()?))
}

fn input(entries: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in entries {
        array.insert(key, value);
    }
    array
}

fn list(values: &[&str]) -> Value {
    Value::Array(PhpArray::list(values.iter().map(|v| Value::string(*v))))
}

fn verbose() -> ExecuteOptions {
    ExecuteOptions {
        verbosity: Some(VERBOSITY_VERBOSE),
        decorated: Some(false),
        interactive: None,
    }
}

fn undecorated() -> ExecuteOptions {
    ExecuteOptions {
        decorated: Some(false),
        ..ExecuteOptions::default()
    }
}

/// `assertStringMatchesFormat()` with `%A` and `%s`.
fn matches_format(format: &str, text: &str) -> bool {
    let mut pattern = String::from("(?s)^");
    let mut rest = format;
    while let Some(at) = rest.find('%') {
        pattern.push_str(&regex::escape(&rest[..at]));
        match rest.as_bytes().get(at + 1) {
            Some(b'A') => pattern.push_str(".*"),
            Some(b's') => pattern.push_str("[^\\r\\n]+"),
            _ => pattern.push_str(&regex::escape(&rest[at..at + 2])),
        }
        rest = &rest[at + 2..];
    }
    pattern.push_str(&regex::escape(rest));
    pattern.push('$');
    Regex::new(&pattern).is_ok_and(|r| r.is_match(text))
}

#[test]
fn testLintCorrectFile() -> R {
    let mut files = Files::new();
    let mut tester = create_command_tester()?;
    let filename = files.create_file("foo: bar");

    let ret = tester.execute(
        input(vec![("filename", Value::string(filename))]),
        verbose(),
    )?;

    assert_eq!(0, ret, "Returns 0 in case of success");
    assert!(tester.get_display(false)?.trim().starts_with("// OK in "));
    Ok(())
}

#[test]
fn testLintCorrectFiles() -> R {
    let mut files = Files::new();
    let mut tester = create_command_tester()?;
    let filename1 = files.create_file("foo: bar");
    let filename2 = files.create_file("bar: baz");

    let ret = tester.execute(
        input(vec![("filename", list(&[&filename1, &filename2]))]),
        verbose(),
    )?;

    assert_eq!(0, ret, "Returns 0 in case of success");
    assert!(tester.get_display(false)?.trim().starts_with("// OK in "));
    Ok(())
}

#[test]
fn testLintIncorrectFile() -> R {
    let mut files = Files::new();
    let incorrect_content = "\nfoo:\nbar";
    let mut tester = create_command_tester()?;
    let filename = files.create_file(incorrect_content);

    let ret = tester.execute(
        input(vec![("filename", Value::string(filename))]),
        undecorated(),
    )?;

    assert_eq!(1, ret, "Returns 1 in case of error");
    assert!(
        tester
            .get_display(false)?
            .trim()
            .contains("Unable to parse at line 3 (near \"bar\").")
    );
    Ok(())
}

#[test]
fn testLintIncorrectFileWithGithubFormat() -> R {
    let mut files = Files::new();
    let incorrect_content = "foo:\nbar";
    let mut tester = create_command_tester()?;
    let filename = files.create_file(incorrect_content);

    tester.execute(
        input(vec![
            ("filename", Value::string(filename)),
            ("--format", Value::string("github")),
        ]),
        undecorated(),
    )?;

    assert_eq!(1, tester.get_status_code()?, "Returns 1 in case of error");
    assert!(matches_format(
        "%A::error file=%s,line=2,col=0::Unable to parse at line 2 (near \"bar\")%A",
        tester.get_display(false)?.trim()
    ));
    Ok(())
}

#[test]
fn testLintAutodetectsGithubActionEnvironment() -> R {
    let mut files = Files::new();
    let prev = getenv("GITHUB_ACTIONS");
    putenv("GITHUB_ACTIONS", None);

    putenv("GITHUB_ACTIONS", Some("1"));
    let incorrect_content = "foo:\nbar";
    let mut tester = create_command_tester()?;
    let filename = files.create_file(incorrect_content);

    let result = tester.execute(
        input(vec![("filename", Value::string(filename))]),
        undecorated(),
    );
    let display = tester.get_display(false);
    putenv("GITHUB_ACTIONS", prev.as_deref().filter(|p| !p.is_empty()));

    result?;
    assert!(matches_format(
        "%A::error file=%s,line=2,col=0::Unable to parse at line 2 (near \"bar\")%A",
        display?.trim()
    ));
    Ok(())
}

#[test]
fn testConstantAsKey() -> R {
    let mut files = Files::new();
    let yaml = "!php/const 'Symfony\\Component\\Yaml\\Tests\\Command\\Foo::TEST': bar";
    let filename = files.create_file(yaml);
    let ret = create_command_tester()?.execute(
        input(vec![("filename", Value::string(filename))]),
        verbose(),
    )?;
    assert_eq!(0, ret, "lint:yaml exits with code 0 in case of success");
    Ok(())
}

#[test]
fn testCustomTags() -> R {
    let mut files = Files::new();
    let filename = files.create_file("foo: !my_tag {foo: bar}");
    let ret = create_command_tester()?.execute(
        input(vec![
            ("filename", Value::string(filename)),
            ("--parse-tags", Value::Bool(true)),
        ]),
        verbose(),
    )?;
    assert_eq!(0, ret, "lint:yaml exits with code 0 in case of success");
    Ok(())
}

#[test]
fn testCustomTagsError() -> R {
    let mut files = Files::new();
    let filename = files.create_file("foo: !my_tag {foo: bar}");
    let ret = create_command_tester()?.execute(
        input(vec![("filename", Value::string(filename))]),
        verbose(),
    )?;
    assert_eq!(1, ret, "lint:yaml exits with code 1 in case of error");
    Ok(())
}

#[test]
fn testLintWithExclude() -> R {
    let mut files = Files::new();
    let mut tester = create_command_tester()?;
    let filename1 = files.create_file("foo: bar");
    let filename2 = files.create_file("bar: baz");

    let ret = tester.execute(
        input(vec![
            ("filename", list(&[&filename1, &filename2])),
            ("--exclude", list(&[&filename1])),
        ]),
        verbose(),
    )?;
    assert_eq!(0, ret, "lint:yaml exits with code 0 in case of success");
    assert!(
        tester
            .get_display(false)?
            .trim()
            .contains("All 1 YAML files contain valid syntax.")
    );
    Ok(())
}

#[test]
fn testLintFileNotReadable() -> R {
    let mut files = Files::new();
    let mut tester = create_command_tester()?;
    let filename = files.create_file("");
    std::fs::remove_file(&filename).expect("unlink");

    let error = tester
        .execute(
            input(vec![("filename", Value::string(filename))]),
            undecorated(),
        )
        .expect_err("RuntimeException");
    assert!(error.is_runtime());
    Ok(())
}

fn provide_completion_suggestions() -> Vec<(&'static str, Vec<&'static str>, Vec<&'static str>)> {
    vec![(
        "option",
        vec!["--format", ""],
        vec!["txt", "json", "github"],
    )]
}

#[test]
fn testComplete() -> R {
    for (name, input, expected_suggestions) in provide_completion_suggestions() {
        let tester = CommandCompletionTester::new(create_command()?);
        assert_eq!(expected_suggestions, tester.complete(&input)?, "{name}");
    }
    Ok(())
}
