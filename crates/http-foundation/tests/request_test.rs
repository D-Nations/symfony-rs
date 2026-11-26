//! RequestTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's; a data provider's rows run in its test, with
//! the row in the failure message. `tearDown()` and the tests' reflection
//! resets are `Request::reset_statics()`, which a guard runs as the test
//! ends. `assertEquals(80, $port)` compares loosely, as PHPUnit does.
//!
//! Upstream: symfony/http-foundation Tests/RequestTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
// the providers' rows are tuples of slices, as PHPUnit's are arrays
#![allow(clippy::type_complexity)]

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, compare::loose_equals, php_array};
use symfony_http_foundation::{
    exception::Error,
    file_bag::FileBag,
    header_bag::HeaderBag,
    input_bag::InputBag,
    ip_utils,
    parameter_bag::ParameterBag,
    php::{Globals, Stream, set_globals, with_globals},
    request::{Content, ContentValue, Create, Duplicate, Parameters, Request},
    server_bag::ServerBag,
    session::{
        Session, session_interface::SessionInterface,
        storage::mock_array_session_storage::MockArraySessionStorage,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `tearDown()`.
struct TearDown;

impl Drop for TearDown {
    fn drop(&mut self) {
        Request::reset_statics();
        set_globals(Globals::default());
    }
}

fn s(text: &str) -> PhpString {
    PhpString::from(text)
}

fn some(text: &str) -> Option<PhpString> {
    Some(s(text))
}

fn text(value: &PhpString) -> String {
    value.to_string_lossy().into_owned()
}

fn server(entries: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in entries {
        array.insert(*key, Value::string(*value));
    }
    array
}

fn with_server(entries: &[(&str, &str)]) -> Result<Request, Error> {
    Request::new(Parameters {
        server: server(entries),
        ..Parameters::default()
    })
}

fn initialize_server(request: &mut Request, entries: &[(&str, &str)]) -> Result<(), Error> {
    request.initialize(Parameters {
        server: server(entries),
        ..Parameters::default()
    })
}

fn create_with_server(uri: &str, method: &str, entries: &[(&str, &str)]) -> Result<Request, Error> {
    Request::create_with(
        uri,
        Create {
            method: Some(s(method)),
            server: server(entries),
            ..Create::default()
        },
    )
}

fn create_get(uri: &str, parameters: PhpArray) -> Result<Request, Error> {
    Request::create_with(
        uri,
        Create {
            method: Some(s("GET")),
            parameters,
            ..Create::default()
        },
    )
}

/// `assertEquals($expected, $value)` for an int.
fn assert_int(expected: i64, value: &Value, message: &str) {
    assert!(
        loose_equals(&Value::Int(expected), value),
        "{message}: expected {expected}, got {value:?}"
    );
}

fn set_header(request: &mut Request, name: &str, value: &str) -> Result<(), Error> {
    request.headers_mut().set(name.as_bytes(), value, true)
}

fn set_server(request: &mut Request, key: &str, value: Value) {
    request.server_mut().set(key, value);
}

fn expect_err<T: std::fmt::Debug>(result: Result<T, Error>, class: &str, message: &str) {
    match result {
        Ok(value) => panic!("expected {class}, got {value:?}"),
        Err(e) => {
            assert!(e.is_a(class), "expected {class}, got {e}");
            assert!(
                text(e.message()).contains(message),
                "expected a message containing {message:?}, got {e}"
            );
        }
    }
}

const BAD_REQUEST: &str = "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException";
const SUSPICIOUS: &str =
    "Symfony\\Component\\HttpFoundation\\Exception\\SuspiciousOperationException";
const CONFLICTING: &str =
    "Symfony\\Component\\HttpFoundation\\Exception\\ConflictingHeadersException";
const JSON: &str = "Symfony\\Component\\HttpFoundation\\Exception\\JsonException";

#[test]
fn testInitialize() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();

    request.initialize(Parameters {
        query: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    assert_eq!(
        Value::string("bar"),
        request.query().get("foo", Value::Null)?
    );

    request.initialize(Parameters {
        request: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    assert_eq!(
        Value::string("bar"),
        request.request().get("foo", Value::Null)?
    );

    request.initialize(Parameters {
        attributes: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    assert_eq!(
        Value::string("bar"),
        request.attributes().get("foo", Value::Null)
    );

    initialize_server(&mut request, &[("HTTP_FOO", "bar")])?;
    assert_eq!(some("bar"), request.headers().get("FOO", None));
    Ok(())
}

#[test]
fn testGetLocale() {
    let _t = TearDown;
    let mut request = Request::default();
    request.set_locale("pl");
    assert_eq!(s("pl"), request.get_locale());
}

#[test]
fn testGetUser() -> TestResult {
    let _t = TearDown;
    let request = Request::create("http://user:password@test.com")?;
    assert_eq!(some("user"), request.get_user());
    Ok(())
}

#[test]
fn testGetPassword() -> TestResult {
    let _t = TearDown;
    let request = Request::create("http://user:password@test.com")?;
    assert_eq!(some("password"), request.get_password());
    Ok(())
}

#[test]
fn testIsNoCache() {
    let _t = TearDown;
    let request = Request::default();
    assert!(!request.is_no_cache());
}

#[test]
fn testGetContentTypeFormat() -> TestResult {
    let _t = TearDown;
    let request = Request::default();
    assert_eq!(None, request.get_content_type_format());

    let request = with_server(&[("HTTP_CONTENT_TYPE", "application/json")])?;
    assert_eq!(some("json"), request.get_content_type_format());

    let request = with_server(&[("HTTP_CONTENT_TYPE", "text/html")])?;
    assert_eq!(some("html"), request.get_content_type_format());
    Ok(())
}

#[test]
fn testSetDefaultLocale() {
    let _t = TearDown;
    let mut request = Request::default();
    request.set_default_locale("pl");
    assert_eq!(s("pl"), request.get_locale());
}

/// The assertions `testCreate()` repeats for each URI.
#[allow(clippy::too_many_arguments)]
fn check(
    request: &mut Request,
    uri: &str,
    path_info: &str,
    query_string: Option<&str>,
    port: i64,
    http_host: &str,
    secure: bool,
) -> TestResult {
    assert_eq!(uri, text(&request.get_uri()?));
    assert_eq!(path_info, text(&request.get_path_info()?));
    // assertEquals('', null) holds, as does assertEquals('bar=baz', 'bar=baz')
    assert_eq!(
        query_string.unwrap_or_default(),
        request
            .get_query_string()?
            .as_ref()
            .map(text)
            .unwrap_or_default()
    );
    assert_int(port, &request.get_port()?, uri);
    assert_eq!(http_host, text(&request.get_http_host()?));
    assert_eq!(secure, request.is_secure()?);
    Ok(())
}

#[test]
fn testCreate() -> TestResult {
    let _t = TearDown;
    let mut request = Request::create("http://test.com/foo?bar=baz")?;
    check(
        &mut request,
        "http://test.com/foo?bar=baz",
        "/foo",
        Some("bar=baz"),
        80,
        "test.com",
        false,
    )?;

    let mut request = create_get("http://test.com/foo", php_array!["bar" => "baz"])?;
    check(
        &mut request,
        "http://test.com/foo?bar=baz",
        "/foo",
        Some("bar=baz"),
        80,
        "test.com",
        false,
    )?;

    let mut request = create_get("http://test.com/foo?bar=foo", php_array!["bar" => "baz"])?;
    check(
        &mut request,
        "http://test.com/foo?bar=baz",
        "/foo",
        Some("bar=baz"),
        80,
        "test.com",
        false,
    )?;

    let mut request = Request::create("https://test.com/foo?foo.bar=baz")?;
    check(
        &mut request,
        "https://test.com/foo?foo.bar=baz",
        "/foo",
        Some("foo.bar=baz"),
        443,
        "test.com",
        true,
    )?;
    assert_eq!(&php_array!["foo_bar" => "baz"], request.query().all());

    let mut request = Request::create("test.com:90/foo")?;
    assert_eq!("http://test.com:90/foo", text(&request.get_uri()?));
    assert_eq!("/foo", text(&request.get_path_info()?));
    assert_eq!("test.com", text(&request.get_host()?));
    assert_eq!("test.com:90", text(&request.get_http_host()?));
    assert_int(90, &request.get_port()?, "test.com:90/foo");
    assert!(!request.is_secure()?);

    for (uri, host, http_host, port) in [
        ("https://test.com:90/foo", "test.com", "test.com:90", 90),
        ("https://127.0.0.1:90/foo", "127.0.0.1", "127.0.0.1:90", 90),
        ("https://[::1]:90/foo", "[::1]", "[::1]:90", 90),
        ("https://[::1]/foo", "[::1]", "[::1]", 443),
    ] {
        let mut request = Request::create(uri)?;
        assert_eq!(uri, text(&request.get_uri()?));
        assert_eq!("/foo", text(&request.get_path_info()?));
        assert_eq!(host, text(&request.get_host()?));
        assert_eq!(http_host, text(&request.get_http_host()?));
        assert_int(port, &request.get_port()?, uri);
        assert!(request.is_secure()?);
    }

    let json = r#"{"jsonrpc":"2.0","method":"echo","id":7,"params":["Hello World"]}"#;
    let mut request = Request::create_with(
        "http://example.com/jsonrpc",
        Create {
            method: Some(s("POST")),
            content: Content::from(json),
            ..Create::default()
        },
    )?;
    let ContentValue::String(content) = request.get_content(false)? else {
        panic!("a string");
    };
    assert_eq!(json, text(&content));
    assert!(!request.is_secure()?);

    let mut request = Request::create("http://test.com")?;
    check(
        &mut request,
        "http://test.com/",
        "/",
        Some(""),
        80,
        "test.com",
        false,
    )?;

    let mut request = Request::create("http://test.com?test=1")?;
    check(
        &mut request,
        "http://test.com/?test=1",
        "/",
        Some("test=1"),
        80,
        "test.com",
        false,
    )?;

    let mut request = Request::create("http://test.com:90/?test=1")?;
    check(
        &mut request,
        "http://test.com:90/?test=1",
        "/",
        Some("test=1"),
        90,
        "test.com:90",
        false,
    )?;

    let mut request = Request::create("http://username:password@test.com")?;
    check(
        &mut request,
        "http://test.com/",
        "/",
        Some(""),
        80,
        "test.com",
        false,
    )?;
    assert_eq!(some("username"), request.get_user());
    assert_eq!(some("password"), request.get_password());

    let mut request = Request::create("http://username@test.com")?;
    check(
        &mut request,
        "http://test.com/",
        "/",
        Some(""),
        80,
        "test.com",
        false,
    )?;
    assert_eq!(some("username"), request.get_user());
    assert_eq!(some(""), request.get_password());

    let mut request = Request::create("http://test.com/?foo")?;
    assert_eq!("/?foo", text(&request.get_request_uri()?));
    assert_eq!(&php_array!["foo" => ""], request.query().all());

    // assume rewrite rule: (.*) --> app/app.php; app/ is a symlink to a symfony web/ directory
    let mut request = create_with_server(
        "http://test.com/apparthotel-1234",
        "GET",
        &[
            ("DOCUMENT_ROOT", "/var/www/www.test.com"),
            ("SCRIPT_FILENAME", "/var/www/www.test.com/app/app.php"),
            ("SCRIPT_NAME", "/app/app.php"),
            ("PHP_SELF", "/app/app.php/apparthotel-1234"),
        ],
    )?;
    check(
        &mut request,
        "http://test.com/apparthotel-1234",
        "/apparthotel-1234",
        Some(""),
        80,
        "test.com",
        false,
    )?;

    // Fragment should not be included in the URI
    let mut request = Request::create("http://test.com/foo#bar")?;
    assert_eq!("http://test.com/foo", text(&request.get_uri()?));
    Ok(())
}

fn post_with_override(header: &str) -> Result<Request, Error> {
    let mut request = Request::create_with(
        "http://example.com/",
        Create {
            method: Some(s("POST")),
            ..Create::default()
        },
    )?;
    set_header(&mut request, "X-HTTP-METHOD-OVERRIDE", header)?;
    Ok(request)
}

#[test]
fn testHttpMethodOverrideRespectsAllowedListWithHeader() -> TestResult {
    let _t = TearDown;
    let mut request = post_with_override("PATCH")?;
    Request::set_allowed_http_method_override(Some(["PUT", "PATCH"]))?;
    assert_eq!(s("PATCH"), request.get_method()?);
    Ok(())
}

#[test]
fn testHttpMethodOverrideDisallowedSkipsOverrideWithHeader() -> TestResult {
    let _t = TearDown;
    let mut request = post_with_override("DELETE")?;
    Request::set_allowed_http_method_override(Some(["PUT", "PATCH"]))?;
    assert_eq!(s("POST"), request.get_method()?);
    Ok(())
}

#[test]
fn testHttpMethodOverrideDisabledWithEmptyAllowedList() -> TestResult {
    let _t = TearDown;
    let mut request = post_with_override("PUT")?;
    Request::set_allowed_http_method_override(Some(Vec::<&str>::new()))?;
    assert_eq!(s("POST"), request.get_method()?);
    Ok(())
}

#[test]
fn testHttpMethodOverrideRespectsAllowedListWithParameter() -> TestResult {
    let _t = TearDown;
    Request::enable_http_method_parameter_override();
    Request::set_allowed_http_method_override(Some(["PUT"]))?;
    let mut request = Request::create_with(
        "http://example.com/",
        Create {
            method: Some(s("POST")),
            parameters: php_array!["_method" => "PUT"],
            ..Create::default()
        },
    )?;
    assert_eq!(s("PUT"), request.get_method()?);
    Ok(())
}

#[test]
fn testHttpMethodOverrideCannotBeAppliedToCertainMethods() {
    let _t = TearDown;
    for method in ["CONNECT", "GET", "HEAD", "TRACE"] {
        expect_err(
            Request::set_allowed_http_method_override(Some([method])),
            "InvalidArgumentException",
            "",
        );
    }
}

#[test]
fn testCreateWithRequestUri() -> TestResult {
    let _t = TearDown;
    for (uri, expected, host, http_host, port, secure) in [
        (
            "http://test.com:80/foo",
            "http://test.com/foo",
            "test.com",
            "test.com",
            80,
            false,
        ),
        (
            "http://test.com:8080/foo",
            "http://test.com:8080/foo",
            "test.com",
            "test.com:8080",
            8080,
            false,
        ),
        (
            "https://test.com:443/foo",
            "https://test.com/foo",
            "test.com",
            "test.com",
            443,
            true,
        ),
    ] {
        let mut request = Request::create(uri)?;
        set_server(&mut request, "REQUEST_URI", Value::string(uri));
        assert_eq!(expected, text(&request.get_uri()?));
        assert_eq!("/foo", text(&request.get_path_info()?));
        assert_eq!(host, text(&request.get_host()?));
        assert_eq!(http_host, text(&request.get_http_host()?));
        assert_int(port, &request.get_port()?, uri);
        assert_eq!(secure, request.is_secure()?);
    }

    let mut request = create_get("http://test.com/foo?bar=foo", php_array!["bar" => "baz"])?;
    set_server(
        &mut request,
        "REQUEST_URI",
        Value::string("http://test.com/foo?bar=foo"),
    );
    assert_eq!("http://test.com/foo?bar=baz", text(&request.get_uri()?));
    assert_eq!("/foo", text(&request.get_path_info()?));
    assert_eq!(some("bar=baz"), request.get_query_string()?);
    assert_eq!("test.com", text(&request.get_host()?));
    assert_eq!("test.com", text(&request.get_http_host()?));
    assert_int(80, &request.get_port()?, "bar=foo");
    assert!(!request.is_secure()?);

    // Fragment should not be included in the URI
    let mut request = Request::create(r"http://test.com/foo#bar\baz")?;
    set_server(
        &mut request,
        "REQUEST_URI",
        Value::string(r"http://test.com/foo#bar\baz"),
    );
    assert_eq!("http://test.com/foo", text(&request.get_uri()?));

    let mut request = Request::create(r"http://test.com/foo?bar=f\o")?;
    assert_eq!("http://test.com/foo?bar=f%5Co", text(&request.get_uri()?));
    assert_eq!("/foo", text(&request.get_path_info()?));
    assert_eq!(some("bar=f%5Co"), request.get_query_string()?);
    Ok(())
}

#[test]
fn testCreateWithBadRequestUri() {
    let _t = TearDown;
    for uri in [
        r"http://foo.com\bar",
        r"\\foo.com/bar",
        "a\rb",
        "a\nb",
        "a\tb",
        "\u{0}foo",
        "foo\u{0}",
        " foo",
        "foo ",
        "//",
    ] {
        expect_err(Request::create(uri), BAD_REQUEST, "Invalid URI");
    }
}

#[test]
fn testGetRequestUri() -> TestResult {
    let _t = TearDown;
    let modify = "Do not modify the path.";
    let host = "Handle when the scheme, host are on REQUEST_URI.";
    let port = "Handle when the scheme, host and port are on REQUEST_URI.";
    let fragment = "Fragment should not be included in the URI";
    for (server_request_uri, expected, message) in [
        ("/foo", "/foo", modify),
        ("//bar/foo", "//bar/foo", modify),
        ("///bar/foo", "///bar/foo", modify),
        ("http://test.com/foo?bar=baz", "/foo?bar=baz", host),
        ("http://test.com:80/foo", "/foo", port),
        ("https://test.com:8080/foo", "/foo", port),
        ("https://test.com:443/foo", "/foo", port),
        ("http://test.com/foo#bar", "/foo", fragment),
        ("/foo#bar", "/foo", fragment),
    ] {
        let mut request = Request::default();
        request.server_mut().add(php_array![
            "REQUEST_URI" => server_request_uri,
            "SERVER_NAME" => "test.com",
            "SERVER_PORT" => 80,
        ]);
        assert_eq!(expected, text(&request.get_request_uri()?), "{message}");
        assert_eq!(
            Value::string(expected),
            request.server().get("REQUEST_URI", Value::Null),
            "Normalize the request URI."
        );
    }
    Ok(())
}

#[test]
fn testGetRequestUriWithoutRequiredHeader() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(
        "",
        text(&request.get_request_uri()?),
        "Fallback to empty URI when headers are missing."
    );
    assert_eq!(
        Value::string(""),
        request.server().get("REQUEST_URI", Value::Null),
        "Normalize the request URI."
    );
    Ok(())
}

#[test]
fn testCreateCheckPrecedence() -> TestResult {
    let _t = TearDown;
    // server is used by default
    let mut request = Request::create_with(
        "/",
        Create {
            method: Some(s("DELETE")),
            server: php_array![
                "HTTP_HOST" => "example.com",
                "HTTPS" => "on",
                "SERVER_PORT" => 443,
                "PHP_AUTH_USER" => "fabien",
                "PHP_AUTH_PW" => "pa$$",
                "QUERY_STRING" => "foo=bar",
                "CONTENT_TYPE" => "application/json",
            ],
            ..Create::default()
        },
    )?;
    assert_eq!("example.com", text(&request.get_host()?));
    assert_int(443, &request.get_port()?, "server");
    assert!(request.is_secure()?);
    assert_eq!(some("fabien"), request.get_user());
    assert_eq!(some("pa$$"), request.get_password());
    assert_eq!(None, request.get_query_string()?);
    assert_eq!(
        some("application/json"),
        request.headers().get("CONTENT_TYPE", None)
    );

    // URI has precedence over server
    let mut request = Request::create_with(
        "http://thomas:pokemon@example.net:8080/?foo=bar",
        Create {
            method: Some(s("GET")),
            server: php_array!["HTTP_HOST" => "example.com", "HTTPS" => "on", "SERVER_PORT" => 443],
            ..Create::default()
        },
    )?;
    assert_eq!("example.net", text(&request.get_host()?));
    assert_int(8080, &request.get_port()?, "URI");
    assert!(!request.is_secure()?);
    assert_eq!(some("thomas"), request.get_user());
    assert_eq!(some("pokemon"), request.get_password());
    assert_eq!(some("foo=bar"), request.get_query_string()?);
    Ok(())
}

#[test]
fn testDuplicate() -> TestResult {
    let _t = TearDown;
    let mut request = Request::new(Parameters {
        query: php_array!["foo" => "bar"],
        request: php_array!["foo" => "bar"],
        attributes: php_array!["foo" => "bar"],
        server: server(&[("HTTP_FOO", "bar")]),
        ..Parameters::default()
    })?;
    let dup = request.duplicate(Duplicate::default())?;
    assert_eq!(request.query().all(), dup.query().all());
    assert_eq!(request.request().all(), dup.request().all());
    assert_eq!(request.attributes().all(), dup.attributes().all());
    assert_eq!(request.headers().all(), dup.headers().all());

    let dup = request.duplicate(Duplicate {
        query: Some(php_array!["foo" => "foobar"]),
        request: Some(php_array!["foo" => "foobar"]),
        attributes: Some(php_array!["foo" => "foobar"]),
        cookies: Some(PhpArray::new()),
        files: Some(PhpArray::new()),
        server: Some(server(&[("HTTP_FOO", "foobar")])),
    })?;
    assert_eq!(&php_array!["foo" => "foobar"], dup.query().all());
    assert_eq!(&php_array!["foo" => "foobar"], dup.request().all());
    assert_eq!(&php_array!["foo" => "foobar"], dup.attributes().all());
    assert_eq!(
        HeaderBag::new([("foo", "foobar")]).all(),
        dup.headers().all()
    );
    Ok(())
}

#[test]
fn testDuplicateWithFormat() -> TestResult {
    let _t = TearDown;
    let mut request = Request::new(Parameters {
        attributes: php_array!["_format" => "json"],
        ..Parameters::default()
    })?;
    let mut dup = request.duplicate(Duplicate::default())?;
    assert_eq!(some("json"), dup.get_request_format(Some("html"))?);
    assert_eq!(
        Value::string("json"),
        dup.attributes().get("_format", Value::Null)
    );

    let mut request = Request::default();
    request.set_request_format(some("xml"));
    let mut dup = request.duplicate(Duplicate::default())?;
    assert_eq!(some("xml"), dup.get_request_format(Some("html"))?);
    Ok(())
}

#[test]
fn testGetPreferredFormat() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(None, request.get_preferred_format(None)?);
    assert_eq!(some("html"), request.get_preferred_format(Some("html"))?);
    assert_eq!(some("json"), request.get_preferred_format(Some("json"))?);

    request.set_request_format(some("atom"));
    set_header(&mut request, "Accept", "application/ld+json")?;
    assert_eq!(some("atom"), request.get_preferred_format(Some("html"))?);

    for accept in [
        "application/xml",
        "application/xml",
        "application/json;q=0.8,application/xml;q=0.9",
    ] {
        let mut request = Request::default();
        set_header(&mut request, "Accept", accept)?;
        assert_eq!(some("xml"), request.get_preferred_format(Some("html"))?);
    }
    Ok(())
}

const FORMAT_TO_MIME_TYPES: [(&str, &[&str]); 17] = [
    ("txt", &["text/plain"]),
    (
        "js",
        &[
            "application/javascript",
            "application/x-javascript",
            "text/javascript",
        ],
    ),
    ("css", &["text/css"]),
    ("json", &["application/json", "application/x-json"]),
    ("jsonld", &["application/ld+json"]),
    ("xml", &["text/xml", "application/xml", "application/x-xml"]),
    ("rdf", &["application/rdf+xml"]),
    ("atom", &["application/atom+xml"]),
    (
        "form",
        &["application/x-www-form-urlencoded", "multipart/form-data"],
    ),
    ("rss", &["application/rss+xml"]),
    ("soap", &["application/soap+xml"]),
    ("html", &["text/html", "application/xhtml+xml"]),
    ("problem", &["application/problem+json"]),
    ("hal", &["application/hal+json", "application/hal+xml"]),
    ("jsonapi", &["application/vnd.api+json"]),
    ("yaml", &["text/yaml", "application/x-yaml"]),
    ("wbxml", &["application/vnd.wap.wbxml"]),
];

#[test]
fn testGetFormatFromMimeType() {
    for (format, mime_types) in FORMAT_TO_MIME_TYPES {
        let _t = TearDown;
        let request = Request::default();
        for mime in mime_types {
            assert_eq!(
                some(format),
                request.get_format(Some(mime.as_bytes()), false)
            );
        }
        request.set_format(format, mime_types.iter().copied());
        for mime in mime_types {
            assert_eq!(
                some(format),
                request.get_format(Some(mime.as_bytes()), false)
            );
            assert_eq!(
                some(mime_types[0]),
                request.get_mime_type(format.as_bytes())
            );
        }
    }
}

#[test]
fn testGetFormatFromMimeTypeWithParameters() {
    let _t = TearDown;
    let request = Request::default();
    for mime in [
        "application/json; charset=utf-8",
        "application/json;charset=utf-8",
        "application/json ; charset=utf-8",
        "application/json ;charset=utf-8",
    ] {
        assert_eq!(
            some("json"),
            request.get_format(Some(mime.as_bytes()), false)
        );
    }
}

#[test]
fn testGetMimeTypeFromFormat() {
    for (format, mime_types) in FORMAT_TO_MIME_TYPES {
        let _t = TearDown;
        let request = Request::default();
        assert_eq!(
            some(mime_types[0]),
            request.get_mime_type(format.as_bytes())
        );
    }
}

#[test]
fn testGetMimeTypesFromFormat() {
    for (format, mime_types) in FORMAT_TO_MIME_TYPES {
        let _t = TearDown;
        let expected: Vec<PhpString> = mime_types.iter().map(|m| s(m)).collect();
        assert_eq!(expected, Request::get_mime_types(format.as_bytes()));
    }
}

#[test]
fn testGetMimeTypesFromInexistentFormat() {
    let _t = TearDown;
    let request = Request::default();
    assert_eq!(None, request.get_mime_type(b"foo"));
    assert_eq!(
        Request::get_mime_types(b"foo"),
        [] as [php_value::PhpString; 0]
    );
}

#[test]
fn testGetFormatWithCustomMimeType() {
    let _t = TearDown;
    let request = Request::default();
    request.set_format("custom", ["application/vnd.foo.api;myversion=2.3"]);
    assert_eq!(
        some("custom"),
        request.get_format(Some(b"application/vnd.foo.api;myversion=2.3"), false)
    );
}

#[test]
fn testGetFormatFromMimeTypeWithSubtypeFallback() {
    let _t = TearDown;
    for (expected, mime_types) in [
        ("cbor", &["application/example+cbor"][..]),
        (
            "asn1",
            &["application/ber-stream+ber", "application/secure-data+der"],
        ),
        ("zip", &["application/foobar+zip"]),
        ("tlv", &["application/device-config+tlv"]),
        ("pdf", &["application/pdf"]),
        ("csv", &["text/csv"]),
    ] {
        let request = Request::default();
        for mime in mime_types {
            assert_eq!(
                some(expected),
                request.get_format(Some(mime.as_bytes()), true),
                "{mime}"
            );
        }
    }
}

#[test]
fn testGetUri() -> TestResult {
    let _t = TearDown;
    let standard = [
        ("QUERY_STRING", "query=string"),
        ("REQUEST_URI", "/index.php/path/info?query=string"),
        ("SCRIPT_NAME", "/index.php"),
        ("PATH_INFO", "/path/info"),
        ("PATH_TRANSLATED", "redirect:/index.php/path/info"),
        ("PHP_SELF", "/index_dev.php/path/info"),
        ("SCRIPT_FILENAME", "/some/where/index.php"),
    ];
    let mut request = Request::default();
    let mut entries = vec![
        ("HTTP_HOST", "host:8080"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "8080"),
    ];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/index.php/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with non default port"
    );

    let mut entries = vec![
        ("HTTP_HOST", "host"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "80"),
    ];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host/index.php/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with default port"
    );

    let mut entries = vec![("SERVER_NAME", "servername"), ("SERVER_PORT", "80")];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/index.php/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with default port without HOST_HEADER"
    );

    // Request with URL REWRITING (hide index.php)
    let rewrite = [
        ("REDIRECT_QUERY_STRING", "query=string"),
        ("REDIRECT_URL", "/path/info"),
        ("QUERY_STRING", "query=string"),
        ("REQUEST_URI", "/path/info?toto=test&1=1"),
        ("SCRIPT_NAME", "/index.php"),
        ("PHP_SELF", "/index.php"),
        ("SCRIPT_FILENAME", "/some/where/index.php"),
    ];
    let mut entries = vec![
        ("HTTP_HOST", "host:8080"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "8080"),
    ];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with rewrite"
    );

    let mut entries = vec![
        ("HTTP_HOST", "host"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "80"),
    ];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with rewrite and default port"
    );

    let mut entries = vec![("SERVER_NAME", "servername"), ("SERVER_PORT", "80")];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/path/info?query=string",
        text(&request.get_uri()?),
        "->getUri() with rewrite, default port without HOST_HEADER"
    );

    // With encoded characters
    let mut entries = vec![
        ("HTTP_HOST", "host:8080"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "8080"),
        ("QUERY_STRING", "query=string"),
        (
            "REQUEST_URI",
            "/ba%20se/index_dev.php/foo%20bar/in+fo?query=string",
        ),
        ("SCRIPT_NAME", "/ba se/index_dev.php"),
        ("PATH_TRANSLATED", "redirect:/index.php/foo bar/in+fo"),
        ("PHP_SELF", "/ba se/index_dev.php/path/info"),
        ("SCRIPT_FILENAME", "/some/where/ba se/index_dev.php"),
    ];
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/ba%20se/index_dev.php/foo%20bar/in+fo?query=string",
        text(&request.get_uri()?)
    );

    // with user info
    entries.push(("PHP_AUTH_USER", "fabien"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/ba%20se/index_dev.php/foo%20bar/in+fo?query=string",
        text(&request.get_uri()?)
    );

    entries.push(("PHP_AUTH_PW", "symfony"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/ba%20se/index_dev.php/foo%20bar/in+fo?query=string",
        text(&request.get_uri()?)
    );
    Ok(())
}

#[test]
fn testGetUriForPath() -> TestResult {
    let _t = TearDown;
    for (uri, expected) in [
        ("http://test.com/foo?bar=baz", "http://test.com/some/path"),
        (
            "http://test.com:90/foo?bar=baz",
            "http://test.com:90/some/path",
        ),
        ("https://test.com/foo?bar=baz", "https://test.com/some/path"),
        (
            "https://test.com:90/foo?bar=baz",
            "https://test.com:90/some/path",
        ),
    ] {
        let mut request = Request::create(uri)?;
        assert_eq!(expected, text(&request.get_uri_for_path(b"/some/path")?));
    }

    let standard = [
        ("QUERY_STRING", "query=string"),
        ("REQUEST_URI", "/index.php/path/info?query=string"),
        ("SCRIPT_NAME", "/index.php"),
        ("PATH_INFO", "/path/info"),
        ("PATH_TRANSLATED", "redirect:/index.php/path/info"),
        ("PHP_SELF", "/index_dev.php/path/info"),
        ("SCRIPT_FILENAME", "/some/where/index.php"),
    ];
    let mut request = Request::default();
    let mut entries = vec![
        ("HTTP_HOST", "host:8080"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "8080"),
    ];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/index.php/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUriForPath() with non default port"
    );

    let mut entries = vec![
        ("HTTP_HOST", "host"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "80"),
    ];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host/index.php/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUriForPath() with default port"
    );

    let mut entries = vec![("SERVER_NAME", "servername"), ("SERVER_PORT", "80")];
    entries.extend(standard);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/index.php/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUriForPath() with default port without HOST_HEADER"
    );

    let rewrite = [
        ("REDIRECT_QUERY_STRING", "query=string"),
        ("REDIRECT_URL", "/path/info"),
        ("QUERY_STRING", "query=string"),
        ("REQUEST_URI", "/path/info?toto=test&1=1"),
        ("SCRIPT_NAME", "/index.php"),
        ("PHP_SELF", "/index.php"),
        ("SCRIPT_FILENAME", "/some/where/index.php"),
    ];
    let mut entries = vec![
        ("HTTP_HOST", "host:8080"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "8080"),
    ];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host:8080/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUri() with rewrite"
    );

    let mut entries = vec![
        ("HTTP_HOST", "host"),
        ("SERVER_NAME", "servername"),
        ("SERVER_PORT", "80"),
    ];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://host/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUriForPath() with rewrite and default port"
    );

    let mut entries = vec![("SERVER_NAME", "servername"), ("SERVER_PORT", "80")];
    entries.extend(rewrite);
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/some/path",
        text(&request.get_uri_for_path(b"/some/path")?),
        "->getUriForPath() with rewrite, default port without HOST_HEADER"
    );
    assert_eq!("servername", text(&request.get_http_host()?));

    // with user info
    entries.push(("PHP_AUTH_USER", "fabien"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/some/path",
        text(&request.get_uri_for_path(b"/some/path")?)
    );

    entries.push(("PHP_AUTH_PW", "symfony"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername/some/path",
        text(&request.get_uri_for_path(b"/some/path")?)
    );
    Ok(())
}

#[test]
fn testGetRelativeUriForPath() -> TestResult {
    let _t = TearDown;
    for (expected, pathinfo, path) in [
        ("me.png", "/foo", "/me.png"),
        ("../me.png", "/foo/bar", "/me.png"),
        ("me.png", "/foo/bar", "/foo/me.png"),
        ("../baz/me.png", "/foo/bar/b", "/foo/baz/me.png"),
        ("../../fooz/baz/me.png", "/foo/bar/b", "/fooz/baz/me.png"),
        ("baz/me.png", "/foo/bar/b", "baz/me.png"),
    ] {
        let mut request = Request::create(pathinfo)?;
        assert_eq!(
            expected,
            text(&request.get_relative_uri_for_path(path.as_bytes())?),
            "{pathinfo} {path}"
        );
    }
    Ok(())
}

#[test]
fn testGetUserInfo() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    initialize_server(&mut request, &[("PHP_AUTH_USER", "fabien")])?;
    assert_eq!(some("fabien"), request.get_user_info());

    initialize_server(&mut request, &[("PHP_AUTH_USER", "0")])?;
    assert_eq!(some("0"), request.get_user_info());

    initialize_server(
        &mut request,
        &[("PHP_AUTH_USER", "0"), ("PHP_AUTH_PW", "0")],
    )?;
    assert_eq!(some("0:0"), request.get_user_info());
    Ok(())
}

