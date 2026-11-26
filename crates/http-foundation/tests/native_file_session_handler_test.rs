//! Session/Storage/Handler/NativeFileSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! each test runs on a thread of its own (PHPUnit: a process of its own),
//! under a directory of its own in the system's temporary one. Each data
//! set of the provider is a case of the same test.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/NativeFileSessionHandlerTest.php
#![allow(non_snake_case)]

#[path = "native_session/mod.rs"]
mod native_session;

use std::sync::Arc;

use native_session::{SavePath, TestResult};
use php_value::{PhpArray, PhpString, Value, path::from_path};
use symfony_http_foundation::{
    php_session::ini_get,
    session::storage::{
        handler::native_file_session_handler::NativeFileSessionHandler,
        native_session_storage::{NativeSessionStorage, SaveHandler},
    },
};

fn name_testing() -> PhpArray {
    let mut options = PhpArray::new();
    options.insert("name", Value::string("TESTING"));
    options
}

#[test]
fn testConstruct() -> TestResult {
    let base = SavePath::unique("native-file");
    std::fs::create_dir_all(&base)?;
    let base_bytes = from_path(&base);
    NativeSessionStorage::new(
        &name_testing(),
        Some(SaveHandler::Handler(Arc::new(
            NativeFileSessionHandler::new(Some(&base_bytes))?,
        ))),
        None,
    )?;
    assert_eq!(Some(PhpString::from("user")), ini_get("save_handler"));
    assert_eq!(Some(PhpString::from(base_bytes)), ini_get("save_path"));
    assert_eq!(Some(PhpString::from("TESTING")), ini_get("name"));
    std::fs::remove_dir_all(&base)?;
    Ok(())
}

#[test]
fn testConstructSavePath() -> TestResult {
    let base = SavePath::unique("native-file");
    let base = from_path(&base);
    let base = String::from_utf8(base)?;
    for (save_path, expected_save_path, path) in [
        (
            format!("{base}/foo"),
            format!("{base}/foo"),
            format!("{base}/foo"),
        ),
        (
            format!("5;{base}/foo"),
            format!("5;{base}/foo"),
            format!("{base}/foo"),
        ),
        (
            format!("5;0600;{base}/foo"),
            format!("5;0600;{base}/foo"),
            format!("{base}/foo"),
        ),
    ] {
        NativeFileSessionHandler::new(Some(save_path.as_bytes()))?;
        assert_eq!(
            Some(PhpString::from(expected_save_path)),
            ini_get("save_path")
        );
        assert!(std::path::Path::new(&path).is_dir());
        std::fs::remove_dir(&path)?;
    }
    let _ = std::fs::remove_dir(&base);
    Ok(())
}

#[test]
fn testConstructException() -> TestResult {
    let error = NativeFileSessionHandler::new(Some(b"something;invalid;with;too-many-args"))
        .err()
        .ok_or("expected an InvalidArgumentException")?;
    assert!(error.is_a("InvalidArgumentException"));
    Ok(())
}

#[test]
fn testConstructDefault() -> TestResult {
    let path = ini_get("save_path");
    NativeSessionStorage::new(
        &name_testing(),
        Some(SaveHandler::Handler(Arc::new(
            NativeFileSessionHandler::new(None)?,
        ))),
        None,
    )?;
    assert_eq!(path, ini_get("save_path"));
    Ok(())
}
