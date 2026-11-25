//! DotenvTest.php at symfony/dotenv 4ea87b35c75f7052309ec9735c0eb5c1fd7d2182.
//! Names and assertions are PHPUnit's. PHP's process globals (`$_ENV`,
//! `$_SERVER`, `putenv()`) are a fresh `Globals` per test, so upstream's
//! cleanup of them is only kept where a test resets between its cases.
//! The data-provider tests read the provider rows recorded from upstream
//! (`tests/data/dotenv_test.json`, `scripts/record.sh dotenv`). The rows and
//! tests upstream runs only off Windows (they run `$(...)` commands) are
//! skipped on Windows here too.
//!
//! testGetVariablesValueFromEnvFirst runs `php -r` in a command upstream;
//! PHP is not installed on hosts here (AGENTS.md), so that command reads
//! the same variable with `printenv`.
//!
//! Upstream: symfony/dotenv Tests/DotenvTest.php
#![allow(non_snake_case)]

use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use php_value::{Key, PhpArray, Value, php_array, vectors::decode};
use serde_json::Value as Json;
use symfony_dotenv::{Dotenv, Error, Globals};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Rows upstream's providers add only off Windows (at their end).
const UNIX_ONLY_PARSE_ROWS: usize = 6;
const UNIX_ONLY_FORMAT_ERROR_ROWS: usize = 1;