#[test]
fn testGetSchemeAndHttpHost() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    let mut entries = vec![("SERVER_NAME", "servername"), ("SERVER_PORT", "90")];
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername:90",
        text(&request.get_scheme_and_http_host()?)
    );

    entries.push(("PHP_AUTH_USER", "fabien"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername:90",
        text(&request.get_scheme_and_http_host()?)
    );

    entries[2] = ("PHP_AUTH_USER", "0");
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername:90",
        text(&request.get_scheme_and_http_host()?)
    );

    entries.push(("PHP_AUTH_PW", "0"));
    initialize_server(&mut request, &entries)?;
    assert_eq!(
        "http://servername:90",
        text(&request.get_scheme_and_http_host()?)
    );
    Ok(())
}

#[test]
fn testGetQueryString() -> TestResult {
    let _t = TearDown;
    for (query, expected, message) in [
        ("foo", "foo=", "works with valueless parameters"),
        ("foo=", "foo=", "includes a dangling equal sign"),
        (
            "bar=&foo=bar",
            "bar=&foo=bar",
            "->works with empty parameters",
        ),
        ("foo=bar&bar=", "bar=&foo=bar", "sorts keys alphabetically"),
        (
            "baz=Foo%20Baz&bar=Foo+Bar",
            "bar=Foo%20Bar&baz=Foo%20Baz",
            "normalizes spaces in both encodings \"%20\" and \"+\"",
        ),
        (
            "foo[]=1&foo[]=2",
            "foo%5B0%5D=1&foo%5B1%5D=2",
            "allows array notation",
        ),
        ("foo=1&foo=2", "foo=2", "merges repeated parameters"),
        (
            "pa%3Dram=foo%26bar%3Dbaz&test=test",
            "pa%3Dram=foo%26bar%3Dbaz&test=test",
            "works with encoded delimiters",
        ),
        ("0", "0=", "allows \"0\""),
        (
            "Foo Bar&Foo%20Baz",
            "Foo%20Bar=&Foo%20Baz=",
            "normalizes encoding in keys",
        ),
        (
            "bar=Foo Bar&baz=Foo%20Baz",
            "bar=Foo%20Bar&baz=Foo%20Baz",
            "normalizes encoding in values",
        ),
        (
            "foo=bar&&&test&&",
            "foo=bar&test=",
            "removes unneeded delimiters",
        ),
        (
            "formula=e=m*c^2",
            "formula=e%3Dm%2Ac%5E2",
            "correctly treats only the first \"=\" as delimiter and the next as value",
        ),
        (
            "foo=bar&=a=b&=x=y",
            "foo=bar",
            "removes params with empty key",
        ),
        (
            "foo[]=Z&foo[]=A",
            "foo%5B0%5D=Z&foo%5B1%5D=A",
            "keeps order of values",
        ),
        (
            "foo[Z]=B&foo[A]=B",
            "foo%5BZ%5D=B&foo%5BA%5D=B",
            "keeps order of keys",
        ),
        ("utf8=✓", "utf8=%E2%9C%93", "encodes UTF-8"),
    ] {
        let mut request = Request::default();
        set_server(&mut request, "QUERY_STRING", Value::string(query));
        assert_eq!(some(expected), request.get_query_string()?, "{message}");
    }
    Ok(())
}

