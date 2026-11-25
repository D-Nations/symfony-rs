//! `UploadedFile` and `FileBag` against PHP 8.4.27's answers for what
//! `UploadedFileTest` and `FileBagTest` leave open (tests/data/files.json,
//! `oracle/recorders/http_foundation_files.php`): 108 `parseFilesize()`
//! texts, 126 `getMaxFilesize()` ini pairs with their `UPLOAD_ERR_INI_SIZE`
//! message, 55 client names, 33 client MIME types, 26 error codes in and
//! out of test mode, and 71 `$_FILES`-like arrays through `FileBag`.
//!
//! Paths under the fixtures directory are recorded as `@FIX@...`. PHP's
//! `TypeError` messages are compared up to their ", called in" suffix.
//!
//! Upstream: symfony/http-foundation File/UploadedFile.php FileBag.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod files;

use php_value::{
    Number, PhpArray, PhpString, Value,
    vectors::{Outcome, decode, outcome},
};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::Error,
    file_bag::{FileBag, uploaded_file},
    uploaded_file::{UploadedFile, exception_message, max_filesize, parse_filesize},
};

/// The recorder's fixtures directory.
const RECORDED_FIX: &str = "/work/crates/http-foundation/tests/fixtures/file";

fn data() -> Json {
    serde_json::from_str(include_str!("data/files.json")).unwrap()
}

fn rows<'a>(data: &'a Json, section: &str) -> &'a Vec<Json> {
    data[section].as_array().unwrap()
}

fn bytes(value: &Value) -> Vec<u8> {
    value.as_string().unwrap().as_bytes().to_vec()
}

fn number(n: Number) -> Value {
    match n {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

fn string_or_null(s: Option<String>) -> Value {
    s.map_or(Value::Null, |s| Value::from(s.as_str()))
}

/// `@FIX@` paths to this checkout's fixtures, in a recorded input.
fn fix(value: Value) -> Value {
    match value {
        Value::String(s) if s.as_bytes().starts_with(b"@FIX@") => {
            let mut path = files::fixtures().into_bytes();
            path.extend_from_slice(&s.as_bytes()[5..]);
            Value::String(PhpString::from(path))
        }
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|(k, v)| (k.clone(), fix(v.clone())))
                .collect(),
        ),
        other => other,
    }
}

fn unfix_bytes(path: &[u8]) -> Vec<u8> {
    let fixtures = files::fixtures().into_bytes();
    match path.strip_prefix(fixtures.as_slice()) {
        Some(rest) => [b"@FIX@".as_slice(), rest].concat(),
        None => path.to_vec(),
    }
}

/// A result as the recorder writes it: `UploadedFiles` as
/// `['@uploaded', pathname, name, path, type, error]`.
fn unfix(value: &Value) -> Value {
    if let Some(file) = uploaded_file(value) {
        return Value::Array(PhpArray::list([
            Value::from("@uploaded"),
            Value::String(PhpString::from(unfix_bytes(file.get_pathname()))),
            Value::String(file.get_client_original_name().clone()),
            Value::String(file.get_client_original_path().clone()),
            Value::String(file.get_client_mime_type().clone()),
            Value::Int(file.get_error()),
        ]));
    }
    match value {
        Value::String(s) => Value::String(PhpString::from(unfix_bytes(s.as_bytes()))),
        Value::Array(array) => {
            Value::Array(array.iter().map(|(k, v)| (k.clone(), unfix(v))).collect())
        }
        other => other.clone(),
    }
}

fn assert_throws(error: &Error, class: &str, message: &str, case: &str) {
    assert_eq!(class, error.class().name(), "{case}: {error}");
    let expected = message.replace(RECORDED_FIX, &files::fixtures());
    let expected = expected.split(", called in ").next().unwrap_or_default();
    assert_eq!(expected, error.message().to_string_lossy(), "{case}");
}

#[test]
fn recorded_parse_filesize() {
    let data = data();
    for row in rows(&data, "parse_filesize") {
        let text = bytes(&decode(&row[0]).unwrap());
        let Outcome::Value(expected) = outcome(&row[1]).unwrap() else {
            panic!("{row}");
        };
        let actual = number(parse_filesize(&text));
        match (&expected, &actual) {
            (Value::Float(a), Value::Float(b)) => {
                assert_eq!(a.to_bits(), b.to_bits(), "{}", text.escape_ascii());
            }
            _ => assert_eq!(expected, actual, "{}", text.escape_ascii()),
        }
    }
}

