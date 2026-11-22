//! XmlUtilsTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; provider rows are upstream's, written as literals. The
//! fixtures are upstream's Tests/Fixtures, copied to tests/fixtures.
//!
//! `assertSame([], libxml_get_errors())` holds by construction: php-dom
//! keeps no global error buffer (each call returns its errors), so those
//! assertions check the internal-errors setting XmlUtils restores instead.
//! The mocked Validator is a closure counting its calls
//! (`expects($this->exactly(2))`).
//!
//! Upstream: symfony/config Tests/Util/XmlUtilsTest.php
#![allow(non_snake_case)]

mod common;

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

use common::{TestResult, expect_exception};
use php_dom::{Document, LibxmlError, libxml_use_internal_errors, load_xml};
use php_value::{Key, PhpArray, Value};
use symfony_config::{Class, Error, XmlUtils, util::Validation};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Util")
}

fn message(e: &Error) -> String {
    String::from_utf8_lossy(e.get_message()).into_owned()
}

#[track_caller]
fn expect_invalid_argument(result: Result<Document, Error>, contains: &str) {
    match result {
        Ok(_) => panic!("expected an InvalidArgumentException containing {contains:?}"),
        Err(e) => {
            assert!(e.class().is_a(&Class::InvalidArgument), "got {e}");
            assert!(
                message(&e).contains(contains),
                "{:?} does not contain {contains:?}",
                message(&e)
            );
        }
    }
}

/// How many times a mocked method was called.
type Calls = Rc<Cell<usize>>;

/// A mocked `Validator::validate()` returning `results` in turn.
fn validator(results: Vec<bool>) -> (Validate, Calls) {
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let f = move |_: &Document, _: bool, _: &mut Vec<LibxmlError>| {
        let n = counter.get();
        counter.set(n + 1);
        Ok(Value::Bool(results.get(n).copied().unwrap_or(false)))
    };
    (Box::new(f), calls)
}

/// The mocked method, as a validation callable.
type Validate = Box<dyn FnMut(&Document, bool, &mut Vec<LibxmlError>) -> Result<Value, Error>>;

#[test]
fn testLoadFile() -> TestResult {
    let fixtures = fixtures();

    expect_invalid_argument(XmlUtils::load_file(&fixtures, None), "is not a file");

    expect_invalid_argument(
        XmlUtils::load_file(&fixtures.join("non_existing.xml"), None),
        "is not a file",
    );

    // chmod($fixtures.'not_readable.xml', 0o00): upstream skips this on
    // Windows ("chmod is not supported on Windows").
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = fixtures.join("not_readable.xml");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000))?;
        let result = XmlUtils::load_file(&file, None);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644))?;
        expect_invalid_argument(result, "is not readable");
    }

    expect_invalid_argument(
        XmlUtils::load_file(&fixtures.join("invalid.xml"), None),
        "ERROR ",
    );

    expect_invalid_argument(
        XmlUtils::load_file(&fixtures.join("document_type.xml"), None),
        "Document types are not allowed",
    );

    let schema = fixtures.join("schema.xsd");
    expect_invalid_argument(
        XmlUtils::load_file(
            &fixtures.join("invalid_schema.xml"),
            Some(Validation::Schema(&schema)),
        ),
        "ERROR 1845",
    );

    expect_invalid_argument(
        XmlUtils::load_file(
            &fixtures.join("invalid_schema.xml"),
            Some(Validation::Schema(Path::new("invalid_callback_or_file"))),
        ),
        "Invalid XSD file: \"invalid_callback_or_file\".",
    );

    let (mut mock, calls) = validator(vec![false, true]);

    match XmlUtils::load_file(
        &fixtures.join("valid.xml"),
        Some(Validation::Callable(&mut *mock)),
    ) {
        Ok(_) => panic!("expected an InvalidArgumentException"),
        Err(e) => {
            assert!(e.class().is_a(&Class::InvalidArgument));
            let m = message(&e);
            assert!(
                m.starts_with("The XML file \"") && m.ends_with("\" is not valid.") && m.len() > 30,
                "{m:?}"
            );
        }
    }

    let dom = XmlUtils::load_file(
        &fixtures.join("valid.xml"),
        Some(Validation::Callable(&mut *mock)),
    )?;
    assert!(dom.document_element().is_some());
    assert_eq!(2, calls.get());
    assert!(!libxml_use_internal_errors(None));
    Ok(())
}