#[test]
fn testGetQueryStringReturnsNull() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(
        None,
        request.get_query_string()?,
        "->getQueryString() returns null for non-existent query string"
    );
    set_server(&mut request, "QUERY_STRING", Value::string(""));
    assert_eq!(
        None,
        request.get_query_string()?,
        "->getQueryString() returns null for empty query string"
    );
    Ok(())
}

#[test]
fn testGetHost() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();

    request.initialize(Parameters {
        query: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    assert_eq!(
        "",
        text(&request.get_host()?),
        "->getHost() return empty string if not initialized"
    );

    initialize_server(&mut request, &[("HTTP_HOST", "www.example.com")])?;
    assert_eq!(
        "www.example.com",
        text(&request.get_host()?),
        "->getHost() from Host Header"
    );

    initialize_server(&mut request, &[("HTTP_HOST", "www.example.com:8080")])?;
    assert_eq!(
        "www.example.com",
        text(&request.get_host()?),
        "->getHost() from Host Header with port number"
    );

    initialize_server(&mut request, &[("SERVER_NAME", "www.example.com")])?;
    assert_eq!(
        "www.example.com",
        text(&request.get_host()?),
        "->getHost() from server name"
    );

    initialize_server(
        &mut request,
        &[
            ("SERVER_NAME", "www.example.com"),
            ("HTTP_HOST", "www.host.com"),
        ],
    )?;
    assert_eq!(
        "www.host.com",
        text(&request.get_host()?),
        "->getHost() value from Host header has priority over SERVER_NAME "
    );
    Ok(())
}

#[test]
fn testGetPort() -> TestResult {
    let _t = TearDown;
    let mut request = create_with_server(
        "http://example.com",
        "GET",
        &[
            ("HTTP_X_FORWARDED_PROTO", "https"),
            ("HTTP_X_FORWARDED_PORT", "443"),
        ],
    )?;
    assert_int(
        80,
        &request.get_port()?,
        "Without trusted proxies FORWARDED_PROTO and FORWARDED_PORT are ignored.",
    );

    Request::set_trusted_proxies(
        ["1.1.1.1"],
        Request::HEADER_X_FORWARDED_PROTO | Request::HEADER_X_FORWARDED_PORT,
    )?;
    let mut request = create_with_server(
        "http://example.com",
        "GET",
        &[
            ("HTTP_X_FORWARDED_PROTO", "https"),
            ("HTTP_X_FORWARDED_PORT", "8443"),
        ],
    )?;
    assert_int(
        80,
        &request.get_port()?,
        "With PROTO and PORT on untrusted connection server value takes precedence.",
    );
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    assert_int(
        8443,
        &request.get_port()?,
        "With PROTO and PORT set PORT takes precedence.",
    );

    for (proto, trusted_port, message) in [
        (
            "https",
            443,
            "With only PROTO set getPort() defaults to 443.",
        ),
        (
            "http",
            80,
            "If X_FORWARDED_PROTO is set to HTTP getPort() returns port of the original request.",
        ),
        (
            "On",
            443,
            "With only PROTO set and value is On, getPort() defaults to 443.",
        ),
        (
            "1",
            443,
            "With only PROTO set and value is 1, getPort() defaults to 443.",
        ),
    ] {
        let mut request = create_with_server(
            "http://example.com",
            "GET",
            &[("HTTP_X_FORWARDED_PROTO", proto)],
        )?;
        assert_int(
            80,
            &request.get_port()?,
            "ignores trusted headers on untrusted connection",
        );
        set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
        assert_int(trusted_port, &request.get_port()?, message);
    }

    let mut request = create_with_server(
        "http://example.com",
        "GET",
        &[("HTTP_X_FORWARDED_PROTO", "something-else")],
    )?;
    assert_int(
        80,
        &request.get_port()?,
        "With only PROTO set and value is not recognized, getPort() defaults to 80.",
    );
    Ok(())
}

#[test]
fn testGetHostWithFakeHttpHostValue() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    initialize_server(&mut request, &[("HTTP_HOST", "www.host.com?query=string")])?;
    expect_err(request.get_host(), "RuntimeException", "");
    Ok(())
}

#[test]
fn testGetSetMethod() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(
        s("GET"),
        request.get_method()?,
        "->getMethod() returns GET if no method is defined"
    );

    request.set_method("get");
    assert_eq!(
        s("GET"),
        request.get_method()?,
        "->getMethod() returns an uppercased string"
    );

    request.set_method("PURGE");
    assert_eq!(
        s("PURGE"),
        request.get_method()?,
        "->getMethod() returns the method even if it is not a standard one"
    );

    request.set_method("POST");
    assert_eq!(
        s("POST"),
        request.get_method()?,
        "->getMethod() returns the method POST if no _method is defined"
    );

    request.set_method("POST");
    request
        .request_mut()
        .set("_method", Value::string("purge"))?;
    assert_eq!(
        s("POST"),
        request.get_method()?,
        "->getMethod() does not return the method from _method if defined and POST but support not enabled"
    );

    let mut request = Request::default();
    request.set_method("POST");
    request
        .request_mut()
        .set("_method", Value::string("purge"))?;
    assert!(
        !Request::get_http_method_parameter_override(),
        "httpMethodParameterOverride should be disabled by default"
    );
    Request::enable_http_method_parameter_override();
    assert!(
        Request::get_http_method_parameter_override(),
        "httpMethodParameterOverride should be enabled now but it is not"
    );
    assert_eq!(
        s("PURGE"),
        request.get_method()?,
        "->getMethod() returns the method from _method if defined and POST"
    );
    Request::reset_statics();

    let mut request = Request::default();
    request.set_method("POST");
    request.query_mut().set("_method", Value::string("purge"))?;
    assert_eq!(
        s("POST"),
        request.get_method()?,
        "->getMethod() does not return the method from _method if defined and POST but support not enabled"
    );

    let mut request = Request::default();
    request.set_method("POST");
    request.query_mut().set("_method", Value::string("purge"))?;
    Request::enable_http_method_parameter_override();
    assert_eq!(
        s("PURGE"),
        request.get_method()?,
        "->getMethod() returns the method from _method if defined and POST"
    );
    Request::reset_statics();

    for message in [
        "->getMethod() returns the method from X-HTTP-Method-Override even though _method is set if defined and POST",
        "->getMethod() returns the method from X-HTTP-Method-Override if defined and POST",
    ] {
        let mut request = Request::default();
        request.set_method("POST");
        set_header(&mut request, "X-HTTP-METHOD-OVERRIDE", "delete")?;
        assert_eq!(s("DELETE"), request.get_method()?, "{message}");
    }

    let mut request = Request::default();
    request.set_method("POST");
    request
        .query_mut()
        .set("_method", Value::Array(php_array!["delete", "patch"]))?;
    assert_eq!(
        s("POST"),
        request.get_method()?,
        "->getMethod() returns the request method if invalid type is defined in query"
    );
    Ok(())
}

#[test]
fn testUnsafeMethodOverride() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    request.set_method("POST");
    set_header(&mut request, "X-HTTP-METHOD-OVERRIDE", "get")?;
    assert_eq!(s("POST"), request.get_method()?);
    Ok(())
}

type IpRow = (
    &'static [&'static str],
    &'static str,
    Option<&'static str>,
    Option<&'static [&'static str]>,
);