fn provider(test: &str) -> Vec<Vec<Value>> {
    let json: Json = serde_json::from_str(include_str!("data/dotenv_test.json"))
        .unwrap_or_else(|e| panic!("dotenv_test.json: {e}"));
    json[test]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no rows for {test}"))
        .iter()
        .map(|row| {
            row[1]
                .as_array()
                .map(|args| {
                    args.iter()
                        .map(|a| decode(a).unwrap_or(Value::Null))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// The rows of `test` this platform runs.
fn rows(test: &str, unix_only: usize) -> Vec<Vec<Value>> {
    let mut rows = provider(test);
    if cfg!(windows) {
        rows.truncate(rows.len() - unix_only);
    }
    rows
}

fn text(value: &Value) -> String {
    value
        .as_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn bytes(value: &Value) -> Vec<u8> {
    value.as_string().map(|s| s.to_vec()).unwrap_or_default()
}

/// `$array[$name]`, as a string.
fn get(array: &PhpArray, name: &str) -> Option<String> {
    array.get(&Key::from(name)).map(text)
}

fn set(array: &mut PhpArray, name: &str, value: &str) {
    array.insert(name, Value::from(value));
}

fn unset(array: &mut PhpArray, names: &[&str]) {
    for name in names {
        array.remove(&Key::from(*name));
    }
}

fn getenv(g: &Globals, name: &str) -> Option<String> {
    g.getenv(name.as_bytes())
        .map(|v| String::from_utf8_lossy(&v).into_owned())
}

fn putenv(g: &mut Globals, setting: &str) {
    g.putenv(setting.as_bytes());
}

/// `unset($_ENV[...], $_SERVER[...]); putenv(...)` for each name.
fn clear(g: &mut Globals, names: &[&str]) {
    unset(&mut g.env, names);
    unset(&mut g.server, names);
    for name in names {
        putenv(g, name);
    }
}

/// `sys_get_temp_dir().'/dotenv'`, one per test, removed afterwards.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "symfony-dotenv-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Self(dir)
    }

    /// `tempnam($tmpdir, 'sf-')`.
    fn tempnam(&self) -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = self
            .0
            .join(format!("sf-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        write(&path.to_string_lossy(), "");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `file_put_contents()`.
fn write(path: &str, contents: &str) {
    std::fs::write(path, contents).unwrap_or_else(|e| panic!("{path}: {e}"));
}

fn unlink(path: &str) {
    let _ = std::fs::remove_file(path);
}

/// PHPUnit's `assertStringMatchesFormat()` for `%s` (one or more
/// characters other than a line break).
fn matches_format(format: &[u8], actual: &[u8]) -> bool {
    match format.windows(2).position(|w| w == b"%s") {
        None => format == actual,
        Some(at) => {
            let (prefix, rest) = (&format[..at], &format[at + 2..]);
            actual.starts_with(prefix)
                && (prefix.len() + 1..=actual.len()).any(|end| {
                    !actual[prefix.len()..end]
                        .iter()
                        .any(|&b| b == b'\n' || b == b'\r')
                        && matches_format(rest, &actual[end..])
                })
        }
    }
}

#[test]
fn testParseWithFormatError() {
    for args in rows("testParseWithFormatError", UNIX_ONLY_FORMAT_ERROR_ROWS) {
        let g = Globals::new();
        let mut dotenv = Dotenv::new();

        match dotenv.parse(&g, &bytes(&args[0]), ".env") {
            Err(Error::Format(e)) => assert!(
                matches_format(&bytes(&args[1]), e.get_message()),
                "{:?} does not match {:?}",
                String::from_utf8_lossy(e.get_message()),
                text(&args[1])
            ),
            other => panic!(
                "Should throw a FormatException ({:?}): {other:?}",
                text(&args[0])
            ),
        }
    }
}

#[test]
fn testParse() -> TestResult {
    for args in rows("testParse", UNIX_ONLY_PARSE_ROWS) {
        // the provider's environment
        let mut g = Globals::new();
        putenv(&mut g, "LOCAL=local");
        set(&mut g.env, "LOCAL", "local");
        set(&mut g.env, "REMOTE", "remote");
        set(&mut g.server, "SERVERVAR", "servervar");

        let mut dotenv = Dotenv::new();
        let parsed = dotenv.parse(&g, &bytes(&args[0]), ".env")?;
        assert_eq!(args[1], Value::Array(parsed), "{:?}", text(&args[0]));
    }
    Ok(())
}

#[test]
fn testLoad() -> TestResult {
    let mut g = Globals::new();
    let tmpdir = TempDir::new();

    let path1 = tmpdir.tempnam();
    let path2 = tmpdir.tempnam();

    write(&path1, "FOO=BAR");
    write(&path2, "BAR=BAZ");

    Dotenv::new()
        .use_putenv(true)
        .load(&mut g, &[&path1, &path2])?;

    assert_eq!(Some("BAR".to_owned()), getenv(&g, "FOO"));
    assert_eq!(Some("BAZ".to_owned()), getenv(&g, "BAR"));
    Ok(())
}

#[test]
fn testLoadDoesNotReResolveAlreadyLoadedVars() -> TestResult {
    let mut g = Globals::new();
    let tmpdir = TempDir::new();

    let path1 = tmpdir.tempnam();
    let path2 = tmpdir.tempnam();

    write(&path1, "FOO='This$isokay'");
    write(&path2, "BAR='hello'");

    Dotenv::new().load(&mut g, &[&path1])?;
    assert_eq!(Some("This$isokay".to_owned()), get(&g.env, "FOO"));

    Dotenv::new().load(&mut g, &[&path2])?;
    assert_eq!(Some("This$isokay".to_owned()), get(&g.env, "FOO"));
    assert_eq!(Some("hello".to_owned()), get(&g.env, "BAR"));
    Ok(())
}

#[test]
fn testLoadDoesNotResolveExternalEnvVarsOnlyPresentInServer() -> TestResult {
    let mut g = Globals::new();
    set(&mut g.server, "FOO", "abc$def");

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();
    write(&path, "FOO=default\n");

    Dotenv::new().load_env(&mut g, &path, None, "prod", &["test"], false)?;
    assert_eq!(Some("abc$def".to_owned()), get(&g.env, "FOO"));
    assert_eq!(Some("abc$def".to_owned()), get(&g.server, "FOO"));
    Ok(())
}

#[test]
fn testLoadDoesNotTruncateExternalEnvVarReferencedFromDotenv() -> TestResult {
    for (in_env, in_server) in [(true, true), (false, true)] {
        let mut g = Globals::new();

        if in_env {
            set(&mut g.env, "EXT_VAR", "secret$word");
        }
        if in_server {
            set(&mut g.server, "EXT_VAR", "secret$word");
        }

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        write(&path, "INDIRECT=${EXT_VAR}\n");

        Dotenv::new().load(&mut g, &[&path])?;
        assert_eq!(Some("secret$word".to_owned()), get(&g.env, "INDIRECT"));
        assert_eq!(Some("secret$word".to_owned()), get(&g.server, "INDIRECT"));
    }
    Ok(())
}

#[test]
fn testLoadDoesNotTruncateExternalEnvVarOnlyPresentInGetenvOnSelfReference() -> TestResult {
    for use_putenv in [false, true] {
        let mut g = Globals::new();

        // Mimics `variables_order` lacking both "E" and "S": the OS-provided
        // env var is only reachable through getenv().
        putenv(&mut g, "EXT_VAR=secret$word");

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        write(&path, "EXT_VAR=${EXT_VAR}\n");

        Dotenv::new()
            .use_putenv(use_putenv)
            .load(&mut g, &[&path])?;
        assert_eq!(Some("secret$word".to_owned()), get(&g.env, "EXT_VAR"));
        assert_eq!(Some("secret$word".to_owned()), get(&g.server, "EXT_VAR"));
        if use_putenv {
            assert_eq!(Some("secret$word".to_owned()), getenv(&g, "EXT_VAR"));
        }
    }
    Ok(())
}

#[test]
fn testLoadResolvesSelfReferencingDefaultWhenNoExternalEnvVarExists() -> TestResult {
    for use_putenv in [false, true] {
        let mut g = Globals::new();

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        write(&path, "EXT_VAR=${EXT_VAR:-default}\n");

        Dotenv::new()
            .use_putenv(use_putenv)
            .load(&mut g, &[&path])?;
        assert_eq!(Some("default".to_owned()), get(&g.env, "EXT_VAR"));
        assert_eq!(Some("default".to_owned()), get(&g.server, "EXT_VAR"));
        if use_putenv {
            assert_eq!(Some("default".to_owned()), getenv(&g, "EXT_VAR"));
        }
    }
    Ok(())
}

#[test]
fn testLoadPrefersExternalEnvVarOnlyPresentInGetenvOverSelfReferencingDefault() -> TestResult {
    for use_putenv in [false, true] {
        let mut g = Globals::new();
        putenv(&mut g, "EXT_VAR=external");

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        write(&path, "EXT_VAR=${EXT_VAR:-default}\n");

        Dotenv::new()
            .use_putenv(use_putenv)
            .load(&mut g, &[&path])?;
        assert_eq!(Some("external".to_owned()), get(&g.env, "EXT_VAR"));
        assert_eq!(Some("external".to_owned()), get(&g.server, "EXT_VAR"));
    }
    Ok(())
}

#[test]
fn testLoadDoesNotExecuteShellSyntaxFromExternalEnvVarOnlyPresentInGetenv() -> TestResult {
    for use_putenv in [false, true] {
        let mut g = Globals::new();
        putenv(&mut g, "EXT_VAR=value$(id)");

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        write(&path, "EXT_VAR=${EXT_VAR}\n");

        Dotenv::new()
            .use_putenv(use_putenv)
            .load(&mut g, &[&path])?;
        assert_eq!(Some("value$(id)".to_owned()), get(&g.env, "EXT_VAR"));
        assert_eq!(Some("value$(id)".to_owned()), get(&g.server, "EXT_VAR"));
    }
    Ok(())
}

#[test]
fn testPreservesBackslashesOfExternalEnvVar() -> TestResult {
    for args in provider("testPreservesBackslashesOfExternalEnvVar") {
        let external = text(&args[0]);
        let only_in_getenv = args[1] == Value::Bool(true);
        let self_reference = args[2] == Value::Bool(true);
        let mut g = Globals::new();

        if only_in_getenv {
            putenv(&mut g, &format!("EXT_VAR={external}"));
        } else {
            set(&mut g.env, "EXT_VAR", &external);
            set(&mut g.server, "EXT_VAR", &external);
        }

        let tmpdir = TempDir::new();
        let path = tmpdir.tempnam();
        let name = if self_reference { "EXT_VAR" } else { "REF_VAR" };
        write(&path, &format!("{name}=pre${{EXT_VAR}}post\n"));

        let mut dotenv = Dotenv::new();
        if only_in_getenv {
            dotenv.load(&mut g, &[&path])?;
        } else {
            dotenv.overload(&mut g, &[&path])?;
        }
        let expected = Some(format!("pre{external}post"));
        assert_eq!(
            expected,
            get(&g.env, name),
            "{external:?} {only_in_getenv} {self_reference}"
        );
        assert_eq!(
            expected,
            get(&g.server, name),
            "{external:?} {only_in_getenv} {self_reference}"
        );
    }
    Ok(())
}

#[test]
fn testOverloadDoesNotExecuteShellSyntaxFromExternalEnvOnSelfReference() -> TestResult {
    let mut g = Globals::new();
    set(&mut g.env, "FOO", "value$(id)");
    set(&mut g.server, "FOO", "value$(id)");

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();
    write(&path, "FOO=${FOO:-default}\n");

    Dotenv::new().overload(&mut g, &[&path])?;
    assert_eq!(Some("value$(id)".to_owned()), get(&g.env, "FOO"));
    assert_eq!(Some("value$(id)".to_owned()), get(&g.server, "FOO"));
    Ok(())
}

#[test]
fn testResolveLoadedVarsClearsStateOnCircularReferenceException() -> TestResult {
    let mut g = Globals::new();
    set(&mut g.env, "A", "external");
    set(&mut g.server, "A", "external");

    let mut dotenv = Dotenv::new();

    let tmpdir = TempDir::new();
    let circular = tmpdir.tempnam();
    write(&circular, "A=${B}\nB=${A}x\n");
    let self_ref = tmpdir.tempnam();
    write(&self_ref, "A=${A:-default}\n");

    match dotenv.overload(&mut g, &[&circular]) {
        Err(Error::VariableCircularReference(_)) => {}
        other => panic!("A VariableCircularReferenceException should have been thrown: {other:?}"),
    }

    clear(&mut g, &["A", "B", "SYMFONY_DOTENV_VARS"]);

    dotenv.load(&mut g, &[&self_ref])?;
    assert_eq!(Some("default".to_owned()), get(&g.env, "A"));
    Ok(())
}

#[test]
fn testLoadEnv() -> TestResult {
    let mut g = Globals::new();
    let reset_context = |g: &mut Globals| {
        clear(g, &["SYMFONY_DOTENV_VARS", "FOO", "TEST_APP_ENV"]);

        set(&mut g.env, "EXISTING_KEY", "EXISTING_VALUE");
        set(&mut g.server, "EXISTING_KEY", "EXISTING_VALUE");
        putenv(g, "EXISTING_KEY=EXISTING_VALUE");
    };
    let load = |g: &mut Globals, path: &str, override_existing: bool| {
        Dotenv::new().use_putenv(true).load_env(
            g,
            path,
            Some("TEST_APP_ENV"),
            "dev",
            &["test"],
            override_existing,
        )
    };
    let s = |v: &str| Some(v.to_owned());

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    // .env
    write(&path, "FOO=BAR\nEXISTING_KEY=NEW_VALUE");

    reset_context(&mut g);
    load(&mut g, &path, false)?;
    assert_eq!(s("BAR"), getenv(&g, "FOO"));
    assert_eq!(s("dev"), getenv(&g, "TEST_APP_ENV"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    load(&mut g, &path, true)?;
    assert_eq!(s("BAR"), getenv(&g, "FOO"));
    assert_eq!(s("dev"), getenv(&g, "TEST_APP_ENV"));
    assert_eq!(s("NEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("NEW_VALUE"), get(&g.env, "EXISTING_KEY"));

    // .env.local
    write(
        &format!("{path}.local"),
        "FOO=localBAR\nEXISTING_KEY=localNEW_VALUE",
    );

    reset_context(&mut g);
    set(&mut g.server, "TEST_APP_ENV", "local");
    load(&mut g, &path, false)?;
    assert_eq!(s("localBAR"), getenv(&g, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    set(&mut g.server, "TEST_APP_ENV", "local");
    load(&mut g, &path, true)?;
    assert_eq!(s("localBAR"), getenv(&g, "FOO"));
    assert_eq!(s("localNEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("localNEW_VALUE"), get(&g.env, "EXISTING_KEY"));

    // special case for test
    reset_context(&mut g);
    set(&mut g.server, "TEST_APP_ENV", "test");
    load(&mut g, &path, false)?;
    assert_eq!(s("BAR"), getenv(&g, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    set(&mut g.server, "TEST_APP_ENV", "test");
    load(&mut g, &path, true)?;
    assert_eq!(s("BAR"), getenv(&g, "FOO"));
    assert_eq!(s("NEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("NEW_VALUE"), get(&g.env, "EXISTING_KEY"));

    // .env.dev
    write(
        &format!("{path}.dev"),
        "FOO=devBAR\nEXISTING_KEY=devNEW_VALUE",
    );

    reset_context(&mut g);
    load(&mut g, &path, false)?;
    assert_eq!(s("devBAR"), getenv(&g, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    load(&mut g, &path, true)?;
    assert_eq!(s("devBAR"), getenv(&g, "FOO"));
    assert_eq!(s("devNEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("devNEW_VALUE"), get(&g.env, "EXISTING_KEY"));

    // .env.dev.local
    write(
        &format!("{path}.dev.local"),
        "FOO=devlocalBAR\nEXISTING_KEY=devlocalNEW_VALUE",
    );

    reset_context(&mut g);
    load(&mut g, &path, false)?;
    assert_eq!(s("devlocalBAR"), getenv(&g, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    load(&mut g, &path, true)?;
    assert_eq!(s("devlocalBAR"), getenv(&g, "FOO"));
    assert_eq!(s("devlocalNEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("devlocalNEW_VALUE"), get(&g.env, "EXISTING_KEY"));
    unlink(&format!("{path}.local"));
    unlink(&format!("{path}.dev"));
    unlink(&format!("{path}.dev.local"));

    // .env.dist
    write(
        &format!("{path}.dist"),
        "FOO=distBAR\nEXISTING_KEY=distNEW_VALUE",
    );

    reset_context(&mut g);
    unlink(&path);
    load(&mut g, &path, false)?;
    assert_eq!(s("distBAR"), getenv(&g, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.env, "EXISTING_KEY"));

    reset_context(&mut g);
    load(&mut g, &path, true)?;
    assert_eq!(s("distBAR"), getenv(&g, "FOO"));
    assert_eq!(s("distNEW_VALUE"), getenv(&g, "EXISTING_KEY"));
    assert_eq!(s("distNEW_VALUE"), get(&g.env, "EXISTING_KEY"));
    Ok(())
}

fn load_test_env(g: &mut Globals, path: &str) -> Result<(), Error> {
    Dotenv::new()
        .use_putenv(true)
        .load_env(g, path, Some("TEST_APP_ENV"), "dev", &["test"], false)
}

#[test]
fn testLoadEnvResolvesVariablesFromOverriddenFiles() -> TestResult {
    let mut g = Globals::new();
    let reset_context = |g: &mut Globals| {
        clear(
            g,
            &[
                "SYMFONY_DOTENV_VARS",
                "REDIS_HOST",
                "LOCK_DSN",
                "HOST",
                "DSN",
                "FOO",
                "BAR",
                "DATABASE_URL",
                "TEST_APP_ENV",
            ],
        );
    };
    let s = |v: &str| Some(v.to_owned());

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();
    let local = format!("{path}.local");

    // .env defines REDIS_HOST and LOCK_DSN referencing it
    write(
        &path,
        "REDIS_HOST=localhost\nLOCK_DSN=redis://${REDIS_HOST}",
    );
    // .env.local overrides REDIS_HOST
    write(&local, "REDIS_HOST=aaa");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(s("aaa"), getenv(&g, "REDIS_HOST"));
    assert_eq!(s("redis://aaa"), getenv(&g, "LOCK_DSN"));

    // backslash + variable in double-quoted value must resolve correctly
    write(&path, "HOST=localhost\nDSN=\"path\\\\${HOST}\"");
    write(&local, "HOST=override");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(s("override"), getenv(&g, "HOST"));
    assert_eq!(s("path\\override"), getenv(&g, "DSN"));

    // single-quoted $ must stay literal and not be resolved
    write(&path, "BAR=hello\nFOO='$BAR'");
    write(&local, "BAR=world");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(s("$BAR"), getenv(&g, "FOO"));
    assert_eq!(s("world"), getenv(&g, "BAR"));

    // escaped $ in double-quoted value must stay literal during deferred resolution
    write(
        &path,
        "FOO=\"\\$2y\\$10\\$AAAAAAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBB\"",
    );
    write(&local, "BAR=dummy");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(
        s("$2y$10$AAAAAAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBB"),
        getenv(&g, "FOO")
    );

    // escaped $ in unquoted value must stay literal during deferred resolution
    write(
        &path,
        "FOO=\\$2y\\$10\\$AAAAAAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBB",
    );
    write(&local, "BAR=dummy");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(
        s("$2y$10$AAAAAAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBB"),
        getenv(&g, "FOO")
    );

    // double backslash in unquoted value without $ must be unescaped during deferred resolution
    write(
        &path,
        "DATABASE_URL=sqlsrv://user:pass@localhost\\\\SQLEXPRESS/db",
    );
    write(&local, "BAR=dummy");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(
        s("sqlsrv://user:pass@localhost\\SQLEXPRESS/db"),
        getenv(&g, "DATABASE_URL")
    );

    // double backslash + variable in cross-file resolution must not double-unescape
    write(&path, "HOST=localhost\nDSN=\"path\\\\\\\\:${HOST}\"");
    write(&local, "HOST=override");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(s("path\\\\:override"), getenv(&g, "DSN"));
    Ok(())
}

#[test]
fn testLoadMultiplePathsResolvesVariables() -> TestResult {
    let mut g = Globals::new();

    let tmpdir = TempDir::new();
    let path1 = tmpdir.tempnam();
    let path2 = tmpdir.tempnam();

    write(&path1, "HOST=localhost\nURL=http://${HOST}");
    write(&path2, "HOST=production");

    Dotenv::new()
        .use_putenv(true)
        .load(&mut g, &[&path1, &path2])?;

    assert_eq!(Some("production".to_owned()), getenv(&g, "HOST"));
    assert_eq!(Some("http://production".to_owned()), getenv(&g, "URL"));
    Ok(())
}

#[test]
fn testLoadEnvResolvesCommandsWithOverriddenVars() -> TestResult {
    if cfg!(windows) {
        // markTestSkipped: This test cannot be run on Windows.
        return Ok(());
    }

    let mut g = Globals::new();

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    write(&path, "HOST=original\nRESOLVED=\"$(echo ${HOST})\"");
    write(&format!("{path}.local"), "HOST=overridden");

    load_test_env(&mut g, &path)?;

    assert_eq!(Some("overridden".to_owned()), getenv(&g, "HOST"));
    assert_eq!(Some("overridden".to_owned()), getenv(&g, "RESOLVED"));
    Ok(())
}

#[test]
fn testLoadEnvResolvesUnquotedCommandsWithOverriddenVars() -> TestResult {
    if cfg!(windows) {
        // markTestSkipped: This test cannot be run on Windows.
        return Ok(());
    }

    let mut g = Globals::new();

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    write(&path, "HOST=original\nRESOLVED=$(echo ${HOST})");
    write(&format!("{path}.local"), "HOST=overridden");

    Dotenv::new().load_env(&mut g, &path, Some("TEST_APP_ENV"), "dev", &["test"], false)?;

    assert_eq!(Some("overridden".to_owned()), get(&g.env, "HOST"));
    assert_eq!(Some("overridden".to_owned()), get(&g.env, "RESOLVED"));
    Ok(())
}

#[test]
fn testLoadEnvSelfReferencingVariableWithDefault() -> TestResult {
    let mut g = Globals::new();
    let reset_context =
        |g: &mut Globals| clear(g, &["SYMFONY_DOTENV_VARS", "MY_VAR", "TEST_APP_ENV"]);

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    // Self-referencing variable with default value
    write(&path, "MY_VAR=\"${MY_VAR:-default_value}\"");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(Some("default_value".to_owned()), getenv(&g, "MY_VAR"));

    // When host env is set, it should take precedence
    reset_context(&mut g);
    putenv(&mut g, "MY_VAR=host_value");
    set(&mut g.env, "MY_VAR", "host_value");
    load_test_env(&mut g, &path)?;

    assert_eq!(Some("host_value".to_owned()), getenv(&g, "MY_VAR"));

    // Self-referencing variable with := (assign default)
    write(&path, "MY_VAR=\"${MY_VAR:=fallback}\"");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(Some("fallback".to_owned()), getenv(&g, "MY_VAR"));
    Ok(())
}

#[test]
fn testLoadSelfReferencingVariableWithSuffix() -> TestResult {
    let mut g = Globals::new();
    let reset_context = |g: &mut Globals| clear(g, &["SYMFONY_DOTENV_VARS", "MY_VAR"]);

    let tmpdir = TempDir::new();
    let base_path = tmpdir.tempnam();
    let override_path = tmpdir.tempnam();

    // Base file sets original value, override file appends suffix
    write(&base_path, "MY_VAR=original");
    write(&override_path, "MY_VAR=\"${MY_VAR}_suffix\"");

    reset_context(&mut g);
    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.load(&mut g, &[&base_path])?;
    dotenv.load(&mut g, &[&override_path])?;

    assert_eq!(Some("original_suffix".to_owned()), getenv(&g, "MY_VAR"));

    // Test with prefix instead of suffix
    write(&override_path, "MY_VAR=\"prefix_${MY_VAR}\"");

    reset_context(&mut g);
    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.load(&mut g, &[&base_path])?;
    dotenv.load(&mut g, &[&override_path])?;

    assert_eq!(Some("prefix_original".to_owned()), getenv(&g, "MY_VAR"));

    // Test chained loads (three files)
    let third_path = tmpdir.tempnam();
    write(&override_path, "MY_VAR=\"${MY_VAR}_middle\"");
    write(&third_path, "MY_VAR=\"${MY_VAR}_end\"");

    reset_context(&mut g);
    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.load(&mut g, &[&base_path])?;
    dotenv.load(&mut g, &[&override_path])?;
    dotenv.load(&mut g, &[&third_path])?;

    assert_eq!(Some("original_middle_end".to_owned()), getenv(&g, "MY_VAR"));
    Ok(())
}

#[test]
fn testLoadEnvSelfReferencingEnvKeyControlsFileLoading() -> TestResult {
    let mut g = Globals::new();
    let reset_context = |g: &mut Globals| clear(g, &["SYMFONY_DOTENV_VARS", "TEST_APP_ENV", "FOO"]);

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    // APP_ENV with self-referencing default must control which .env files are loaded
    write(&path, "TEST_APP_ENV=\"${TEST_APP_ENV:-dev}\"\nFOO=bar");
    write(&format!("{path}.dev"), "FOO=devbar");

    reset_context(&mut g);
    load_test_env(&mut g, &path)?;

    assert_eq!(Some("dev".to_owned()), getenv(&g, "TEST_APP_ENV"));
    assert_eq!(Some("devbar".to_owned()), getenv(&g, "FOO"));

    // Host env should override the default and control file loading
    reset_context(&mut g);
    write(&format!("{path}.prod"), "FOO=prodbar");
    putenv(&mut g, "TEST_APP_ENV=prod");
    set(&mut g.env, "TEST_APP_ENV", "prod");
    load_test_env(&mut g, &path)?;

    assert_eq!(Some("prod".to_owned()), getenv(&g, "TEST_APP_ENV"));
    assert_eq!(Some("prodbar".to_owned()), getenv(&g, "FOO"));
    Ok(())
}

#[test]
fn testLoadEnvThrowsOnCircularVariableReferences() {
    let mut g = Globals::new();

    let tmpdir = TempDir::new();
    let path1 = tmpdir.tempnam();
    let path2 = tmpdir.tempnam();

    // Mutual references that grow each pass — never stabilize
    write(&path1, "A=${B}x");
    write(&path2, "B=${A}y");

    match Dotenv::new().load(&mut g, &[&path1, &path2]) {
        Err(e @ Error::VariableCircularReference(_)) => {
            assert!(
                e.to_string()
                    .contains("Too many levels of variable indirection"),
                "{e}"
            );
        }
        other => panic!("expected a VariableCircularReferenceException, got {other:?}"),
    }
}

#[test]
fn testLoadEnvUnquotedSpaceWithVariableDoesNotThrow() -> TestResult {
    let mut g = Globals::new();

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    // Unquoted value with a space and a variable reference
    write(&path, "PREFIX=hello\nLABEL=${PREFIX} world");
    write(&format!("{path}.local"), "PREFIX=overridden");

    load_test_env(&mut g, &path)?;

    assert_eq!(Some("overridden".to_owned()), getenv(&g, "PREFIX"));
    assert_eq!(Some("overridden world".to_owned()), getenv(&g, "LABEL"));
    Ok(())
}

#[test]
fn testOverload() -> TestResult {
    let mut g = Globals::new();

    putenv(&mut g, "FOO=initial_foo_value");
    putenv(&mut g, "BAR=initial_bar_value");
    set(&mut g.env, "FOO", "initial_foo_value");
    set(&mut g.env, "BAR", "initial_bar_value");

    let tmpdir = TempDir::new();

    let path1 = tmpdir.tempnam();
    let path2 = tmpdir.tempnam();

    write(&path1, "FOO=BAR");
    write(&path2, "BAR=BAZ");

    Dotenv::new()
        .use_putenv(true)
        .overload(&mut g, &[&path1, &path2])?;

    assert_eq!(Some("BAR".to_owned()), getenv(&g, "FOO"));
    assert_eq!(Some("BAZ".to_owned()), getenv(&g, "BAR"));
    Ok(())
}

#[test]
fn testLoadDirectory() {
    let mut g = Globals::new();
    let dir = env!("CARGO_MANIFEST_DIR").replace('\\', "/") + "/tests";
    let result = Dotenv::new().load(&mut g, &[&dir]);
    assert!(matches!(result, Err(Error::Path(_))), "{result:?}");
}

#[test]
fn testServerSuperglobalIsNotOverridden() {
    let mut g = Globals::new();
    // PHPUnit's CLI run has $_SERVER['argc']
    g.server.insert("argc", Value::Int(1));
    let original_value = g.server.get(&Key::from("argc")).cloned();

    let mut dotenv = Dotenv::new();
    dotenv.populate(&mut g, &php_array!["argc" => "new_value"], false);

    assert_eq!(original_value, g.server.get(&Key::from("argc")).cloned());
}

#[test]
fn testEnvVarIsNotOverridden() {
    let mut g = Globals::new();
    putenv(&mut g, "TEST_ENV_VAR=original_value");
    set(&mut g.server, "TEST_ENV_VAR", "original_value");

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(&mut g, &php_array!["TEST_ENV_VAR" => "new_value"], false);

    assert_eq!(
        Some("original_value".to_owned()),
        getenv(&g, "TEST_ENV_VAR")
    );
}

#[test]
fn testHttpVarIsPartiallyOverridden() {
    let mut g = Globals::new();
    set(&mut g.server, "HTTP_TEST_ENV_VAR", "http_value");

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(
        &mut g,
        &php_array!["HTTP_TEST_ENV_VAR" => "env_value"],
        false,
    );

    assert_eq!(
        Some("env_value".to_owned()),
        getenv(&g, "HTTP_TEST_ENV_VAR")
    );
    assert_eq!(
        Some("env_value".to_owned()),
        get(&g.env, "HTTP_TEST_ENV_VAR")
    );
    assert_eq!(
        Some("http_value".to_owned()),
        get(&g.server, "HTTP_TEST_ENV_VAR")
    );
}

#[test]
fn testEnvVarIsOverridden() {
    let mut g = Globals::new();
    putenv(&mut g, "TEST_ENV_VAR_OVERRIDDEN=original_value");

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(
        &mut g,
        &php_array!["TEST_ENV_VAR_OVERRIDDEN" => "new_value"],
        true,
    );

    assert_eq!(
        Some("new_value".to_owned()),
        getenv(&g, "TEST_ENV_VAR_OVERRIDDEN")
    );
    assert_eq!(
        Some("new_value".to_owned()),
        get(&g.env, "TEST_ENV_VAR_OVERRIDDEN")
    );
    assert_eq!(
        Some("new_value".to_owned()),
        get(&g.server, "TEST_ENV_VAR_OVERRIDDEN")
    );
}

#[test]
fn testMemorizingLoadedVarsNamesInSpecialVar() {
    let mut g = Globals::new();
    // Special variable not exists
    clear(
        &mut g,
        &["SYMFONY_DOTENV_VARS", "APP_DEBUG", "DATABASE_URL"],
    );

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(
        &mut g,
        &php_array!["APP_DEBUG" => "1", "DATABASE_URL" => "mysql://root@localhost/db"],
        false,
    );

    assert_eq!(
        Some("APP_DEBUG,DATABASE_URL".to_owned()),
        getenv(&g, "SYMFONY_DOTENV_VARS")
    );

    // Special variable has a value
    set(&mut g.env, "SYMFONY_DOTENV_VARS", "APP_ENV");
    set(&mut g.server, "SYMFONY_DOTENV_VARS", "APP_ENV");
    putenv(&mut g, "SYMFONY_DOTENV_VARS=APP_ENV");

    set(&mut g.env, "APP_DEBUG", "1");
    set(&mut g.server, "APP_DEBUG", "1");
    putenv(&mut g, "APP_DEBUG=1");
    clear(&mut g, &["DATABASE_URL"]);

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(
        &mut g,
        &php_array!["APP_DEBUG" => "0", "DATABASE_URL" => "mysql://root@localhost/db"],
        false,
    );
    dotenv.populate(
        &mut g,
        &php_array!["DATABASE_URL" => "sqlite:///somedb.sqlite"],
        false,
    );

    assert_eq!(
        Some("APP_ENV,DATABASE_URL".to_owned()),
        getenv(&g, "SYMFONY_DOTENV_VARS")
    );
}

#[test]
fn testOverridingEnvVarsWithNamesMemorizedInSpecialVar() {
    let mut g = Globals::new();
    // PHP's CLI (PHPUnit's) has $_SERVER['DOCUMENT_ROOT'] = ''
    set(&mut g.server, "DOCUMENT_ROOT", "");
    set(&mut g.server, "SYMFONY_DOTENV_VARS", "FOO,BAR,BAZ");
    putenv(&mut g, "SYMFONY_DOTENV_VARS=FOO,BAR,BAZ");

    putenv(&mut g, "FOO=foo");
    putenv(&mut g, "BAR=bar");
    putenv(&mut g, "BAZ=baz");
    putenv(&mut g, "DOCUMENT_ROOT=/var/www");

    let mut dotenv = Dotenv::new().use_putenv(true);
    dotenv.populate(
        &mut g,
        &php_array!["FOO" => "foo1", "BAR" => "bar1", "BAZ" => "baz1", "DOCUMENT_ROOT" => "/boot"],
        false,
    );

    assert_eq!(Some("foo1".to_owned()), getenv(&g, "FOO"));
    assert_eq!(Some("bar1".to_owned()), getenv(&g, "BAR"));
    assert_eq!(Some("baz1".to_owned()), getenv(&g, "BAZ"));
    assert_eq!(Some("/var/www".to_owned()), getenv(&g, "DOCUMENT_ROOT"));
}

#[test]
fn testGetVariablesValueFromEnvFirst() -> TestResult {
    let mut g = Globals::new();
    set(&mut g.env, "APP_ENV", "prod");
    let mut dotenv = Dotenv::new();

    let test = "APP_ENV=dev\nTEST1=foo1_${APP_ENV}";
    let values = dotenv.parse(&g, test.as_bytes(), ".env")?;
    assert_eq!(Some("foo1_prod".to_owned()), get(&values, "TEST1"));

    if !cfg!(windows) {
        // upstream: $(php -r 'echo $_SERVER["APP_ENV"];')
        let test = "APP_ENV=dev\nTEST2=foo2_$(printenv APP_ENV)";
        let values = dotenv.parse(&g, test.as_bytes(), ".env")?;
        assert_eq!(Some("foo2_prod".to_owned()), get(&values, "TEST2"));
    }
    Ok(())
}

#[test]
fn testGetVariablesValueFromGetenv() -> TestResult {
    let mut g = Globals::new();
    putenv(&mut g, "Foo=Bar");

    let mut dotenv = Dotenv::new();

    let values = dotenv.parse(&g, b"Foo=${Foo}", ".env")?;
    assert_eq!(Some("Bar".to_owned()), get(&values, "Foo"));
    Ok(())
}

#[test]
fn testNoDeprecationWarning() {
    // assertInstanceOf(Dotenv::class, new Dotenv())
    let _dotenv: Dotenv = Dotenv::new();
}

#[test]
fn testDoNotUsePutenv() {
    let mut g = Globals::new();
    let mut dotenv = Dotenv::new();
    dotenv.populate(&mut g, &php_array!["TEST_USE_PUTENV" => "no"], false);

    assert_eq!(Some("no".to_owned()), get(&g.server, "TEST_USE_PUTENV"));
    assert_eq!(Some("no".to_owned()), get(&g.env, "TEST_USE_PUTENV"));
    assert_eq!(None, getenv(&g, "TEST_USE_PUTENV"));
}

#[test]
fn testSERVERVarsDuplicationInENV() {
    let mut g = Globals::new();
    set(&mut g.server, "FOO", "CCC");

    Dotenv::new().populate(&mut g, &php_array!["FOO" => "BAR"], false);

    assert_eq!(Some("CCC".to_owned()), get(&g.env, "FOO"));
}

#[test]
fn testBootEnv() -> TestResult {
    let mut g = Globals::new();
    let reset_context = |g: &mut Globals| {
        for name in [
            "SYMFONY_DOTENV_VARS",
            "TEST_APP_ENV",
            "TEST_APP_DEBUG",
            "FOO",
        ] {
            unset(&mut g.server, &[name]);
            unset(&mut g.env, &[name]);
        }

        set(&mut g.env, "EXISTING_KEY", "EXISTING_VALUE");
        set(&mut g.server, "EXISTING_KEY", "EXISTING_VALUE");
    };
    let dotenv = || Dotenv::with_keys("TEST_APP_ENV", "TEST_APP_DEBUG");
    let s = |v: &str| Some(v.to_owned());

    let tmpdir = TempDir::new();
    let path = tmpdir.tempnam();

    write(&path, "FOO=BAR\nEXISTING_KEY=NEW_VALUE");
    reset_context(&mut g);
    dotenv().boot_env(&mut g, &path, "dev", &["test"], false)?;
    assert_eq!(s("BAR"), get(&g.server, "FOO"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.server, "EXISTING_KEY"));

    reset_context(&mut g);
    dotenv().boot_env(&mut g, &path, "dev", &["test"], true)?;
    assert_eq!(s("BAR"), get(&g.server, "FOO"));
    assert_eq!(s("NEW_VALUE"), get(&g.server, "EXISTING_KEY"));
    unlink(&path);

    let local_php = format!("{path}.local.php");
    write(
        &local_php,
        "<?php return [\"TEST_APP_ENV\" => \"dev\", \"FOO\" => \"BAR\", \"EXISTING_KEY\" => \"localphpNEW_VALUE\"];",
    );
    reset_context(&mut g);
    dotenv().boot_env(&mut g, &path, "dev", &["test"], false)?;
    assert_eq!(s("BAR"), get(&g.server, "FOO"));
    assert_eq!(s("1"), get(&g.server, "TEST_APP_DEBUG"));
    assert_eq!(s("EXISTING_VALUE"), get(&g.server, "EXISTING_KEY"));

    reset_context(&mut g);
    dotenv().boot_env(&mut g, &path, "dev", &["test"], true)?;
    assert_eq!(s("BAR"), get(&g.server, "FOO"));
    assert_eq!(s("1"), get(&g.server, "TEST_APP_DEBUG"));
    assert_eq!(s("localphpNEW_VALUE"), get(&g.server, "EXISTING_KEY"));

    reset_context(&mut g);
    set(&mut g.server, "TEST_APP_ENV", "ccc");
    dotenv().boot_env(&mut g, &path, "dev", &["test"], true)?;
    assert_eq!(s("BAR"), get(&g.server, "FOO"));
    assert_eq!(s("1"), get(&g.server, "TEST_APP_DEBUG"));
    assert_eq!(s("localphpNEW_VALUE"), get(&g.server, "EXISTING_KEY"));
    unlink(&local_php);
    Ok(())
}

#[test]
fn testExceptionWithBom() {
    let mut g = Globals::new();
    let mut dotenv = Dotenv::new();

    let path = env!("CARGO_MANIFEST_DIR").replace('\\', "/") + "/tests/fixtures/file_with_bom";
    match dotenv.load(&mut g, &[&path]) {
        Err(e @ Error::Format(_)) => assert!(
            e.to_string()
                .contains("Loading files starting with a byte-order-mark (BOM) is not supported."),
            "{e}"
        ),
        other => panic!("expected a FormatException, got {other:?}"),
    }
}