#[test]
fn testParseWithInvalidValidatorCallable() -> TestResult {
    let content = std::fs::read(fixtures().join("valid.xml"))?;

    let (mut mock, calls) = validator(vec![false]);

    expect_exception(
        XmlUtils::parse(&content, Some(Validation::Callable(&mut *mock))),
        Class::InvalidXml,
        "The XML is not valid",
    );
    assert_eq!(1, calls.get());
    Ok(())
}

#[test]
fn testLoadFileWithInternalErrorsEnabled() -> TestResult {
    let internal_errors = libxml_use_internal_errors(Some(true));

    assert!(libxml_use_internal_errors(None));
    let dom = XmlUtils::load_file(&fixtures().join("invalid_schema.xml"), None)?;
    assert!(dom.document_element().is_some());
    assert!(libxml_use_internal_errors(None));

    libxml_use_internal_errors(Some(internal_errors));
    Ok(())
}

fn array(pairs: Vec<(&str, Value)>) -> Value {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), v);
    }
    Value::Array(a)
}

fn list(values: Vec<Value>) -> Value {
    let mut a = PhpArray::new();
    for v in values {
        a.push(v).expect("a short list");
    }
    Value::Array(a)
}

fn s(text: &str) -> Value {
    Value::string(text)
}

fn getDataForConvertDomToArray() -> Vec<(Value, &'static str, bool, bool)> {
    vec![
        (Value::Null, "", false, true),
        (s("bar"), "bar", false, true),
        (
            array(vec![("bar", s("foobar"))]),
            "<foo bar=\"foobar\" />",
            true,
            true,
        ),
        (array(vec![("foo", Value::Null)]), "<foo />", false, true),
        (
            array(vec![("foo", s("bar"))]),
            "<foo>bar</foo>",
            false,
            true,
        ),
        (
            array(vec![("foo", array(vec![("foo", s("bar"))]))]),
            "<foo foo=\"bar\"/>",
            false,
            true,
        ),
        (
            array(vec![("foo", array(vec![("foo", Value::Int(0))]))]),
            "<foo><foo>0</foo></foo>",
            false,
            true,
        ),
        (
            array(vec![("foo", array(vec![("foo", s("bar"))]))]),
            "<foo><foo>bar</foo></foo>",
            false,
            true,
        ),
        (
            array(vec![(
                "foo",
                array(vec![("foo", s("bar")), ("value", s("text"))]),
            )]),
            "<foo foo=\"bar\">text</foo>",
            false,
            true,
        ),
        (
            array(vec![(
                "foo",
                array(vec![("attr", s("bar")), ("foo", s("text"))]),
            )]),
            "<foo attr=\"bar\"><foo>text</foo></foo>",
            false,
            true,
        ),
        (
            array(vec![("foo", list(vec![s("bar"), s("text")]))]),
            "<foo>bar</foo><foo>text</foo>",
            false,
            true,
        ),
        (
            array(vec![(
                "foo",
                list(vec![
                    array(vec![("foo", s("bar"))]),
                    array(vec![("foo", s("text"))]),
                ]),
            )]),
            "<foo foo=\"bar\"/><foo foo=\"text\" />",
            false,
            true,
        ),
        (
            array(vec![(
                "foo",
                array(vec![("foo", list(vec![s("bar"), s("text")]))]),
            )]),
            "<foo foo=\"bar\"><foo>text</foo></foo>",
            false,
            true,
        ),
        (
            array(vec![("foo", s("bar"))]),
            "<foo><!-- Comment -->bar</foo>",
            false,
            true,
        ),
        (
            array(vec![("foo", s("text"))]),
            "<foo xmlns:h=\"http://www.example.org/bar\" h:bar=\"bar\">text</foo>",
            false,
            true,
        ),
        (
            array(vec![(
                "foo",
                array(vec![("bar", s("bar")), ("value", s("text"))]),
            )]),
            "<foo xmlns:h=\"http://www.example.org/bar\" h:bar=\"bar\">text</foo>",
            false,
            false,
        ),
        (
            array(vec![("attr", Value::Int(1)), ("b", s("hello"))]),
            "<foo:a xmlns:foo=\"http://www.example.org/foo\" xmlns:h=\"http://www.example.org/bar\" attr=\"1\" h:bar=\"bar\"><foo:b>hello</foo:b><h:c>2</h:c></foo:a>",
            true,
            true,
        ),
    ]
}