const CLIENT_IPS: [IpRow; 28] = [
    // simple IPv4
    (&["88.88.88.88"], "88.88.88.88", None, None),
    // trust the IPv4 remote addr
    (
        &["88.88.88.88"],
        "88.88.88.88",
        None,
        Some(&["88.88.88.88"]),
    ),
    // simple IPv6
    (&["::1"], "::1", None, None),
    // trust the IPv6 remote addr
    (&["::1"], "::1", None, Some(&["::1"])),
    // forwarded for with remote IPv4 addr not trusted
    (&["127.0.0.1"], "127.0.0.1", Some("88.88.88.88"), None),
    // forwarded for with remote IPv4 addr trusted + comma
    (
        &["88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88,"),
        Some(&["127.0.0.1"]),
    ),
    // forwarded for with remote IPv4 and all FF addrs trusted
    (
        &["88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88"),
        Some(&["127.0.0.1", "88.88.88.88"]),
    ),
    // forwarded for with remote IPv4 range trusted
    (
        &["88.88.88.88"],
        "123.45.67.89",
        Some("88.88.88.88"),
        Some(&["123.45.67.0/24"]),
    ),
    // forwarded for with remote IPv6 addr not trusted
    (
        &["1620:0:1cfe:face:b00c::3"],
        "1620:0:1cfe:face:b00c::3",
        Some("2620:0:1cfe:face:b00c::3"),
        None,
    ),
    // forwarded for with remote IPv6 addr trusted
    (
        &["2620:0:1cfe:face:b00c::3"],
        "1620:0:1cfe:face:b00c::3",
        Some("2620:0:1cfe:face:b00c::3"),
        Some(&["1620:0:1cfe:face:b00c::3"]),
    ),
    // forwarded for with remote IPv6 range trusted
    (
        &["88.88.88.88"],
        "2a01:198:603:0:396e:4789:8e99:890f",
        Some("88.88.88.88"),
        Some(&["2a01:198:603:0::/65"]),
    ),
    // multiple forwarded for with remote IPv4 addr trusted
    (
        &["88.88.88.88", "87.65.43.21", "127.0.0.1"],
        "123.45.67.89",
        Some("127.0.0.1, 87.65.43.21, 88.88.88.88"),
        Some(&["123.45.67.89"]),
    ),
    // multiple forwarded for with remote IPv4 addr and some reverse proxies trusted
    (
        &["87.65.43.21", "127.0.0.1"],
        "123.45.67.89",
        Some("127.0.0.1, 87.65.43.21, 88.88.88.88"),
        Some(&["123.45.67.89", "88.88.88.88"]),
    ),
    // multiple forwarded for with remote IPv4 addr and some reverse proxies trusted but in the middle
    (
        &["88.88.88.88", "127.0.0.1"],
        "123.45.67.89",
        Some("127.0.0.1, 87.65.43.21, 88.88.88.88"),
        Some(&["123.45.67.89", "87.65.43.21"]),
    ),
    // multiple forwarded for with remote IPv4 addr and all reverse proxies trusted
    (
        &["127.0.0.1"],
        "123.45.67.89",
        Some("127.0.0.1, 87.65.43.21, 88.88.88.88"),
        Some(&["123.45.67.89", "87.65.43.21", "88.88.88.88", "127.0.0.1"]),
    ),
    // multiple forwarded for with remote IPv6 addr trusted
    (
        &["2620:0:1cfe:face:b00c::3", "3620:0:1cfe:face:b00c::3"],
        "1620:0:1cfe:face:b00c::3",
        Some("3620:0:1cfe:face:b00c::3,2620:0:1cfe:face:b00c::3"),
        Some(&["1620:0:1cfe:face:b00c::3"]),
    ),
    // multiple forwarded for with remote IPv6 addr and some reverse proxies trusted
    (
        &["3620:0:1cfe:face:b00c::3"],
        "1620:0:1cfe:face:b00c::3",
        Some("3620:0:1cfe:face:b00c::3,2620:0:1cfe:face:b00c::3"),
        Some(&["1620:0:1cfe:face:b00c::3", "2620:0:1cfe:face:b00c::3"]),
    ),
    // multiple forwarded for with remote IPv4 addr and some reverse proxies trusted but in the middle
    (
        &["2620:0:1cfe:face:b00c::3", "4620:0:1cfe:face:b00c::3"],
        "1620:0:1cfe:face:b00c::3",
        Some("4620:0:1cfe:face:b00c::3,3620:0:1cfe:face:b00c::3,2620:0:1cfe:face:b00c::3"),
        Some(&["1620:0:1cfe:face:b00c::3", "3620:0:1cfe:face:b00c::3"]),
    ),
    // client IP with port
    (
        &["88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88:12345, 127.0.0.1"),
        Some(&["127.0.0.1"]),
    ),
    // client IPv6 with brackets and port
    (
        &["2001:db8::1"],
        "127.0.0.1",
        Some("[2001:db8::1]:8080"),
        Some(&["127.0.0.1"]),
    ),
    // IPv4-mapped IPv6 addresses, with or without brackets and port
    (
        &["::ffff:8.8.8.8", "88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88, [::ffff:8.8.8.8]"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["::ffff:8.8.8.8", "88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88, [::ffff:8.8.8.8]:4711"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["0:0:0:0:0:ffff:8.8.8.8", "88.88.88.88"],
        "127.0.0.1",
        Some("88.88.88.88, 0:0:0:0:0:ffff:8.8.8.8"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["::ffff:8.8.8.8"],
        "127.0.0.1",
        Some("[::ffff:8.8.8.8]"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["::ffff:8.8.8.8"],
        "127.0.0.1",
        Some("[::ffff:8.8.8.8]:4711"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["0:0:0:0:0:ffff:8.8.8.8"],
        "127.0.0.1",
        Some("0:0:0:0:0:ffff:8.8.8.8"),
        Some(&["127.0.0.1"]),
    ),
    // invalid forwarded IP is ignored
    (
        &["88.88.88.88"],
        "127.0.0.1",
        Some("unknown,88.88.88.88"),
        Some(&["127.0.0.1"]),
    ),
    (
        &["88.88.88.88"],
        "127.0.0.1",
        Some("}__test|O:21:&quot;JDatabaseDriverMysqli&quot;:3:{s:2,88.88.88.88"),
        Some(&["127.0.0.1"]),
    ),
];

/// `getRequestInstanceForClientIpTests()` and its `Forwarded` twin.
fn client_ip_request(
    remote_addr: &str,
    header: (&str, Option<&str>),
    trusted_proxies: Option<&[&str]>,
    trusted_header: i64,
) -> Result<Request, Error> {
    Request::reset_statics();
    let mut request = Request::default();
    let mut entries = vec![("REMOTE_ADDR", remote_addr)];
    if let Some(value) = header.1 {
        entries.push((header.0, value));
    }
    if let Some(proxies) = trusted_proxies.filter(|p| !p.is_empty()) {
        Request::set_trusted_proxies(proxies.iter().copied(), trusted_header)?;
    }
    initialize_server(&mut request, &entries)?;
    Ok(request)
}

fn ips(expected: &[&str]) -> Vec<Option<PhpString>> {
    expected.iter().map(|ip| some(ip)).collect()
}

#[test]
fn testGetClientIp() -> TestResult {
    let _t = TearDown;
    for (expected, remote_addr, forwarded_for, trusted) in CLIENT_IPS {
        let mut request = client_ip_request(
            remote_addr,
            ("HTTP_X_FORWARDED_FOR", forwarded_for),
            trusted,
            Request::HEADER_X_FORWARDED_FOR,
        )?;
        assert_eq!(
            some(expected[0]),
            request.get_client_ip()?,
            "{remote_addr} {forwarded_for:?}"
        );
    }
    Ok(())
}

#[test]
fn testGetClientIps() -> TestResult {
    let _t = TearDown;
    for (expected, remote_addr, forwarded_for, trusted) in CLIENT_IPS {
        let mut request = client_ip_request(
            remote_addr,
            ("HTTP_X_FORWARDED_FOR", forwarded_for),
            trusted,
            Request::HEADER_X_FORWARDED_FOR,
        )?;
        assert_eq!(
            ips(expected),
            request.get_client_ips()?,
            "{remote_addr} {forwarded_for:?}"
        );
    }
    Ok(())
}

#[test]
fn testGetClientIpsForwarded() -> TestResult {
    let _t = TearDown;
    let rows: [(&[&str], &str, &str, Option<&[&str]>); 10] = [
        (&["127.0.0.1"], "127.0.0.1", r#"for="_gazonk""#, None),
        (
            &["127.0.0.1"],
            "127.0.0.1",
            r#"for="_gazonk""#,
            Some(&["127.0.0.1"]),
        ),
        (
            &["88.88.88.88"],
            "127.0.0.1",
            r#"for="88.88.88.88:80""#,
            Some(&["127.0.0.1"]),
        ),
        (
            &["192.0.2.60"],
            "::1",
            "for=192.0.2.60;proto=http;by=203.0.113.43",
            Some(&["::1"]),
        ),
        (
            &["2620:0:1cfe:face:b00c::3", "192.0.2.43"],
            "::1",
            r#"for=192.0.2.43, for="[2620:0:1cfe:face:b00c::3]""#,
            Some(&["::1"]),
        ),
        (
            &["2001:db8:cafe::17"],
            "::1",
            r#"for="[2001:db8:cafe::17]:4711"#,
            Some(&["::1"]),
        ),
        (
            &["::ffff:8.8.8.8", "192.0.2.43"],
            "::1",
            r#"for=192.0.2.43, for="[::ffff:8.8.8.8]""#,
            Some(&["::1"]),
        ),
        (
            &["::ffff:8.8.8.8", "192.0.2.43"],
            "::1",
            r#"for=192.0.2.43, for="[::ffff:8.8.8.8]:4711""#,
            Some(&["::1"]),
        ),
        (
            &["::ffff:8.8.8.8"],
            "::1",
            r#"for="[::ffff:8.8.8.8]""#,
            Some(&["::1"]),
        ),
        (
            &["::ffff:8.8.8.8"],
            "::1",
            r#"for="[::ffff:8.8.8.8]:4711""#,
            Some(&["::1"]),
        ),
    ];
    for (expected, remote_addr, forwarded, trusted) in rows {
        let mut request = client_ip_request(
            remote_addr,
            ("HTTP_FORWARDED", Some(forwarded)),
            trusted,
            Request::HEADER_FORWARDED,
        )?;
        assert_eq!(ips(expected), request.get_client_ips()?, "{forwarded}");
    }
    Ok(())
}

const CONFLICTING_HEADERS: [(&str, &str); 5] = [
    ("for=87.65.43.21", "192.0.2.60"),
    ("for=87.65.43.21, for=192.0.2.60", "192.0.2.60"),
    ("for=192.0.2.60", "192.0.2.60,87.65.43.21"),
    (r#"for="::face", for=192.0.2.60"#, "192.0.2.60,192.0.2.43"),
    ("for=87.65.43.21, for=192.0.2.60", "192.0.2.60,87.65.43.21"),
];

fn forwarded_request(forwarded: &str, forwarded_for: &str, set: i64) -> Result<Request, Error> {
    Request::reset_statics();
    let mut request = Request::default();
    Request::set_trusted_proxies(["88.88.88.88"], set)?;
    initialize_server(
        &mut request,
        &[
            ("REMOTE_ADDR", "88.88.88.88"),
            ("HTTP_FORWARDED", forwarded),
            ("HTTP_X_FORWARDED_FOR", forwarded_for),
        ],
    )?;
    Ok(request)
}

#[test]
fn testGetClientIpsWithConflictingHeaders() -> TestResult {
    let _t = TearDown;
    for (forwarded, forwarded_for) in CONFLICTING_HEADERS {
        let mut request = forwarded_request(
            forwarded,
            forwarded_for,
            Request::HEADER_X_FORWARDED_FOR | Request::HEADER_FORWARDED,
        )?;
        expect_err(request.get_client_ips(), CONFLICTING, "");
    }
    Ok(())
}

#[test]
fn testGetClientIpsOnlyXHttpForwardedForTrusted() -> TestResult {
    let _t = TearDown;
    for (forwarded, forwarded_for) in CONFLICTING_HEADERS {
        let mut request =
            forwarded_request(forwarded, forwarded_for, Request::HEADER_X_FORWARDED_FOR)?;
        let expected: Vec<Option<PhpString>> = forwarded_for.split(',').rev().map(some).collect();
        assert_eq!(expected, request.get_client_ips()?);
    }
    Ok(())
}

#[test]
fn testGetClientIpsWithAgreeingHeaders() -> TestResult {
    let _t = TearDown;
    let rows: [(&str, &str, &[&str]); 6] = [
        (r#"for="192.0.2.60""#, "192.0.2.60", &["192.0.2.60"]),
        (
            "for=192.0.2.60, for=87.65.43.21",
            "192.0.2.60,87.65.43.21",
            &["87.65.43.21", "192.0.2.60"],
        ),
        (
            r#"for="[::face]", for=192.0.2.60"#,
            "::face,192.0.2.60",
            &["192.0.2.60", "::face"],
        ),
        (r#"for="192.0.2.60:80""#, "192.0.2.60", &["192.0.2.60"]),
        (
            "for=192.0.2.60;proto=http;by=203.0.113.43",
            "192.0.2.60",
            &["192.0.2.60"],
        ),
        (
            r#"for="[2001:db8:cafe::17]:4711""#,
            "2001:db8:cafe::17",
            &["2001:db8:cafe::17"],
        ),
    ];
    for (forwarded, forwarded_for, expected) in rows {
        let mut request = forwarded_request(forwarded, forwarded_for, -1)?;
        assert_eq!(ips(expected), request.get_client_ips()?, "{forwarded}");
    }
    Ok(())
}

fn content_string(request: &mut Request) -> Result<String, Error> {
    match request.get_content(false)? {
        ContentValue::String(s) => Ok(text(&s)),
        ContentValue::Resource(r) => Ok(String::from_utf8_lossy(&r.contents()).into_owned()),
    }
}

fn content_resource(request: &mut Request) -> Result<Stream, Error> {
    match request.get_content(true)? {
        ContentValue::Resource(r) => Ok(r),
        ContentValue::String(s) => panic!("expected a resource, got {s:?}"),
    }
}

#[test]
fn testGetContentWorksTwiceInDefaultMode() -> TestResult {
    let _t = TearDown;
    let mut req = Request::default();
    assert_eq!("", content_string(&mut req)?);
    assert_eq!("", content_string(&mut req)?);
    Ok(())
}

#[test]
fn testGetContentReturnsResource() -> TestResult {
    let _t = TearDown;
    let mut req = Request::default();
    let retval = content_resource(&mut req)?;
    assert_eq!(b"", retval.read(1).as_slice());
    assert!(retval.eof());
    Ok(())
}

#[test]
fn testGetContentReturnsResourceWhenContentSetInConstructor() -> TestResult {
    let _t = TearDown;
    let mut req = Request::new(Parameters {
        content: Content::from("MyContent"),
        ..Parameters::default()
    })?;
    let resource = content_resource(&mut req)?;
    assert_eq!(b"MyContent", resource.contents().as_slice());
    Ok(())
}

#[test]
fn testContentAsResource() -> TestResult {
    let _t = TearDown;
    let resource = Stream::default();
    resource.write(b"My other content");
    resource.rewind();
    let mut req = Request::new(Parameters {
        content: Content::Resource(resource),
        ..Parameters::default()
    })?;
    assert_eq!(
        b"My other content",
        content_resource(&mut req)?.contents().as_slice()
    );
    assert_eq!("My other content", content_string(&mut req)?);
    Ok(())
}

#[test]
fn testGetContentCanBeCalledTwiceWithResources() -> TestResult {
    let _t = TearDown;
    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut req = Request::default();
        let read = |req: &mut Request, resource: bool| -> Result<Vec<u8>, Error> {
            Ok(match req.get_content(resource)? {
                ContentValue::String(s) => s.as_bytes().to_vec(),
                ContentValue::Resource(r) => r.contents(),
            })
        };
        let a = read(&mut req, first)?;
        let b = read(&mut req, second)?;
        assert_eq!(a, b, "{first} {second}");
    }
    Ok(())
}

#[test]
fn testToArrayEmpty() {
    let _t = TearDown;
    let mut req = Request::default();
    expect_err(req.to_array(), JSON, "Request body is empty.");
}

#[test]
fn testToArrayNonJson() -> TestResult {
    let _t = TearDown;
    let mut req = Request::new(Parameters {
        content: Content::from("foobar"),
        ..Parameters::default()
    })?;
    let error = req.to_array().err().ok_or("expected a JsonException")?;
    assert!(error.is_a(JSON));
    // expectExceptionMessageMatches('|Could not decode request body.+|')
    let message = text(error.message());
    assert!(
        message.starts_with("Could not decode request body")
            && message.len() > "Could not decode request body".len()
    );
    Ok(())
}

#[test]
fn testToArray() -> TestResult {
    let _t = TearDown;
    let mut req = Request::new(Parameters {
        content: Content::from("[]"),
        ..Parameters::default()
    })?;
    assert_eq!(PhpArray::new(), req.to_array()?);
    let mut req = Request::new(Parameters {
        content: Content::from(r#"{"foo":"bar"}"#),
        ..Parameters::default()
    })?;
    assert_eq!(php_array!["foo" => "bar"], req.to_array()?);
    Ok(())
}

#[test]
fn testGetPayload() -> TestResult {
    let _t = TearDown;
    let mut req = Request::new(Parameters {
        content: Content::from(r#"{"foo":"bar"}"#),
        ..Parameters::default()
    })?;
    assert_eq!(&php_array!["foo" => "bar"], req.get_payload()?.all());
    req.get_payload()?.set("new", Value::string("key"))?;
    assert_eq!(&php_array!["foo" => "bar"], req.get_payload()?.all());

    let mut req = Request::new(Parameters {
        request: php_array!["foo" => "bar"],
        content: Content::from(r#"{"baz":"qux"}"#),
        ..Parameters::default()
    })?;
    assert_eq!(&php_array!["foo" => "bar"], req.get_payload()?.all());

    let mut req = Request::new(Parameters {
        content: Content::from(""),
        ..Parameters::default()
    })?;
    assert_eq!(&PhpArray::new(), req.get_payload()?.all());
    Ok(())
}

#[test]
fn testCreateFromGlobals() -> TestResult {
    let _t = TearDown;
    with_globals(|g| {
        g.get.insert("foo1", Value::string("bar1"));
        g.post.insert("foo2", Value::string("bar2"));
        g.cookie.insert("foo3", Value::string("bar3"));
        g.files.insert("foo4", Value::Array(php_array!["bar4"]));
        g.server.insert("foo5", Value::string("bar5"));
    });
    let request = Request::create_from_globals()?;
    assert_eq!(
        Value::string("bar1"),
        request.query().get("foo1", Value::Null)?,
        "::createFromGlobals() uses values from $_GET"
    );
    assert_eq!(
        Value::string("bar2"),
        request.request().get("foo2", Value::Null)?,
        "::createFromGlobals() uses values from $_POST"
    );
    assert_eq!(
        Value::string("bar3"),
        request.cookies().get("foo3", Value::Null)?,
        "::createFromGlobals() uses values from $_COOKIE"
    );
    assert_eq!(
        Value::Array(php_array!["bar4"]),
        request.files().get("foo4", Value::Null),
        "::createFromGlobals() uses values from $_FILES"
    );
    assert_eq!(
        Value::string("bar5"),
        request.server().get("foo5", Value::Null),
        "::createFromGlobals() uses values from $_SERVER"
    );
    Ok(())
}

#[test]
fn testGetRealMethod() -> TestResult {
    let _t = TearDown;
    Request::enable_http_method_parameter_override();
    let request = Request::new(Parameters {
        request: php_array!["_method" => "PUT"],
        server: php_array!["REQUEST_METHOD" => "PoSt"],
        ..Parameters::default()
    })?;
    assert_eq!(
        s("POST"),
        request.get_real_method()?,
        "->getRealMethod() returns the uppercased request method, even if it has been overridden"
    );
    Ok(())
}

#[test]
fn testCreateDoesNotWarnAboutQueryKeysNestedBeyondMaxInputNestingLevel() -> TestResult {
    let _t = TearDown;
    // max_input_nesting_level is 64; PHP's warnings have no counterpart here
    let key = format!("a{}", "[b]".repeat(65));
    let request = Request::create(format!("/?{key}=1&x=2"))?;
    assert_eq!(&php_array!["x" => "2"], request.query().all());
    Ok(())
}

#[test]
fn testOverrideGlobals() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    request.initialize(Parameters {
        query: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    request.override_globals()?;
    assert_eq!(php_array!["foo" => "bar"], with_globals(|g| g.get.clone()));

    request.initialize(Parameters {
        request: php_array!["foo" => "bar"],
        ..Parameters::default()
    })?;
    request.override_globals()?;
    assert_eq!(php_array!["foo" => "bar"], with_globals(|g| g.post.clone()));
    assert!(!with_globals(|g| g
        .server
        .contains_key(&"HTTP_X_FORWARDED_PROTO".into())));

    set_header(&mut request, "X_FORWARDED_PROTO", "https")?;
    Request::set_trusted_proxies(["1.1.1.1"], Request::HEADER_X_FORWARDED_PROTO)?;
    assert!(!request.is_secure()?);
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    assert!(request.is_secure()?);

    request.override_globals()?;
    assert!(with_globals(|g| g
        .server
        .contains_key(&"HTTP_X_FORWARDED_PROTO".into())));

    set_header(&mut request, "CONTENT_TYPE", "multipart/form-data")?;
    set_header(&mut request, "CONTENT_LENGTH", "12345")?;
    request.override_globals()?;
    assert!(with_globals(|g| g
        .server
        .contains_key(&"CONTENT_TYPE".into())));
    assert!(with_globals(|g| g
        .server
        .contains_key(&"CONTENT_LENGTH".into())));

    request.initialize(Parameters {
        query: php_array!["foo" => "bar", "baz" => "foo"],
        ..Parameters::default()
    })?;
    request.query_mut().remove("baz");
    request.override_globals()?;
    assert_eq!(php_array!["foo" => "bar"], with_globals(|g| g.get.clone()));
    assert_eq!(
        Some(Value::string("foo=bar")),
        with_globals(|g| g.server.get(&"QUERY_STRING".into()).cloned())
    );
    assert_eq!(
        Value::string("foo=bar"),
        request.server().get("QUERY_STRING", Value::Null)
    );
    Ok(())
}

#[test]
fn testGetScriptName() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!("", text(&request.get_script_name()?));

    initialize_server(&mut request, &[("SCRIPT_NAME", "/index.php")])?;
    assert_eq!("/index.php", text(&request.get_script_name()?));

    initialize_server(&mut request, &[("ORIG_SCRIPT_NAME", "/frontend.php")])?;
    assert_eq!("/frontend.php", text(&request.get_script_name()?));

    initialize_server(
        &mut request,
        &[
            ("SCRIPT_NAME", "/index.php"),
            ("ORIG_SCRIPT_NAME", "/frontend.php"),
        ],
    )?;
    assert_eq!("/index.php", text(&request.get_script_name()?));
    Ok(())
}

#[test]
fn testGetBasePath() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!("", text(&request.get_base_path()?));

    for entries in [
        &[("SCRIPT_FILENAME", "/some/where/index.php")][..],
        &[
            ("SCRIPT_FILENAME", "/some/where/index.php"),
            ("SCRIPT_NAME", "/index.php"),
        ],
        &[
            ("SCRIPT_FILENAME", "/some/where/index.php"),
            ("PHP_SELF", "/index.php"),
        ],
        &[
            ("SCRIPT_FILENAME", "/some/where/index.php"),
            ("ORIG_SCRIPT_NAME", "/index.php"),
        ],
    ] {
        initialize_server(&mut request, entries)?;
        assert_eq!("", text(&request.get_base_path()?), "{entries:?}");
    }
    Ok(())
}

#[test]
fn testGetPathInfo() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!("/", text(&request.get_path_info()?));

    for (uri, expected) in [
        ("/path/info", "/path/info"),
        ("/path%20test/info", "/path%20test/info"),
        ("?a=b", "/"),
    ] {
        initialize_server(&mut request, &[("REQUEST_URI", uri)])?;
        assert_eq!(expected, text(&request.get_path_info()?));
    }
    Ok(())
}

#[test]
fn testGetPreferredLanguage() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(None, request.get_preferred_language(None)?);
    Ok(())
}

#[test]
fn testPreferredLanguageWithLocales() -> TestResult {
    let _t = TearDown;
    let rows: [(Option<&str>, Option<&str>, &[&str]); 23] = [
        (Some("es_PA"), Some("es-pa, en-us; q=0.8, en; q=0.6"), &[]),
        (None, None, &[]),
        (Some("fr"), None, &["fr", "en"]),
        (Some("en"), None, &["en", "fr"]),
        (Some("fr_CH"), None, &["fr-ch", "fr-fr"]),
        (
            Some("en_US"),
            Some("zh, en-us; q=0.8, en; q=0.6"),
            &["en", "en-us"],
        ),
        (
            Some("fr_FR"),
            Some("ja-JP,fr_CA;q=0.7,fr;q=0.5,en_US;q=0.3"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("en"),
            Some("zh, en-us; q=0.8, en; q=0.6"),
            &["fr", "en"],
        ),
        (
            Some("en"),
            Some("zh, en-us; q=0.8, fr-fr; q=0.6, fr; q=0.5"),
            &["fr", "en"],
        ),
        (Some("en"), Some("zh, en-us; q=0.8"), &["fr", "en"]),
        (
            Some("fr_FR"),
            Some("ja-JP,fr_CA;q=0.7,fr;q=0.5"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("fr_FR"),
            Some("ja-JP,fr_CA;q=0.7"),
            &["en_US", "fr_FR"],
        ),
        (Some("fr_FR"), Some("ja-JP,fr;q=0.5"), &["en_US", "fr_FR"]),
        (
            Some("fr_FR"),
            Some("ja-JP,fr;q=0.5,en_US;q=0.3"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("fr_FR"),
            Some("ja-JP,fr_CA;q=0.7,ru-ru;q=0.3"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("fr_FR"),
            Some("ja-JP,fr_CA;q=0.7,en;q=0.5"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("fr_FR"),
            Some("en-us;q=0.5,fr-fr"),
            &["en_US", "fr_FR"],
        ),
        (
            Some("hi_IN"),
            Some("fr-fr,hi_Latn_IN;q=0.5"),
            &["hi_IN", "en_US"],
        ),
        (
            Some("hi_Latn_IN"),
            Some("fr-fr,hi_IN;q=0.5"),
            &["hi_Latn_IN", "en_US"],
        ),
        (
            Some("en_US"),
            Some("en-latn-us-fonapi-u-nu-numerical-x-private,fr;q=0.5"),
            &["fr_FR", "en_US"],
        ),
        (
            Some("zh_Hans"),
            Some("zh-hans-tw, zh-hant-tw"),
            &["zh_Hans", "zh_tw"],
        ),
        (Some("en"), Some("fi"), &["en", "fil"]),
        (Some("fil"), Some("fi, fil-ph;q=0.5"), &["en", "fil"]),
    ];
    for (expected, accept_language, locales) in rows {
        let mut request = Request::default();
        if let Some(accept_language) = accept_language {
            set_header(&mut request, "Accept-language", accept_language)?;
        }
        assert_eq!(
            expected.map(s),
            request.get_preferred_language(Some(locales))?,
            "{accept_language:?} {locales:?}"
        );
    }
    Ok(())
}

#[test]
fn testIsXmlHttpRequest() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert!(!request.is_xml_http_request());
    set_header(&mut request, "X-Requested-With", "XMLHttpRequest")?;
    assert!(request.is_xml_http_request());
    request.headers_mut().remove("X-Requested-With");
    assert!(!request.is_xml_http_request());
    Ok(())
}

fn strings(values: &[&str]) -> Vec<PhpString> {
    values.iter().map(|v| s(v)).collect()
}

#[test]
fn testGetCharsets() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(request.get_charsets(), [] as [php_value::PhpString; 0]);
    set_header(
        &mut request,
        "Accept-Charset",
        "ISO-8859-1, US-ASCII, UTF-8; q=0.8, ISO-10646-UCS-2; q=0.6",
    )?;
    assert_eq!(request.get_charsets(), [] as [php_value::PhpString; 0]); // testing caching

    let mut request = Request::default();
    set_header(
        &mut request,
        "Accept-Charset",
        "ISO-8859-1, US-ASCII, UTF-8; q=0.8, ISO-10646-UCS-2; q=0.6",
    )?;
    assert_eq!(
        strings(&["ISO-8859-1", "US-ASCII", "UTF-8", "ISO-10646-UCS-2"]),
        request.get_charsets()
    );

    let mut request = Request::default();
    set_header(
        &mut request,
        "Accept-Charset",
        "ISO-8859-1,utf-8;q=0.7,*;q=0.7",
    )?;
    assert_eq!(
        strings(&["ISO-8859-1", "utf-8", "*"]),
        request.get_charsets()
    );
    Ok(())
}

#[test]
fn testGetEncodings() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(request.get_encodings(), [] as [php_value::PhpString; 0]);
    set_header(&mut request, "Accept-Encoding", "gzip,deflate,sdch")?;
    assert_eq!(request.get_encodings(), [] as [php_value::PhpString; 0]); // testing caching

    let mut request = Request::default();
    set_header(&mut request, "Accept-Encoding", "gzip,deflate,sdch")?;
    assert_eq!(
        strings(&["gzip", "deflate", "sdch"]),
        request.get_encodings()
    );

    let mut request = Request::default();
    set_header(
        &mut request,
        "Accept-Encoding",
        "gzip;q=0.4,deflate;q=0.9,compress;q=0.7",
    )?;
    assert_eq!(
        strings(&["deflate", "compress", "gzip"]),
        request.get_encodings()
    );
    Ok(())
}

#[test]
fn testGetAcceptableContentTypes() -> TestResult {
    let _t = TearDown;
    let accept = "application/vnd.wap.wmlscriptc, text/vnd.wap.wml, application/vnd.wap.xhtml+xml, application/xhtml+xml, text/html, multipart/mixed, */*";
    let mut request = Request::default();
    assert_eq!(
        request.get_acceptable_content_types()?,
        [] as [php_value::PhpString; 0]
    );
    set_header(&mut request, "Accept", accept)?;
    assert_eq!(
        request.get_acceptable_content_types()?,
        [] as [php_value::PhpString; 0]
    ); // testing caching

    let mut request = Request::default();
    set_header(&mut request, "Accept", accept)?;
    assert_eq!(
        strings(&[
            "application/vnd.wap.wmlscriptc",
            "text/vnd.wap.wml",
            "application/vnd.wap.xhtml+xml",
            "application/xhtml+xml",
            "text/html",
            "multipart/mixed",
            "*/*"
        ]),
        request.get_acceptable_content_types()?
    );
    Ok(())
}

/// `getLanguages()` as `assertEquals()` compares it with a list: keys and
/// values.
fn languages(values: &[&str]) -> PhpArray {
    PhpArray::list(values.iter().map(|v| Value::string(*v)))
}

#[test]
fn testGetLanguages() -> TestResult {
    let _t = TearDown;
    let rows: [(&[&str], Option<&str>); 8] = [
        (&[], None),
        (&["zh", "en_US", "en"], Some("zh, en-us; q=0.8, en; q=0.6")),
        (&["zh", "en", "en_US"], Some("zh, en-us; q=0.6, en; q=0.8")),
        (&["zh", "en", "en_US"], Some("zh, en, en-us")),
        (&["en", "zh", "en_US"], Some("zh; q=0.6, en, en-us; q=0.6")),
        (&["zh", "cherokee"], Some("zh, i-cherokee; q=0.6")),
        (
            &["pt_BR", "hy_Latn_IT", "zh_Hans_TW"],
            Some(
                "pt-BR-u-ca-gregory-nu-latn, hy-Latn-IT-arevela, zh-Hans-TW-fonapi-u-islamcal-x-AZE-derbend; q=0.6",
            ),
        ),
        (
            &["en_US", "en_CA", "en_GB", "en"],
            Some("en-us, en-ca, en-gb, en"),
        ),
    ];
    for (expected, accept_language) in rows {
        let mut request = Request::default();
        if let Some(accept_language) = accept_language {
            set_header(&mut request, "Accept-language", accept_language)?;
        }
        assert_eq!(
            languages(expected),
            request.get_languages()?,
            "{accept_language:?}"
        );
    }
    Ok(())
}

#[test]
fn testGetAcceptHeadersReturnString() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    for header in [
        "Accept",
        "Accept-Charset",
        "Accept-Encoding",
        "Accept-Language",
    ] {
        set_header(&mut request, header, "123")?;
    }
    assert_eq!(strings(&["123"]), request.get_acceptable_content_types()?);
    assert_eq!(strings(&["123"]), request.get_charsets());
    assert_eq!(strings(&["123"]), request.get_encodings());
    assert_eq!(languages(&["123"]), request.get_languages()?);
    Ok(())
}

#[test]
fn testGetRequestFormat() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    assert_eq!(some("html"), request.get_request_format(Some("html"))?);

    // Ensure that setting different default values over time is possible,
    // aka. setRequestFormat determines the state.
    assert_eq!(some("json"), request.get_request_format(Some("json"))?);
    assert_eq!(some("html"), request.get_request_format(Some("html"))?);

    let mut request = Request::default();
    assert_eq!(None, request.get_request_format(None)?);

    let mut request = Request::default();
    request.set_request_format(some("foo"));
    assert_eq!(some("foo"), request.get_request_format(None)?);

    let mut request = Request::new(Parameters {
        query: php_array!["_format" => "foo"],
        ..Parameters::default()
    })?;
    assert_eq!(some("html"), request.get_request_format(Some("html"))?);
    Ok(())
}

/// `new Session(new MockArraySessionStorage())`.
fn mock_session() -> Result<Arc<dyn SessionInterface>, Error> {
    Ok(Arc::new(Session::new(
        Arc::new(MockArraySessionStorage::default()),
        None,
        None,
        None,
    )?))
}

#[test]
fn testHasSession() -> TestResult {
    let mut request = Request::default();

    assert!(!request.has_session(false));
    assert!(!request.has_session(true));

    // `static function () {}`: a factory that returns null.
    request.set_session_factory(Arc::new(|| Ok(None)));
    assert!(request.has_session(false));
    assert!(!request.has_session(true));

    request.set_session(mock_session()?);
    assert!(request.has_session(false));
    assert!(request.has_session(true));
    Ok(())
}

#[test]
fn testGetSession() -> TestResult {
    let mut request = Request::default();

    let session = mock_session()?;
    request.set_session(Arc::clone(&session));
    assert!(request.has_session(false));

    // assertInstanceOf(Session::class): the Session given back.
    assert!(Arc::ptr_eq(&session, &request.get_session()?));
    Ok(())
}

#[test]
fn testHasPreviousSession() -> TestResult {
    let mut request = Request::default();

    assert!(!request.has_previous_session()?);
    request
        .cookies_mut()
        .set("MOCKSESSID", Value::string("foo"))?;
    assert!(!request.has_previous_session()?);
    request.set_session(mock_session()?);
    assert!(request.has_previous_session()?);
    Ok(())
}

#[test]
fn testToString() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    set_header(
        &mut request,
        "Accept-language",
        "zh, en-us; q=0.8, en; q=0.6",
    )?;
    request.cookies_mut().set("Foo", Value::string("Bar"))?;

    let as_string = text(&request.to_php_string()?);
    assert!(as_string.contains("Accept-Language: zh, en-us; q=0.8, en; q=0.6"));
    assert!(as_string.contains("Cookie: Foo=Bar"));

    request
        .cookies_mut()
        .set("Another", Value::string("Cookie"))?;
    let as_string = text(&request.to_php_string()?);
    assert!(as_string.contains("Cookie: Foo=Bar; Another=Cookie"));

    request
        .cookies_mut()
        .set("foo.bar", Value::Array(php_array![1, 2]))?;
    let as_string = text(&request.to_php_string()?);
    assert!(as_string.contains("foo.bar%5B0%5D=1; foo.bar%5B1%5D=2"));
    Ok(())
}

#[test]
fn testIsMethod() -> TestResult {
    let _t = TearDown;
    let mut request = Request::default();
    request.set_method("POST");
    assert!(request.is_method("POST")?);
    assert!(request.is_method("post")?);
    assert!(!request.is_method("GET")?);
    assert!(!request.is_method("get")?);

    request.set_method("GET");
    assert!(request.is_method("GET")?);
    assert!(request.is_method("get")?);
    assert!(!request.is_method("POST")?);
    assert!(!request.is_method("post")?);
    Ok(())
}

fn script(filename: &str, name: &str, php_self: &str) -> Vec<(String, String)> {
    vec![
        ("SCRIPT_FILENAME".to_owned(), filename.to_owned()),
        ("SCRIPT_NAME".to_owned(), name.to_owned()),
        ("PHP_SELF".to_owned(), php_self.to_owned()),
    ]
}

/// `getBaseUrlData()`.
fn base_url_data() -> Vec<(String, Vec<(String, String)>, String, String)> {
    let a = "a".repeat(2000);
    let row = |uri: &str, server: Vec<(String, String)>, base: &str, path: &str| {
        (uri.to_owned(), server, base.to_owned(), path.to_owned())
    };
    let foo_bar = script(
        "/home/John Doe/public_html/foo bar/app.php",
        "/foo bar/app.php",
        "/foo bar/app.php",
    );
    let foo_app = script(
        "/home/John Doe/public_html/foo/app.php",
        "/foo/app.php",
        "/foo/app.php",
    );
    let api = script("/var/www/api/index.php", "/api/index.php", "/api/index.php");
    vec![
        row(
            "/fruit/strawberry/1234index.php/blah",
            script(
                "E:/Sites/cc-new/public_html/fruit/index.php",
                "/fruit/index.php",
                "/fruit/index.php",
            ),
            "/fruit",
            "/strawberry/1234index.php/blah",
        ),
        row(
            "/fruit/strawberry/1234index.php/blah",
            script(
                "E:/Sites/cc-new/public_html/index.php",
                "/index.php",
                "/index.php",
            ),
            "",
            "/fruit/strawberry/1234index.php/blah",
        ),
        row("/foo%20bar/", foo_bar.clone(), "/foo%20bar", "/"),
        row("/foo%20bar/home", foo_bar.clone(), "/foo%20bar", "/home"),
        row(
            "/foo%20bar/app.php/home",
            foo_bar.clone(),
            "/foo%20bar/app.php",
            "/home",
        ),
        row(
            "/foo%20bar/app.php/home%3Dbaz",
            foo_bar,
            "/foo%20bar/app.php",
            "/home%3Dbaz",
        ),
        row("/foo/bar+baz", foo_app.clone(), "/foo", "/bar+baz"),
        row("/sub/foo/bar", foo_app.clone(), "", "/sub/foo/bar"),
        row("/sub/foo/app.php/bar", foo_app, "/sub/foo/app.php", "/bar"),
        row(
            "/sub/foo/bar/baz",
            script(
                "/home/John Doe/public_html/foo/app2.phpx",
                "/foo/app2.phpx",
                "/foo/app2.phpx",
            ),
            "",
            "/sub/foo/bar/baz",
        ),
        row("/foo/api/bar", api.clone(), "", "/foo/api/bar"),
        row("/api/index.phpfoo", api.clone(), "/api", "/index.phpfoo"),
        row(
            "/index.phpfoo",
            script("/var/www/index.php", "/index.php", "/index.php"),
            "",
            "/index.phpfoo",
        ),
        row("/api/index.php?foo=bar", api.clone(), "/api/index.php", "/"),
        row(
            "/sub/api/index.phpfoo",
            api.clone(),
            "",
            "/sub/api/index.phpfoo",
        ),
        row("/api%2Ffoo", api.clone(), "/api%2F", "/foo"),
        row("/x/index.php/foo", api.clone(), "", "/x/index.php/foo"),
        row(
            "/sub/index.php?next=/api/index.php",
            api.clone(),
            "",
            "/sub/index.php",
        ),
        row(
            "/sub/api/index.php?foo=bar",
            api.clone(),
            "/sub/api/index.php",
            "/",
        ),
        // Percent-encoded script name after a prefix
        row(
            "/sub/foo%20bar/app.php/home",
            script(
                "/var/www/foo bar/app.php",
                "/foo bar/app.php",
                "/foo bar/app.php",
            ),
            "/sub/foo%20bar/app.php",
            "/home",
        ),
        // Script name after a long prefix
        row(
            &format!("/{a}/api/index.php/foo"),
            api,
            &format!("/{a}/api/index.php"),
            "/foo",
        ),
        // Long script name
        row(
            &format!("/{a}/index.php/foo"),
            script(
                &format!("/var/www/{a}/index.php"),
                &format!("/{a}/index.php"),
                &format!("/{a}/index.php/foo"),
            ),
            &format!("/{a}/index.php"),
            "/foo",
        ),
        // Script name taken from the request path
        row(
            "/x/index.php/foo",
            script(
                "/var/www/app/index.php",
                "/x/index.php/foo",
                "/x/index.php/foo",
            ),
            "",
            "/x/index.php/foo",
        ),
    ]
}

#[test]
fn testGetBaseUrl() -> TestResult {
    let _t = TearDown;
    for (uri, entries, expected_base_url, expected_path_info) in base_url_data() {
        let entries: Vec<(&str, &str)> = entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let mut request = create_with_server(&uri, "GET", &entries)?;
        let short: String = uri.chars().take(60).collect();
        assert_eq!(
            expected_base_url,
            text(&request.get_base_url()?),
            "baseUrl {short}"
        );
        assert_eq!(
            expected_path_info,
            text(&request.get_path_info()?),
            "pathInfo {short}"
        );
    }
    Ok(())
}

#[test]
fn testGetBaseUrlDoesNotStartWithTwoSlashes() -> TestResult {
    let _t = TearDown;
    for request_uri in [
        "//evil.example/public/index.php/admin",
        r"/\evil.example/public/index.php/admin",
    ] {
        let mut request = with_server(&[
            ("REQUEST_URI", request_uri),
            ("SCRIPT_FILENAME", "/var/www/public/index.php"),
            ("SCRIPT_NAME", "/public/index.php"),
            ("PHP_SELF", "/public/index.php"),
        ])?;
        assert_eq!("", text(&request.get_base_url()?));
        assert_eq!("", text(&request.get_base_path()?));
        assert_eq!(request_uri, text(&request.get_path_info()?));
    }
    Ok(())
}

#[test]
fn testGetBaseUrlDoesNotStartWithTwoSlashesWhenTheScriptNameIsTheRequestPath() -> TestResult {
    let _t = TearDown;
    for request_uri in ["//evil.example/index.php", r"/\evil.example/index.php"] {
        let mut request = with_server(&[
            ("REQUEST_URI", request_uri),
            ("SCRIPT_FILENAME", "/var/www/public/index.php"),
            ("SCRIPT_NAME", request_uri),
            ("PHP_SELF", request_uri),
        ])?;
        assert_eq!("", text(&request.get_base_url()?));
        assert_eq!("", text(&request.get_base_path()?));
    }
    Ok(())
}

#[test]
fn testBaseUriDetectionOnIisWithRewrite() -> TestResult {
    let _t = TearDown;
    let rows: [(&[(&str, &str)], &str, &str); 4] = [
        // No rewrite
        (
            &[
                ("PATH_INFO", "/foo/bar"),
                ("PHP_SELF", "/routingtest/index.php/foo/bar"),
                ("REQUEST_URI", "/routingtest/index.php/foo/bar"),
                (
                    "SCRIPT_FILENAME",
                    "C:/Users/derrabus/Projects/routing-test/public/index.php",
                ),
                ("SCRIPT_NAME", "/routingtest/index.php"),
            ],
            "/routingtest/index.php",
            "/foo/bar",
        ),
        // Rewrite with correct case
        (
            &[
                ("IIS_WasUrlRewritten", "1"),
                ("PATH_INFO", "/foo/bar"),
                ("PHP_SELF", "/routingtest/index.php/foo/bar"),
                ("REQUEST_URI", "/routingtest/foo/bar"),
                (
                    "SCRIPT_FILENAME",
                    "C:/Users/derrabus/Projects/routing-test/public/index.php",
                ),
                ("SCRIPT_NAME", "/routingtest/index.php"),
                ("UNENCODED_URL", "/routingtest/foo/bar"),
            ],
            "/routingtest",
            "/foo/bar",
        ),
        // ISS with UrlRewriteModule might report SCRIPT_NAME/PHP_SELF with wrong case
        (
            &[
                ("IIS_WasUrlRewritten", "1"),
                ("PATH_INFO", "/foo/bar"),
                ("PHP_SELF", "/routingtest/index.php/foo/bar"),
                ("REQUEST_URI", "/RoutingTest/foo/bar"),
                (
                    "SCRIPT_FILENAME",
                    "C:/Users/derrabus/Projects/routing-test/public/index.php",
                ),
                ("SCRIPT_NAME", "/routingtest/index.php"),
                ("UNENCODED_URL", "/RoutingTest/foo/bar"),
            ],
            "/RoutingTest",
            "/foo/bar",
        ),
        // Rewrite with case mismatch after a prefix
        (
            &[
                ("IIS_WasUrlRewritten", "1"),
                ("PATH_INFO", "/foo/bar"),
                ("PHP_SELF", "/routingtest/index.php/foo/bar"),
                ("REQUEST_URI", "/Sub/RoutingTest/Index.php/foo/bar"),
                (
                    "SCRIPT_FILENAME",
                    "C:/Users/derrabus/Projects/routing-test/public/index.php",
                ),
                ("SCRIPT_NAME", "/routingtest/index.php"),
                ("UNENCODED_URL", "/Sub/RoutingTest/Index.php/foo/bar"),
            ],
            "/Sub/RoutingTest/Index.php",
            "/foo/bar",
        ),
    ];
    for (entries, expected_base_url, expected_path_info) in rows {
        let mut request = with_server(entries)?;
        assert_eq!(expected_base_url, text(&request.get_base_url()?));
        assert_eq!(expected_path_info, text(&request.get_path_info()?));
    }
    Ok(())
}

#[test]
fn testUrlencodedStringPrefix() -> TestResult {
    let _t = TearDown;
    for (string, prefix, expect) in [
        ("foo", "foo", "foo"),
        ("fo%6f", "foo", "fo%6f"),
        ("foo/bar", "foo", "foo"),
        ("fo%6f/bar", "foo", "fo%6f"),
        ("f%6f%6f/bar", "foo", "f%6f%6f"),
        ("%66%6F%6F/bar", "foo", "%66%6F%6F"),
        ("fo+o/bar", "fo+o", "fo+o"),
        ("fo%2Bo/bar", "fo+o", "fo%2Bo"),
    ] {
        let mut request = Request::default();
        assert_eq!(
            Some(expect.as_bytes().to_vec()),
            request.get_urlencoded_prefix(string.as_bytes(), prefix.as_bytes())?
        );
    }
    Ok(())
}

#[test]
fn testTrustedProxiesXForwardedFor() -> TestResult {
    let _t = TearDown;
    let mut request = Request::create("http://example.com/")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("3.3.3.3"));
    set_header(&mut request, "X_FORWARDED_FOR", "1.1.1.1, 2.2.2.2")?;
    set_header(
        &mut request,
        "X_FORWARDED_HOST",
        "foo.example.com:1234, real.example.com:8080",
    )?;
    set_header(&mut request, "X_FORWARDED_PROTO", "https")?;
    set_header(&mut request, "X_FORWARDED_PORT", "443")?;

    let untrusted = |request: &mut Request| -> TestResult {
        assert_eq!(some("3.3.3.3"), request.get_client_ip()?);
        assert_eq!("example.com", text(&request.get_host()?));
        assert_int(80, &request.get_port()?, "untrusted");
        assert!(!request.is_secure()?);
        Ok(())
    };
    let all = Request::HEADER_X_FORWARDED_FOR
        | Request::HEADER_X_FORWARDED_HOST
        | Request::HEADER_X_FORWARDED_PORT
        | Request::HEADER_X_FORWARDED_PROTO;

    // no trusted proxies
    untrusted(&mut request)?;

    // disabling proxy trusting
    Request::set_trusted_proxies(Vec::<&str>::new(), Request::HEADER_X_FORWARDED_FOR)?;
    untrusted(&mut request)?;

    // request is forwarded by a non-trusted proxy
    Request::set_trusted_proxies(["2.2.2.2"], Request::HEADER_X_FORWARDED_FOR)?;
    untrusted(&mut request)?;

    // trusted proxy via setTrustedProxies()
    Request::set_trusted_proxies(["3.3.3.3", "2.2.2.2"], all)?;
    assert_eq!(some("1.1.1.1"), request.get_client_ip()?);
    assert_eq!("foo.example.com", text(&request.get_host()?));
    assert_int(443, &request.get_port()?, "trusted");
    assert!(request.is_secure()?);

    // trusted proxy via setTrustedProxies()
    Request::set_trusted_proxies(["3.3.3.4", "2.2.2.2"], all)?;
    untrusted(&mut request)?;

    // check various X_FORWARDED_PROTO header values
    Request::set_trusted_proxies(["3.3.3.3", "2.2.2.2"], Request::HEADER_X_FORWARDED_PROTO)?;
    set_header(&mut request, "X_FORWARDED_PROTO", "ssl")?;
    assert!(request.is_secure()?);

    set_header(&mut request, "X_FORWARDED_PROTO", "https, http")?;
    assert!(request.is_secure()?);
    Ok(())
}

#[test]
fn testTrustedProxiesForwarded() -> TestResult {
    let _t = TearDown;
    let mut request = Request::create("http://example.com/")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("3.3.3.3"));
    set_header(
        &mut request,
        "FORWARDED",
        "for=1.1.1.1, host=foo.example.com:8080, proto=https, for=2.2.2.2, host=real.example.com:8080",
    )?;

    let untrusted = |request: &mut Request| -> TestResult {
        assert_eq!(some("3.3.3.3"), request.get_client_ip()?);
        assert_eq!("example.com", text(&request.get_host()?));
        assert_int(80, &request.get_port()?, "untrusted");
        assert!(!request.is_secure()?);
        Ok(())
    };

    // no trusted proxies
    untrusted(&mut request)?;

    // disabling proxy trusting
    Request::set_trusted_proxies(Vec::<&str>::new(), Request::HEADER_FORWARDED)?;
    untrusted(&mut request)?;

    // request is forwarded by a non-trusted proxy
    Request::set_trusted_proxies(["2.2.2.2"], Request::HEADER_FORWARDED)?;
    untrusted(&mut request)?;

    // trusted proxy via setTrustedProxies()
    Request::set_trusted_proxies(["3.3.3.3", "2.2.2.2"], Request::HEADER_FORWARDED)?;
    assert_eq!(some("1.1.1.1"), request.get_client_ip()?);
    assert_eq!("foo.example.com", text(&request.get_host()?));
    assert_int(8080, &request.get_port()?, "trusted");
    assert!(request.is_secure()?);

    // trusted proxy via setTrustedProxies()
    Request::set_trusted_proxies(["3.3.3.4", "2.2.2.2"], Request::HEADER_FORWARDED)?;
    untrusted(&mut request)?;

    // check various X_FORWARDED_PROTO header values
    Request::set_trusted_proxies(["3.3.3.3", "2.2.2.2"], Request::HEADER_FORWARDED)?;
    set_header(&mut request, "FORWARDED", "proto=ssl")?;
    assert!(request.is_secure()?);

    set_header(&mut request, "FORWARDED", "proto=https, proto=http")?;
    assert!(request.is_secure()?);
    Ok(())
}

#[test]
fn testIISRequestUri() -> TestResult {
    let _t = TearDown;
    for (entries, expected) in [
        (
            &[("IIS_WasUrlRewritten", "1"), ("UNENCODED_URL", "/foo/bar")][..],
            "/foo/bar",
        ),
        (&[("ORIG_PATH_INFO", "/foo/bar")], "/foo/bar"),
        (
            &[("ORIG_PATH_INFO", "/foo/bar"), ("QUERY_STRING", "foo=bar")],
            "/foo/bar?foo=bar",
        ),
    ] {
        let mut request = Request::default();
        request.headers_mut().replace(Vec::<(&str, &str)>::new())?;
        request.server_mut().replace(server(entries));
        assert_eq!(
            expected,
            text(&request.get_request_uri()?),
            "->getRequestUri() is correct"
        );

        let sub_request_uri = "/bar/foo";
        let mut sub_request = Request::create_with(
            sub_request_uri,
            Create {
                method: Some(s("get")),
                server: request.server().all().clone(),
                ..Create::default()
            },
        )?;
        assert_eq!(
            sub_request_uri,
            text(&sub_request.get_request_uri()?),
            "->getRequestUri() is correct in sub request"
        );
    }
    Ok(())
}

fn with_host(host: &str) -> Result<Request, Error> {
    let mut request = Request::create("/")?;
    set_header(&mut request, "host", host)?;
    Ok(request)
}

#[test]
fn testTrustedHosts() -> TestResult {
    let _t = TearDown;
    // create a request
    let mut request = Request::create("/")?;

    // no trusted host set -> no host check
    set_header(&mut request, "host", "evil.com")?;
    assert_eq!("evil.com", text(&request.get_host()?));

    // add a trusted domain and all its subdomains
    Request::set_trusted_hosts([r"^([a-z]{9}\.)?trusted\.com$"])?;

    // untrusted host
    set_header(&mut request, "host", "evil.com")?;
    match request.get_host() {
        Ok(host) => panic!(
            "Request::getHost() should throw an exception when host is not trusted, got {host:?}"
        ),
        Err(e) => {
            assert!(e.is_a(SUSPICIOUS));
            assert_eq!("Untrusted Host \"evil.com\".", text(e.message()));
        }
    }

    // trusted hosts
    set_header(&mut request, "host", "trusted.com")?;
    assert_eq!("trusted.com", text(&request.get_host()?));
    assert_int(80, &request.get_port()?, "trusted.com");

    set_server(&mut request, "HTTPS", Value::Bool(true));
    set_header(&mut request, "host", "trusted.com")?;
    assert_eq!("trusted.com", text(&request.get_host()?));
    assert_int(443, &request.get_port()?, "HTTPS");
    set_server(&mut request, "HTTPS", Value::Bool(false));

    set_header(&mut request, "host", "trusted.com:8000")?;
    assert_eq!("trusted.com", text(&request.get_host()?));
    assert_int(8000, &request.get_port()?, "trusted.com:8000");

    set_header(&mut request, "host", "subdomain.trusted.com")?;
    assert_eq!("subdomain.trusted.com", text(&request.get_host()?));
    Ok(())
}

#[test]
fn testSetTrustedHostsDoesNotBreakOnSpecialCharacters() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_hosts([r"localhost(\.local){0,1}#,example.com", "localhost"])?;
    let mut request = with_host("localhost")?;
    assert_eq!("localhost", text(&request.get_host()?));
    Ok(())
}

#[test]
fn testSetTrustedHostsKeepsPatternsIndependent() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_hosts([r"^(a)\.example\.com$", r"^(b)\.\1\.example\.com$"])?;
    let mut request = with_host("b.b.example.com")?;
    assert_eq!("b.b.example.com", text(&request.get_host()?));
    Ok(())
}

#[test]
fn testTrustedHostsAreNotAccumulated() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_hosts([r"^[a-z]+\.example\.com$"])?;
    let mut request = Request::create("/")?;
    set_header(&mut request, "host", "a.example.com")?;
    assert_eq!("a.example.com", text(&request.get_host()?));
    set_header(&mut request, "host", "b.example.com")?;
    assert_eq!("b.example.com", text(&request.get_host()?));
    // upstream also checks that its legacy `$trustedHosts` cache stays
    // empty; the port has no such cache
    Ok(())
}

#[test]
fn testTrustedHostsAreNotMatchedLoosely() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_hosts(["^123$"])?;
    let mut request = with_host("123")?;
    assert_eq!("123", text(&request.get_host()?));

    let mut request = with_host("0123")?;
    expect_err(request.get_host(), SUSPICIOUS, "Untrusted Host \"0123\".");
    Ok(())
}

#[test]
fn testTrustedHostsWithManyPatterns() -> TestResult {
    let _t = TearDown;
    let patterns: Vec<String> = (0..5000)
        .map(|i| format!(r"^customer-{i}\.[a-z]+\.example\.com$"))
        .collect();
    Request::set_trusted_hosts(patterns.iter().map(String::as_str))?;

    let mut request = Request::create("/")?;
    set_header(&mut request, "host", "customer-0.eu.example.com")?;
    assert_eq!("customer-0.eu.example.com", text(&request.get_host()?));
    set_header(&mut request, "host", "customer-4999.eu.example.com")?;
    assert_eq!("customer-4999.eu.example.com", text(&request.get_host()?));

    let mut request = with_host("evil.com")?;
    expect_err(
        request.get_host(),
        SUSPICIOUS,
        "Untrusted Host \"evil.com\".",
    );
    Ok(())
}

#[test]
fn testTrustedHostsWithManyConstantPatterns() -> TestResult {
    let _t = TearDown;
    let patterns: Vec<String> = (0..5000)
        .map(|i| format!(r"^customer-{i}\.example\.com$"))
        .collect();
    Request::set_trusted_hosts(patterns.iter().map(String::as_str))?;
    assert_eq!(
        Request::trusted_hosts_regexps(),
        [] as [php_value::PhpString; 0]
    );

    let mut request = Request::create("/")?;
    set_header(&mut request, "host", "customer-0.example.com")?;
    assert_eq!("customer-0.example.com", text(&request.get_host()?));
    set_header(&mut request, "host", "CUSTOMER-4999.EXAMPLE.COM")?;
    assert_eq!("customer-4999.example.com", text(&request.get_host()?));

    let mut request = with_host("evil.com")?;
    expect_err(
        request.get_host(),
        SUSPICIOUS,
        "Untrusted Host \"evil.com\".",
    );
    Ok(())
}

#[test]
fn testTrustedHostsWithMetaCharactersAreMatchedAsRegexps() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_hosts([r"^a.example\.com$", r"^customer-\d+\.example\.org$"])?;
    let mut request = with_host("axexample.com")?;
    assert_eq!("axexample.com", text(&request.get_host()?));
    set_header(&mut request, "host", "customer-42.example.org")?;
    assert_eq!("customer-42.example.org", text(&request.get_host()?));
    Ok(())
}