#[test]
fn recorded_max_filesize() {
    let data = data();
    for row in rows(&data, "max_filesize") {
        let post = bytes(&decode(&row[0]).unwrap());
        let upload = bytes(&decode(&row[1]).unwrap());
        let case = format!("{} / {}", post.escape_ascii(), upload.escape_ascii());
        let max = max_filesize(&post, &upload);
        assert_eq!(decode(&row[2]).unwrap(), number(max), "{case}");
        let message = exception_message(1, &PhpString::from("a.gif"), max);
        assert_eq!(decode(&row[3]).unwrap(), Value::String(message), "{case}");
    }
}

#[test]
fn recorded_names() {
    let data = data();
    for row in rows(&data, "names") {
        let name = decode(&row[0]).unwrap();
        let file = UploadedFile::new(
            b"/nonexistent".to_vec(),
            name.as_string().unwrap().clone(),
            None,
            Some(4),
            false,
        )
        .unwrap();
        let case = format!("{name:?}");
        assert_eq!(
            decode(&row[1]).unwrap(),
            Value::String(file.get_client_original_name().clone()),
            "{case}"
        );
        assert_eq!(
            decode(&row[2]).unwrap(),
            Value::String(file.get_client_original_extension()),
            "{case}"
        );
        assert_eq!(
            decode(&row[3]).unwrap(),
            Value::String(file.get_client_original_path().clone()),
            "{case}"
        );
    }
}

#[test]
fn recorded_client_mime_types() {
    let data = data();
    for row in rows(&data, "mime") {
        let given = decode(&row[0]).unwrap();
        let file = UploadedFile::new(
            b"/nonexistent".to_vec(),
            "a",
            given.as_string().cloned(),
            Some(4),
            false,
        )
        .unwrap();
        let case = format!("{given:?}");
        assert_eq!(
            decode(&row[1]).unwrap(),
            Value::String(file.get_client_mime_type().clone()),
            "{case}"
        );
        let Outcome::Value(expected) = outcome(&row[2]).unwrap() else {
            panic!("{row}");
        };
        assert_eq!(
            expected,
            string_or_null(file.guess_client_extension()),
            "{case}"
        );
    }
}

#[test]
fn recorded_errors() {
    let data = data();
    for row in rows(&data, "errors") {
        let error = match decode(&row[0]).unwrap() {
            Value::Int(i) => Some(i),
            _ => None,
        };
        let test = decode(&row[1]).unwrap() == Value::Bool(true);
        let case = format!("{error:?} test={test}");
        let file = UploadedFile::new(
            files::fixture("test.gif"),
            "../x/<b>original</b>.gif",
            Some(PhpString::from("image/gif")),
            error,
            test,
        )
        .unwrap();
        assert_eq!(
            decode(&row[2]).unwrap(),
            Value::Int(file.get_error()),
            "{case}"
        );
        assert_eq!(
            decode(&row[3]).unwrap(),
            Value::String(file.get_error_message()),
            "{case}"
        );
        assert_eq!(
            decode(&row[4]).unwrap(),
            Value::Bool(file.is_valid()),
            "{case}"
        );
        if row[5][0] == "skipped" {
            continue;
        }
        let Outcome::Throws { class, message } = outcome(&row[5]).unwrap() else {
            panic!("{row}");
        };
        let thrown = file
            .move_to(&files::fixture("directory"), None)
            .unwrap_err();
        assert_throws(&thrown, &class, &message, &case);
    }
}

#[test]
fn recorded_file_bags() {
    let data = data();
    for row in rows(&data, "bags") {
        let Value::Array(given) = fix(decode(&row[0]).unwrap()) else {
            panic!("{row}");
        };
        let case = format!("{given:?}");
        match (FileBag::new(given.clone()), outcome(&row[1]).unwrap()) {
            (Ok(bag), Outcome::Value(expected)) => {
                assert_eq!(expected, unfix(&Value::Array(bag.all().clone())), "{case}");
            }
            (Err(error), Outcome::Throws { class, message }) => {
                assert_throws(&error, &class, &message, &case);
            }
            (answer, expected) => panic!("{case}: {answer:?}, PHP: {expected:?}"),
        }
    }
    assert_eq!(71, rows(&data, "bags").len());
}