#[test]
fn testConvertDomToArray() -> TestResult {
    for (expected, xml, root, check_prefix) in getDataForConvertDomToArray() {
        let source = if root {
            xml.to_string()
        } else {
            format!("<root>{xml}</root>")
        };
        let dom = load_xml(source.as_bytes(), Default::default())?
            .document
            .ok_or("loadXML() failed")?;
        let element = dom.document_element().ok_or("no document element")?;

        assert_eq!(
            expected,
            XmlUtils::convert_dom_element_to_array(&dom, element, check_prefix)?,
            "{xml}"
        );
    }
    Ok(())
}

fn getDataForPhpize() -> Vec<(Value, &'static str)> {
    vec![
        (s(""), ""),
        (Value::Null, "null"),
        (Value::Bool(true), "true"),
        (Value::Bool(false), "false"),
        (Value::Null, "Null"),
        (Value::Bool(true), "True"),
        (Value::Bool(false), "False"),
        (Value::Int(0), "0"),
        (Value::Int(1), "1"),
        (Value::Int(-1), "-1"),
        (Value::Int(0o777), "0777"),
        (Value::Int(-511), "-0777"),
        (s("0877"), "0877"),
        (Value::Int(255), "0xFF"),
        (Value::Float(100.0), "1e2"),
        (Value::Float(-120.0), "-1.2E2"),
        (Value::Float(-10100.1), "-10100.1"),
        (s("-10,100.1"), "-10,100.1"),
        (s("1234 5678 9101 1121 3141"), "1234 5678 9101 1121 3141"),
        (s("1,2,3,4"), "1,2,3,4"),
        (s("11,22,33,44"), "11,22,33,44"),
        (s("11,222,333,4"), "11,222,333,4"),
        (s("1,222,333,444"), "1,222,333,444"),
        (s("11,222,333,444"), "11,222,333,444"),
        (s("111,222,333,444"), "111,222,333,444"),
        (s("1111,2222,3333,4444,5555"), "1111,2222,3333,4444,5555"),
        (s("foo"), "foo"),
        (Value::Int(6), "0b0110"),
    ]
}

#[test]
fn testPhpize() {
    for (expected, value) in getDataForPhpize() {
        // assertSame: the type and the value
        assert_eq!(expected, XmlUtils::phpize(value.as_bytes()), "{value:?}");
    }
}

fn foo_xml() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/foo.xml")
}

#[test]
fn testLoadEmptyXmlFile() {
    let file = foo_xml();

    expect_exception(
        XmlUtils::load_file(&file, None),
        Class::InvalidArgument,
        &format!(
            "File \"{}\" does not contain valid XML, it is empty.",
            file.display()
        ),
    );
}

// test for issue https://github.com/symfony/symfony/issues/9731
#[test]
fn testLoadWrongEmptyXMLWithErrorHandler() -> TestResult {
    // set_error_handler() turning warnings into exceptions: nothing here
    // raises PHP warnings, so the handler has nothing to catch.
    let file = foo_xml();
    match XmlUtils::load_file(&file, None) {
        Ok(_) => panic!("An exception should have been raised"),
        Err(e) => {
            assert!(e.class().is_a(&Class::InvalidArgument));
            assert_eq!(
                format!(
                    "File \"{}\" does not contain valid XML, it is empty.",
                    file.display()
                ),
                message(&e)
            );
        }
    }

    // should not throw an exception
    let schema = fixtures().join("schema.xsd");
    XmlUtils::load_file(
        &fixtures().join("valid.xml"),
        Some(Validation::Schema(&schema)),
    )?;
    Ok(())
}