#[test]
fn testVeryLongHosts() -> TestResult {
    let _t = TearDown;
    let host = format!("a{}", ".a".repeat(40000));
    let start = std::time::Instant::now();
    let mut request = with_host(&host)?;
    assert_eq!(host, text(&request.get_host()?));
    assert!(start.elapsed().as_secs_f64() < 5.0);
    Ok(())
}

#[test]
fn testHostValidity() -> TestResult {
    let _t = TearDown;
    let rows: [(&[u8], bool, Option<&str>, Option<i64>); 7] = [
        (b".a", false, None, None),
        (b"a..", false, None, None),
        (b"a.", true, None, None),
        (b"\xE9", false, None, None),
        (b"[::1]", true, None, None),
        (b"[::1]:80", true, Some("[::1]"), Some(80)),
        (&[b'.'; 101], false, None, None),
    ];
    for (host, is_valid, expected_host, expected_port) in rows {
        let mut request = Request::create("/")?;
        request
            .headers_mut()
            .set(b"host", PhpString::from(host), true)?;
        if is_valid {
            let expected = expected_host.map_or_else(|| PhpString::from(host), s);
            assert_eq!(expected, request.get_host()?);
            if let Some(port) = expected_port {
                assert_eq!(Value::Int(port), request.get_port()?);
            }
        } else {
            expect_err(request.get_host(), SUSPICIOUS, "Invalid Host");
        }
    }
    Ok(())
}

