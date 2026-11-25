//! FileBagTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. `createTempFile()` writes
//! into a directory of the test's own (upstream: `form_test` in the
//! system's temporary directory, made in `setUp()`, emptied in
//! `tearDown()`). `assertEquals()` on two `UploadedFiles` compares their
//! properties, as `==` on the Rust values does.
//!
//! Upstream: symfony/http-foundation Tests/FileBagTest.php
#![allow(
    clippy::similar_names,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod files;

use std::path::PathBuf;

use files::{TestResult, fixture};
use php_value::{Key, PhpArray, PhpString, Value};
use symfony_http_foundation::{
    file_bag::{FileBag, uploaded_file},
    php::UPLOAD_ERR_NO_FILE,
    uploaded_file::UploadedFile,
};

/// `setUp()` / `tearDown()`: a temporary directory for one test.
struct FormTest(PathBuf);

impl FormTest {
    fn new(tag: &str) -> std::io::Result<Self> {
        let dir =
            std::env::temp_dir().join(format!("symfony-rs-form_test-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }

    /// `createTempFile()`: a file holding `1`.
    fn create_temp_file(&self) -> std::io::Result<(String, String)> {
        let path = self.0.join("FormTest");
        std::fs::write(&path, b"1")?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((path.display().to_string(), name))
    }
}

impl Drop for FormTest {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn s(text: &str) -> Value {
    Value::from(text)
}

fn array(pairs: Vec<(&str, Value)>) -> Value {
    Value::Array(pairs.into_iter().map(|(k, v)| (Key::from(k), v)).collect())
}

fn list(values: Vec<Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

fn bag(pairs: Vec<(&str, Value)>) -> Result<FileBag, symfony_http_foundation::exception::Error> {
    let Value::Array(parameters) = array(pairs) else {
        unreachable!()
    };
    FileBag::new(parameters)
}

fn text_plain(path: &str, name: &str) -> UploadedFile {
    UploadedFile::new(
        path.as_bytes().to_vec(),
        name,
        Some(PhpString::from("text/plain")),
        None,
        false,
    )
    .expect("the temporary file exists")
}

/// `$value[$key]` of an array value.
fn at(value: &Value, key: impl Into<Key>) -> &Value {
    let key = key.into();
    value
        .as_array()
        .and_then(|a| a.get(&key))
        .unwrap_or_else(|| panic!("no {key:?} in {value:?}"))
}

#[test]
fn testFileMustBeAnArrayOrUploadedFile() {
    let error = bag(vec![("file", s("foo"))]).expect_err("InvalidArgumentException");
    assert!(error.is_a("InvalidArgumentException"), "{error}");
}

#[test]
fn testShouldConvertsUploadedFiles() -> TestResult {
    let dir = FormTest::new("converts")?;
    let (tmp_file, name) = dir.create_temp_file()?;
    let file = text_plain(&tmp_file, &name);

    let bag = bag(vec![(
        "file",
        array(vec![
            ("name", s(&name)),
            ("type", s("text/plain")),
            ("tmp_name", s(&tmp_file)),
            ("error", Value::Int(0)),
            ("size", Value::Null),
        ]),
    )])?;

    assert_eq!(Some(&file), uploaded_file(&bag.get("file", Value::Null)));
    Ok(())
}

#[test]
fn testShouldSetEmptyUploadedFilesToNull() -> TestResult {
    let bag = bag(vec![(
        "file",
        array(vec![
            ("name", s("")),
            ("type", s("")),
            ("tmp_name", s("")),
            ("error", Value::Int(UPLOAD_ERR_NO_FILE)),
            ("size", Value::Int(0)),
        ]),
    )])?;

    assert!(bag.get("file", Value::Bool(false)).is_null());
    Ok(())
}

#[test]
fn testShouldRemoveEmptyUploadedFilesForMultiUpload() -> TestResult {
    let bag = bag(vec![(
        "files",
        array(vec![
            ("name", list(vec![s("")])),
            ("type", list(vec![s("")])),
            ("tmp_name", list(vec![s("")])),
            ("error", list(vec![Value::Int(UPLOAD_ERR_NO_FILE)])),
            ("size", list(vec![Value::Int(0)])),
        ]),
    )])?;

    assert_eq!(Value::Array(PhpArray::new()), bag.get("files", Value::Null));
    Ok(())
}

#[test]
fn testShouldNotRemoveEmptyUploadedFilesForAssociativeArray() -> TestResult {
    let bag = bag(vec![(
        "files",
        array(vec![
            ("name", array(vec![("file1", s(""))])),
            ("type", array(vec![("file1", s(""))])),
            ("tmp_name", array(vec![("file1", s(""))])),
            (
                "error",
                array(vec![("file1", Value::Int(UPLOAD_ERR_NO_FILE))]),
            ),
            ("size", array(vec![("file1", Value::Int(0))])),
        ]),
    )])?;

    assert_eq!(
        array(vec![("file1", Value::Null)]),
        bag.get("files", Value::Null)
    );
    Ok(())
}

#[test]
fn testShouldConvertUploadedFilesWithPhpBug() -> TestResult {
    let dir = FormTest::new("php-bug")?;
    let (tmp_file, name) = dir.create_temp_file()?;
    let file = text_plain(&tmp_file, &name);

    let bag = bag(vec![(
        "child",
        array(vec![
            ("name", array(vec![("file", s(&name))])),
            ("type", array(vec![("file", s("text/plain"))])),
            ("tmp_name", array(vec![("file", s(&tmp_file))])),
            ("error", array(vec![("file", Value::Int(0))])),
            ("size", array(vec![("file", Value::Null)])),
        ]),
    )])?;

    let files = Value::Array(bag.all().clone());
    assert_eq!(Some(&file), uploaded_file(at(at(&files, "child"), "file")));
    Ok(())
}

#[test]
fn testShouldConvertNestedUploadedFilesWithPhpBug() -> TestResult {
    let dir = FormTest::new("nested-php-bug")?;
    let (tmp_file, name) = dir.create_temp_file()?;
    let file = text_plain(&tmp_file, &name);

    let nested = |v: Value| array(vec![("sub", array(vec![("file", v)]))]);
    let bag = bag(vec![(
        "child",
        array(vec![
            ("name", nested(s(&name))),
            ("type", nested(s("text/plain"))),
            ("tmp_name", nested(s(&tmp_file))),
            ("error", nested(Value::Int(0))),
            ("size", nested(Value::Null)),
        ]),
    )])?;

    let files = Value::Array(bag.all().clone());
    assert_eq!(
        Some(&file),
        uploaded_file(at(at(at(&files, "child"), "sub"), "file"))
    );
    Ok(())
}

#[test]
fn testShouldNotConvertNestedUploadedFiles() -> TestResult {
    let dir = FormTest::new("not-nested")?;
    let (tmp_file, name) = dir.create_temp_file()?;
    let file = text_plain(&tmp_file, &name);
    let object = Value::Object(std::sync::Arc::new(file.clone()));
    let bag = bag(vec![("image", array(vec![("file", object)]))])?;

    let files = Value::Array(bag.all().clone());
    assert_eq!(Some(&file), uploaded_file(at(at(&files, "image"), "file")));
    Ok(())
}

#[test]
fn testWebkitDirectoryUpload() -> TestResult {
    let file1 = String::from_utf8(fixture("webkitdirectory/test.txt"))?;
    let file2 = String::from_utf8(fixture("webkitdirectory/nested/test.txt"))?;

    let bag = bag(vec![(
        "child",
        array(vec![
            ("name", list(vec![s("test.txt"), s("test.txt")])),
            (
                "full_path",
                list(vec![
                    s("webkitdirectory/test.txt"),
                    s("webkitdirectory/nested/test.txt"),
                ]),
            ),
            ("type", list(vec![s("text/plain"), s("text/plain")])),
            ("tmp_name", list(vec![s(&file1), s(&file2)])),
            ("error", list(vec![Value::Int(0), Value::Int(0)])),
            ("size", list(vec![Value::Null, Value::Null])),
        ]),
    )])?;

    let files = bag.get("child", Value::Null);
    let first = uploaded_file(at(&files, 0)).expect("an UploadedFile");
    let second = uploaded_file(at(&files, 1)).expect("an UploadedFile");

    assert_eq!(
        &PhpString::from("test.txt"),
        first.get_client_original_name()
    );
    assert_eq!(
        &PhpString::from("test.txt"),
        second.get_client_original_name()
    );

    assert_eq!(
        &PhpString::from("webkitdirectory/test.txt"),
        first.get_client_original_path()
    );
    assert_eq!(
        &PhpString::from("webkitdirectory/nested/test.txt"),
        second.get_client_original_path()
    );
    Ok(())
}