fn method_rows(expected: [bool; 11]) -> [(&'static str, bool); 11] {
    let methods = [
        "HEAD", "GET", "POST", "PUT", "PATCH", "DELETE", "PURGE", "OPTIONS", "TRACE", "CONNECT",
        "QUERY",
    ];
    let mut rows = [("", false); 11];
    for (i, method) in methods.into_iter().enumerate() {
        rows[i] = (method, expected[i]);
    }
    rows
}

#[test]
fn testMethodIdempotent() -> TestResult {
    let _t = TearDown;
    for (method, idempotent) in method_rows([
        true, true, false, true, false, true, true, true, true, false, true,
    ]) {
        let mut request = Request::default();
        request.set_method(method);
        assert_eq!(idempotent, request.is_method_idempotent()?, "{method}");
    }
    Ok(())
}

#[test]
fn testMethodSafe() -> TestResult {
    let _t = TearDown;
    for (method, safe) in method_rows([
        true, true, false, false, false, false, false, true, true, false, true,
    ]) {
        let mut request = Request::default();
        request.set_method(method);
        assert_eq!(safe, request.is_method_safe()?, "{method}");
    }
    Ok(())
}

#[test]
fn testMethodCacheable() -> TestResult {
    let _t = TearDown;
    for (method, cacheable) in method_rows([
        true, true, false, false, false, false, false, false, false, false, true,
    ]) {
        let mut request = Request::default();
        request.set_method(method);
        assert_eq!(cacheable, request.is_method_cacheable()?, "{method}");
    }
    Ok(())
}

#[test]
fn testProtocolVersion() -> TestResult {
    let _t = TearDown;
    for (server_protocol, trusted_proxy, via, expected) in [
        ("HTTP/2.0", false, Some(""), "HTTP/2.0"),
        ("HTTP/2.0", false, None, "HTTP/2.0"),
        (
            "HTTP/2.0",
            false,
            Some("1.0 fred, 1.1 nowhere.com (Apache/1.1)"),
            "HTTP/2.0",
        ),
        ("HTTP/2.0", true, Some(""), "HTTP/2.0"),
        ("HTTP/2.0", true, None, "HTTP/2.0"),
        (
            "HTTP/2.0",
            true,
            Some("1.0 fred, 1.1 nowhere.com (Apache/1.1)"),
            "HTTP/1.0",
        ),
        (
            "HTTP/2.0",
            true,
            Some("HTTP/1.0 fred, HTTP/1.1 nowhere.com (Apache/1.1)"),
            "HTTP/1.0",
        ),
        ("HTTP/2.0", true, Some("HTTP/1^0 foo"), "HTTP/2.0"),
        ("HTTP/2.0", true, Some("1.0 fred, foo"), "HTTP/1.0"),
        ("HTTP/2.0", true, Some("HTTP/1.0"), "HTTP/1.0"),
        ("HTTP/2.0", true, Some("1.0"), "HTTP/1.0"),
    ] {
        Request::reset_statics();
        if trusted_proxy {
            Request::set_trusted_proxies(["1.1.1.1"], -1)?;
        }
        let mut request = Request::default();
        set_server(
            &mut request,
            "SERVER_PROTOCOL",
            Value::string(server_protocol),
        );
        set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
        if let Some(via) = via {
            set_header(&mut request, "Via", via)?;
        }
        assert_eq!(
            some(expected),
            request.get_protocol_version()?,
            "{trusted_proxy} {via:?}"
        );
    }
    Ok(())
}

#[test]
fn testNonstandardRequests() -> TestResult {
    let _t = TearDown;
    let rows: [(&str, &str, &str, &str, &str, Option<&str>); 15] = [
        ("", "", "/", "http://host:8080/", "", None),
        ("/", "", "/", "http://host:8080/", "", None),
        (
            "hello/app.php/x",
            "",
            "/x",
            "http://host:8080/hello/app.php/x",
            "/hello",
            Some("/hello/app.php"),
        ),
        (
            "/hello/app.php/x",
            "",
            "/x",
            "http://host:8080/hello/app.php/x",
            "/hello",
            Some("/hello/app.php"),
        ),
        ("", "a=b", "/", "http://host:8080/?a=b", "", None),
        ("?a=b", "a=b", "/", "http://host:8080/?a=b", "", None),
        ("/?a=b", "a=b", "/", "http://host:8080/?a=b", "", None),
        ("x", "a=b", "/x", "http://host:8080/x?a=b", "", None),
        ("x?a=b", "a=b", "/x", "http://host:8080/x?a=b", "", None),
        ("/x?a=b", "a=b", "/x", "http://host:8080/x?a=b", "", None),
        (
            "hello/x",
            "",
            "/x",
            "http://host:8080/hello/x",
            "/hello",
            None,
        ),
        (
            "/hello/x",
            "",
            "/x",
            "http://host:8080/hello/x",
            "/hello",
            None,
        ),
        (
            "hello/app.php/x",
            "a=b",
            "/x",
            "http://host:8080/hello/app.php/x?a=b",
            "/hello",
            Some("/hello/app.php"),
        ),
        (
            "hello/app.php/x?a=b",
            "a=b",
            "/x",
            "http://host:8080/hello/app.php/x?a=b",
            "/hello",
            Some("/hello/app.php"),
        ),
        (
            "/hello/app.php/x?a=b",
            "a=b",
            "/x",
            "http://host:8080/hello/app.php/x?a=b",
            "/hello",
            Some("/hello/app.php"),
        ),
    ];
    for (
        request_uri,
        query_string,
        expected_path_info,
        expected_uri,
        expected_base_path,
        expected_base_url,
    ) in rows
    {
        let expected_base_url = expected_base_url.unwrap_or(expected_base_path);
        let mut request = with_server(&[
            ("HTTP_HOST", "host:8080"),
            ("SERVER_PORT", "8080"),
            ("QUERY_STRING", query_string),
            ("PHP_SELF", "/hello/app.php"),
            ("SCRIPT_FILENAME", "/some/path/app.php"),
            ("REQUEST_URI", request_uri),
        ])?;
        assert_eq!(
            expected_path_info,
            text(&request.get_path_info()?),
            "{request_uri}"
        );
        assert_eq!(expected_uri, text(&request.get_uri()?), "{request_uri}");
        // assertEquals('', null)
        assert_eq!(
            query_string,
            request
                .get_query_string()?
                .as_ref()
                .map(text)
                .unwrap_or_default()
        );
        assert_int(8080, &request.get_port()?, request_uri);
        assert_eq!("host:8080", text(&request.get_http_host()?));
        assert_eq!(
            expected_base_url,
            text(&request.get_base_url()?),
            "{request_uri}"
        );
        assert_eq!(
            expected_base_path,
            text(&request.get_base_path()?),
            "{request_uri}"
        );
    }
    Ok(())
}

fn trusted_request() -> Result<Request, Error> {
    let mut request = Request::create("/")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    Ok(request)
}

#[test]
fn testTrustedHost() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["1.1.1.1"], -1)?;

    let mut request = trusted_request()?;
    set_header(&mut request, "Forwarded", "host=localhost:8080")?;
    set_header(&mut request, "X-Forwarded-Host", "localhost:8080")?;
    assert_eq!("localhost:8080", text(&request.get_http_host()?));
    assert_eq!(Value::Int(8080), request.get_port()?);

    let mut request = trusted_request()?;
    set_header(&mut request, "Forwarded", r#"host="[::1]:443""#)?;
    set_header(&mut request, "X-Forwarded-Host", "[::1]:443")?;
    set_header(&mut request, "X-Forwarded-Port", "443")?;
    assert_eq!("[::1]:443", text(&request.get_http_host()?));
    assert_eq!(Value::Int(443), request.get_port()?);
    Ok(())
}

#[test]
fn testTrustedPrefix() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["1.1.1.1"], Request::HEADER_X_FORWARDED_TRAEFIK)?;

    // test with index deployed under root
    let mut request = Request::create("/method")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    set_header(&mut request, "X-Forwarded-Prefix", "/myprefix")?;
    set_header(&mut request, "Forwarded", "host=localhost:8080")?;
    assert_eq!("/myprefix", text(&request.get_base_url()?));
    assert_eq!("/myprefix", text(&request.get_base_path()?));
    assert_eq!("/method", text(&request.get_path_info()?));
    Ok(())
}

#[test]
fn testTrustedPrefixWithSubdir() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["1.1.1.1"], Request::HEADER_X_FORWARDED_TRAEFIK)?;

    // test with index file deployed in subdir, i.e. local dev server (insecure!!)
    let mut request = create_with_server(
        "/public/method",
        "GET",
        &[
            ("SCRIPT_FILENAME", "/var/hidden/app/public/public/index.php"),
            ("SCRIPT_NAME", "/public/index.php"),
            ("PHP_SELF", "/public/index.php"),
        ],
    )?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    set_header(&mut request, "X-Forwarded-Prefix", "/prefix")?;
    set_header(&mut request, "Forwarded", "host=localhost:8080")?;
    assert_eq!("/prefix/public", text(&request.get_base_url()?));
    assert_eq!("/prefix/public", text(&request.get_base_path()?));
    assert_eq!("/method", text(&request.get_path_info()?));
    Ok(())
}

#[test]
fn testTrustedPrefixEmpty() -> TestResult {
    let _t = TearDown;
    // check that there is no error, if no prefix is provided
    Request::set_trusted_proxies(["1.1.1.1"], Request::HEADER_X_FORWARDED_TRAEFIK)?;
    let mut request = Request::create("/method")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("1.1.1.1"));
    assert_eq!("", text(&request.get_base_url()?));
    Ok(())
}

#[test]
fn testTrustedPort() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["1.1.1.1"], -1)?;

    let mut request = trusted_request()?;
    set_header(&mut request, "Forwarded", "host=localhost:8080")?;
    set_header(&mut request, "X-Forwarded-Port", "8080")?;
    assert_eq!(Value::Int(8080), request.get_port()?);

    let mut request = trusted_request()?;
    set_header(&mut request, "Forwarded", "host=localhost")?;
    set_header(&mut request, "X-Forwarded-Port", "80")?;
    assert_eq!(Value::Int(80), request.get_port()?);

    let mut request = trusted_request()?;
    set_header(&mut request, "Forwarded", r#"host="[::1]""#)?;
    set_header(&mut request, "X-Forwarded-Proto", "https")?;
    set_header(&mut request, "X-Forwarded-Port", "443")?;
    assert_eq!(Value::Int(443), request.get_port()?);
    Ok(())
}

#[test]
fn testTrustedPortDoesNotDefaultToZero() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["1.1.1.1"], Request::HEADER_X_FORWARDED_FOR)?;
    let mut request = trusted_request()?;
    set_header(&mut request, "X-Forwarded-Host", "test.example.com")?;
    set_header(&mut request, "X-Forwarded-Port", "")?;
    assert_eq!(Value::Int(80), request.get_port()?);
    Ok(())
}

#[test]
fn testTrustedProxiesRemoteAddr() -> TestResult {
    let _t = TearDown;
    for (server_remote_addr, trusted_proxies, result) in [
        (Some("1.1.1.1"), &["REMOTE_ADDR"][..], &["1.1.1.1"][..]),
        (
            Some("1.1.1.1"),
            &["REMOTE_ADDR", "2.2.2.2"],
            &["1.1.1.1", "2.2.2.2"],
        ),
        (None, &["REMOTE_ADDR"], &[]),
        (None, &["REMOTE_ADDR", "2.2.2.2"], &["2.2.2.2"]),
    ] {
        with_globals(|g| {
            g.server.insert(
                "REMOTE_ADDR",
                server_remote_addr.map_or(Value::Null, Value::string),
            );
        });
        Request::set_trusted_proxies(
            trusted_proxies.iter().copied(),
            Request::HEADER_X_FORWARDED_FOR,
        )?;
        assert_eq!(strings(result), Request::get_trusted_proxies());
    }
    Ok(())
}

#[test]
fn testTrustedProxiesPrivateSubnets() -> TestResult {
    let _t = TearDown;
    for key in ["PRIVATE_SUBNETS", "private_ranges"] {
        Request::set_trusted_proxies([key], Request::HEADER_X_FORWARDED_FOR)?;
        assert_eq!(
            strings(&ip_utils::PRIVATE_RANGES),
            Request::get_trusted_proxies()
        );
    }
    Ok(())
}

#[test]
fn testPrivateRangesDoNotTrustAddressesEmbeddingARemoteIpv4() -> TestResult {
    let _t = TearDown;
    Request::set_trusted_proxies(["private_ranges"], Request::HEADER_X_FORWARDED_FOR)?;

    for remote_addr in [
        "::ffff:8.8.8.8",
        "::8.8.8.8",
        "2002:808:808::1",
        "2001:0:4136:e378:8000:63bf:3fff:fdd2",
        "64:ff9b::8.8.8.8",
    ] {
        let mut request = Request::create("/")?;
        set_server(&mut request, "REMOTE_ADDR", Value::string(remote_addr));
        set_header(&mut request, "X-Forwarded-For", "1.2.3.4")?;
        assert!(!request.is_from_trusted_proxy()?, "{remote_addr}");
        assert_eq!(some(remote_addr), request.get_client_ip()?);
    }

    for remote_addr in ["10.0.0.5", "::ffff:10.0.0.5", "::ffff:127.0.0.1", "fd00::1"] {
        let mut request = Request::create("/")?;
        set_server(&mut request, "REMOTE_ADDR", Value::string(remote_addr));
        set_header(&mut request, "X-Forwarded-For", "1.2.3.4")?;
        assert!(request.is_from_trusted_proxy()?, "{remote_addr}");
        assert_eq!(some("1.2.3.4"), request.get_client_ip()?);
    }
    Ok(())
}

#[test]
fn testTrustedValuesCache() -> TestResult {
    let _t = TearDown;
    let mut request = Request::create("http://example.com/")?;
    set_server(&mut request, "REMOTE_ADDR", Value::string("3.3.3.3"));
    set_header(&mut request, "X_FORWARDED_FOR", "1.1.1.1, 2.2.2.2")?;
    set_header(&mut request, "X_FORWARDED_PROTO", "https")?;
    assert!(!request.is_secure()?);

    Request::set_trusted_proxies(
        ["3.3.3.3", "2.2.2.2"],
        Request::HEADER_X_FORWARDED_FOR
            | Request::HEADER_X_FORWARDED_HOST
            | Request::HEADER_X_FORWARDED_PORT
            | Request::HEADER_X_FORWARDED_PROTO,
    )?;
    assert!(request.is_secure()?);

    // Header is changed, cache must not be hit now
    set_header(&mut request, "X_FORWARDED_PROTO", "http")?;
    assert!(!request.is_secure()?);
    Ok(())
}

#[test]
fn testPreferSafeContent() -> TestResult {
    let _t = TearDown;
    for (entries, expected) in [
        (&[][..], false),
        (&[("HTTPS", "on")], false),
        (&[("HTTPS", "off"), ("HTTP_PREFER", "safe")], false),
        (&[("HTTPS", "on"), ("HTTP_PREFER", "safe")], true),
        (
            &[("HTTPS", "on"), ("HTTP_PREFER", "unknown-preference")],
            false,
        ),
        (
            &[
                ("HTTPS", "on"),
                ("HTTP_PREFER", "unknown-preference=42, safe"),
            ],
            true,
        ),
        (
            &[
                ("HTTPS", "on"),
                ("HTTP_PREFER", "safe, unknown-preference=42"),
            ],
            true,
        ),
    ] {
        let mut request = with_server(entries)?;
        assert_eq!(expected, request.prefer_safe_content()?, "{entries:?}");
    }
    Ok(())
}

#[test]
fn testReservedFlags() {
    let _t = TearDown;
    for (constant, value) in Request::INT_CONSTANTS {
        assert_ne!(
            0b1000_0000, value,
            "The constant \"{constant}\" should not use the reserved value \"0b10000000\"."
        );
    }
}

#[test]
fn testMalformedUrls() {
    let _t = TearDown;
    let rows: [(&[u8], &str); 20] = [
        (
            b"http://normal.com[@vulndetector.com/",
            "Invalid URI: Userinfo is malformed.",
        ),
        (
            b"http://[normal.com@vulndetector.com/",
            "Invalid URI: Userinfo is malformed.",
        ),
        (
            b"http://normal.com@[vulndetector.com/",
            "Invalid URI: Host is malformed.",
        ),
        (
            b"http://[[normal.com@][vulndetector.com/",
            "Invalid URI: Userinfo is malformed.",
        ),
        (
            b"http://[vulndetector.com]",
            "Invalid URI: Host is malformed.",
        ),
        (
            b"http://[0:0::vulndetector.com]:80",
            "Invalid URI: Host is malformed.",
        ),
        (
            b"http://[2001:db8::vulndetector.com]",
            "Invalid URI: Host is malformed.",
        ),
        (b"http://[malicious.com]", "Invalid URI: Host is malformed."),
        (b"http://[evil.org]", "Invalid URI: Host is malformed."),
        (
            b"http://[internal.server]",
            "Invalid URI: Host is malformed.",
        ),
        (b"http://[192.168.1.1]", "Invalid URI: Host is malformed."),
        (b"http://192.abc.1.1", "Invalid URI: Host is malformed."),
        (b"http://[localhost]", "Invalid URI: Host is malformed."),
        (
            b"\x80https://example.com",
            "Invalid URI: Scheme is malformed.",
        ),
        (b">https://example.com", "Invalid URI: Scheme is malformed."),
        (
            b"http\x0b://example.com",
            "Invalid URI: Scheme is malformed.",
        ),
        (
            b"https\x80://example.com",
            "Invalid URI: Scheme is malformed.",
        ),
        (b"http>://example.com", "Invalid URI: Scheme is malformed."),
        (b"0http://example.com", "Invalid URI: Scheme is malformed."),
        (b":path", "Invalid URI: Path is malformed."),
    ];
    for (url, expected) in rows {
        expect_err(Request::create(url), BAD_REQUEST, expected);
    }
}

#[test]
fn testLegitimateUrls() -> TestResult {
    let _t = TearDown;
    for url in [
        "http://example.com",
        "https://example.com",
        "http://example.com:8080",
        "https://example.com:8443",
        "http://user:pass@example.com",
        "http://user:pass@example.com:8080",
        "http://user:pass@example.com/path",
        "http://[2001:db8::1]",
        "http://[2001:db8::1]:8080",
        "http://[2001:db8::1]/path",
        "http://[::1]",
        "http://example.com/path",
    ] {
        Request::create(url)?;
    }
    Ok(())
}

#[test]
fn testGetAcceptableContentTypesRfc9110() -> TestResult {
    let _t = TearDown;
    let rows: [(&str, &[&str]); 18] = [
        (
            "text/html;q=0.9, application/json;q=0.8, text/plain;q=1.0",
            &["text/plain", "text/html", "application/json"],
        ),
        (
            "text/plain, text/plain;format=flowed",
            &["text/plain", "text/plain;format=flowed"],
        ),
        (
            "text/*;q=0.3, text/plain;q=0.7, text/plain;format=flowed, text/plain;format=fixed;q=0.4, */*;q=0.5",
            &[
                "text/plain;format=flowed",
                "text/plain",
                "*/*",
                "text/plain;format=fixed",
                "text/*",
            ],
        ),
        (
            "text/html;level=1, text/html;level=2;q=0.4, text/html;q=0.7, text/*;q=0.3",
            &[
                "text/html;level=1",
                "text/html",
                "text/html;level=2",
                "text/*",
            ],
        ),
        (
            "text/plain;format=flowed;q=0.8, text/plain;Format=fixed",
            &["text/plain;format=fixed", "text/plain;format=flowed"],
        ),
        (
            "text/plain;Charset=utf-8, text/plain;charset=iso-8859-1;q=0.8",
            &["text/plain;charset=utf-8", "text/plain;charset=iso-8859-1"],
        ),
        (
            r#"text/plain;param="value with space""#,
            &[r#"text/plain;param="value with space""#],
        ),
        (
            r#"text/plain;param="value;with=special,chars""#,
            &[r#"text/plain;param="value;with=special,chars""#],
        ),
        (
            "text/*;format=flowed, text/plain",
            &["text/*;format=flowed", "text/plain"],
        ),
        ("*/*;q=0.5, text/html;q=0.9", &["text/html", "*/*"]),
        (
            "application/json, application/xml, text/html",
            &["application/json", "application/xml", "text/html"],
        ),
        (
            "text/plain;format=flowed;q=0.8, text/plain;format=fixed;q=0.8",
            &["text/plain;format=flowed", "text/plain;format=fixed"],
        ),
        (
            "text/html, application/xhtml+xml, application/xml;q=0.9, text/*;q=0.8, */*;q=0.7",
            &[
                "text/html",
                "application/xhtml+xml",
                "application/xml",
                "text/*",
                "*/*",
            ],
        ),
        (
            "text/plain, text/plain;charset=utf-8",
            &["text/plain", "text/plain;charset=utf-8"],
        ),
        (
            "text/plain;charset=utf-8;format=flowed, text/plain;charset=utf-8, text/plain",
            &[
                "text/plain;charset=utf-8;format=flowed",
                "text/plain;charset=utf-8",
                "text/plain",
            ],
        ),
        ("*/*", &["*/*"]),
        ("text/*", &["text/*"]),
        (
            "Text/HTML, Application/JSON",
            &["Text/HTML", "Application/JSON"],
        ),
    ];
    for (accept, expected) in rows {
        let mut request = Request::default();
        set_header(&mut request, "Accept", accept)?;
        assert_eq!(
            strings(expected),
            request.get_acceptable_content_types()?,
            "{accept}"
        );
    }
    Ok(())
}

#[test]
fn testGetPreferredFormatRfc9110() -> TestResult {
    let _t = TearDown;
    let rows: [(&str, Option<&str>, Option<&str>); 21] = [
        ("application/json;charset=utf-8", Some("json"), Some("html")),
        ("application/xml;version=1.0", Some("xml"), Some("html")),
        (
            "application/json;q=0.9, application/xml;q=0.8",
            Some("json"),
            Some("html"),
        ),
        (
            "application/xml;q=0.9, application/json;q=0.8",
            Some("xml"),
            Some("html"),
        ),
        (
            "application/json, application/json;charset=utf-8",
            Some("json"),
            Some("html"),
        ),
        ("text/html;level=1, application/json", Some("html"), None),
        ("text/*", Some("html"), Some("html")),
        ("*/*", Some("html"), Some("html")),
        (
            "application/json;q=0.9, */*;q=0.5",
            Some("json"),
            Some("html"),
        ),
        (
            "text/html;q=0.9, application/xhtml+xml, application/xml;q=0.8, text/*;q=0.7",
            Some("html"),
            Some("html"),
        ),
        (
            "application/json;charset=utf-8;profile=strict, text/html",
            Some("json"),
            Some("html"),
        ),
        ("application/json", Some("json"), Some("html")),
        ("application/json;charset=utf-8", Some("json"), Some("html")),
        (
            r#"application/json;profile="http://example.com/schema""#,
            Some("json"),
            Some("html"),
        ),
        (
            "application/json;q=0.9, application/xml;q=0.9",
            Some("json"),
            Some("html"),
        ),
        ("application/vnd.custom+unknown", Some("html"), Some("html")),
        ("application/vnd.custom+unknown", None, None),
        ("", Some("html"), Some("html")),
        (
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            Some("html"),
            Some("html"),
        ),
        (
            "application/json, text/plain, */*",
            Some("json"),
            Some("html"),
        ),
        (
            "application/atom+xml;q=0.9, application/rss+xml;q=0.8",
            Some("atom"),
            Some("html"),
        ),
    ];
    for (accept, expected, default) in rows {
        let mut request = Request::default();
        set_header(&mut request, "Accept", accept)?;
        assert_eq!(
            expected.map(s),
            request.get_preferred_format(default)?,
            "{accept}"
        );
    }
    Ok(())
}

#[test]
fn testDirectPropertyWriteIsDeprecated() {
    let _t = TearDown;
    let extra = |what: &str| {
        format!("pass {what} as a constructor argument or call \"initialize()\" instead.")
    };
    let rows: Vec<(&str, Box<dyn Fn(&mut Request)>, String)> = vec![
        (
            "attributes",
            Box::new(|r: &mut Request| r.set_attributes(ParameterBag::new(php_array!["k" => "v"]))),
            extra("attributes"),
        ),
        (
            "request",
            Box::new(|r: &mut Request| r.set_request(InputBag::new(php_array!["k" => "v"]))),
            extra("the POST data"),
        ),
        (
            "query",
            Box::new(|r: &mut Request| r.set_query(InputBag::new(php_array!["k" => "v"]))),
            extra("query parameters"),
        ),
        (
            "server",
            Box::new(|r: &mut Request| r.set_server(ServerBag::new(php_array!["k" => "v"]))),
            extra("server parameters"),
        ),
        (
            "files",
            Box::new(|r: &mut Request| r.set_files(FileBag::default())),
            extra("files"),
        ),
        (
            "cookies",
            Box::new(|r: &mut Request| r.set_cookies(InputBag::new(php_array!["k" => "v"]))),
            extra("cookies"),
        ),
        (
            "headers",
            Box::new(|r: &mut Request| r.set_headers(HeaderBag::new([("k", "v")]))),
            extra("header parameters"),
        ),
    ];
    for (property, write, extra) in rows {
        let mut request = Request::default();
        let ((), deprecations) = symfony_deprecation_contracts::collect(|| write(&mut request));
        let expected = format!(
            "Since symfony/http-foundation 8.1: Directly setting property \"{property}\" of \"Symfony\\Component\\HttpFoundation\\Request\" is deprecated; {extra}"
        );
        assert_eq!(
            vec![expected.into_bytes()],
            deprecations
                .into_iter()
                .map(|d| d.message)
                .collect::<Vec<_>>()
        );
    }
}

/// `setFactory()` without upstream's `NewRequest` subclass: `create()` and
/// `createFromGlobals()` go through the factory, and `setFactory(null)`
/// restores `new static(...)`.
#[test]
fn factory_creates_the_request() -> TestResult {
    let _t = TearDown;
    Request::set_factory(Some(std::rc::Rc::new(|parameters: Parameters| {
        let mut request = Request::new(parameters)?;
        request.attributes_mut().set("foo", Value::string("foo"));
        Ok(request)
    })));
    assert_eq!(
        Value::string("foo"),
        Request::create("/")?.attributes().get("foo", Value::Null)
    );
    assert_eq!(
        Value::string("foo"),
        Request::create_from_globals()?
            .attributes()
            .get("foo", Value::Null)
    );
    Request::set_factory(None);
    assert!(!Request::create("/")?.attributes().has("foo"));
    Ok(())
}
