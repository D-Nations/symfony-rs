//! RequestFunctionalTest.php and ResponseFunctionalTest.php at
//! symfony/http-foundation v8.1.11, through the PHP-FPM emulation
//! (`sapi`) in place of PHP's built-in server: each request is what the
//! test's `file_get_contents()` sends, its fixture script is written in
//! Rust from `Tests/Fixtures/{request,response}-functional/`, and what the
//! test reads is the body the SAPI sends. Names and assertions are
//! PHPUnit's; a data provider's sets run in one test.
//!
//! `testCookie` reads each fixture's `.expected` file from `upstream/`
//! (skipped with a note when it is not fetched); its `deleted_cookie` set
//! needs the Session classes (`SessionListener`, `NativeSessionStorage`),
//! which are not ported, so the other five sets run in
//! `cookie_fixtures_without_a_session` and the manifest row stays
//! pending.
//!
//! Upstream: symfony/http-foundation Tests/RequestFunctionalTest.php Tests/ResponseFunctionalTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::path::{Path, PathBuf};

use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    cookie::{Cookie, Options},
    file_bag::uploaded_file,
    header_bag::ResponseHeaderBag,
    json_response::JsonResponse,
    php::{Globals, set_globals, take_globals, with_output},
    request::Request,
    response::Response,
    sapi::{self, CgiRequest, Ini, Sapi, SetCookie},
};

fn param(name: &str, value: &str) -> (PhpString, PhpString) {
    (PhpString::from(name), PhpString::from(value))
}

/// What PHP's built-in server passes for the test's request.
fn request(method: &str, content_type: &str, body: &[u8]) -> CgiRequest {
    let length = body.len().to_string();
    CgiRequest {
        environment: Vec::new(),
        params: vec![
            param("DOCUMENT_ROOT", "/fixtures/request-functional"),
            param("REMOTE_ADDR", "127.0.0.1"),
            param("SERVER_PROTOCOL", "HTTP/1.1"),
            param("SERVER_NAME", "localhost"),
            param("SERVER_PORT", "8054"),
            param("REQUEST_URI", "/"),
            param("REQUEST_METHOD", method),
            param("SCRIPT_NAME", "/index.php"),
            param("SCRIPT_FILENAME", "/fixtures/request-functional/index.php"),
            param("HTTP_HOST", "localhost:8054"),
            param("HTTP_CONTENT_TYPE", content_type),
            param("HTTP_CONTENT_LENGTH", &length),
            param("CONTENT_TYPE", content_type),
            param("CONTENT_LENGTH", &length),
        ],
        stdin: body.to_vec(),
        time: 0.0,
    }
}

fn upload_dir() -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("http-foundation-functional-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `Fixtures/request-functional/index.php`: `createFromGlobals()`, then
/// a JsonResponse of the request's fields and files, sent.
fn request_functional(cgi: &CgiRequest) -> serde_json::Value {
    let ini = Ini {
        upload_tmp_dir: upload_dir(),
        ..Ini::default()
    };
    let started = sapi::start(cgi, &ini).unwrap();
    set_globals(started.globals);
    let request = Request::create_from_globals().unwrap();
    let mut files = PhpArray::new();
    for (key, value) in request.files().all().iter() {
        let file = uploaded_file(value).unwrap();
        let mut entry = PhpArray::new();
        entry.insert(
            "clientOriginalName",
            Value::String(file.get_client_original_name().clone()),
        );
        entry.insert(
            "clientMimeType",
            Value::String(file.get_client_mime_type().clone()),
        );
        entry.insert(
            "content",
            Value::String(PhpString::from(file.get_content().unwrap())),
        );
        files.insert(key.clone(), Value::Array(entry));
    }
    let mut data = PhpArray::new();
    data.insert("request", Value::Array(request.request().all().clone()));
    data.insert("files", Value::Array(files));
    let mut response =
        JsonResponse::new(Value::Array(data), 200, Vec::<(&str, &str)>::new(), false).unwrap();
    response.send(true).unwrap();
    let mut out = Sapi::new(&ini, false);
    for call in with_output(|o| o.calls.clone()) {
        out.apply(&call);
    }
    let body = out.end().body;
    take_globals();
    sapi::finish(&started.uploaded);
    serde_json::from_slice(&body).unwrap()
}

fn provideMethodsRequiringExplicitBodyParsing() -> Vec<&'static str> {
    // PHP's built-in server doesn't support QUERY
    vec!["PUT", "DELETE", "PATCH"]
}

#[test]
fn testFormUrlEncodedBodyParsing() {
    for method in provideMethodsRequiringExplicitBodyParsing() {
        let response = request_functional(&request(
            method,
            "application/x-www-form-urlencoded",
            b"foo=bar",
        ));

        assert_eq!(
            serde_json::json!({"foo": "bar"}),
            response["request"],
            "{method}"
        );
    }
}

#[test]
fn testMultipartFormDataBodyParsing() {
    for method in provideMethodsRequiringExplicitBodyParsing() {
        let body = b"--boundary\r\n\
                     Content-Disposition: form-data; name=foo\r\n\
                     \r\n\
                     bar\r\n\
                     --boundary\r\n\
                     Content-Disposition: form-data; name=baz; filename=baz.txt\r\n\
                     Content-Type: text/plain\r\n\
                     \r\n\
                     qux\r\n\
                     --boundary--";
        let data = request_functional(&request(
            method,
            "multipart/form-data; boundary=boundary",
            body,
        ));

        assert_eq!(
            serde_json::json!({"foo": "bar"}),
            data["request"],
            "{method}"
        );
        assert_eq!(
            serde_json::json!({"baz": {
                "clientOriginalName": "baz.txt",
                "clientMimeType": "text/plain",
                "content": "qux",
            }}),
            data["files"],
            "{method}"
        );
    }
}

/// `print_r()` of a list of strings.
fn print_r(list: &[PhpString]) -> Vec<u8> {
    let mut out = b"Array\n(\n".to_vec();
    for (i, line) in list.iter().enumerate() {
        out.extend_from_slice(format!("    [{i}] => ").as_bytes());
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    out.extend_from_slice(b")\n");
    out
}

/// One response-functional fixture: `common.inc`, the fixture's body, then
/// the shutdown function; the body PHP sends.
fn response_fixture(fixture: &str) -> Vec<u8> {
    set_globals(Globals::default());
    let ini = Ini {
        x_powered_by: Some(PhpString::from("PHP/8.4.27")),
        ..Ini::default()
    };
    let mut sapi = Sapi::new(&ini, false);
    // common.inc
    sapi.header_remove(Some(b"X-Powered-By"));
    sapi.header(b"Content-Type: text/plain; charset=utf-8", true, 0);
    sapi.ob_start();
    let mut r = Response::new(
        Some(""),
        200,
        ResponseHeaderBag::new(Vec::<(&str, &str)>::new()).unwrap(),
    )
    .unwrap();
    r.headers_mut()
        .set(b"Date", "Sat, 12 Nov 1955 20:04:00 GMT", true)
        .unwrap();

    let str1 = "=,; \t\r\n\x0b\x0c";
    let str2 = "?*():@&+$/%#[]";
    let cookie = |name: &str,
                  value: &str,
                  path: &str,
                  http_only: bool,
                  raw: bool,
                  same_site: Option<&str>| {
        Cookie::new(
            name,
            Options {
                value: Some(PhpString::from(value)),
                expire: 0.into(),
                path: Some(PhpString::from(path)),
                domain: None,
                secure: Some(false),
                http_only,
                raw,
                same_site: same_site.map(PhpString::from),
                partitioned: false,
            },
        )
    };
    let mut after_send: Vec<(SetCookie, bool)> = Vec::new();
    match fixture {
        "cookie_urlencode" => {
            r.headers_mut()
                .set_cookie(cookie(str1, str1, "", false, false, None).unwrap());
            r.headers_mut()
                .set_cookie(cookie(str2, str2, "", false, false, None).unwrap());
            r.send_headers(None).unwrap();
            after_send.push((
                SetCookie {
                    name: PhpString::from(str2),
                    value: PhpString::from(str2),
                    path: PhpString::from("/"),
                    ..SetCookie::default()
                },
                false,
            ));
        }
        "cookie_raw_urlencode" => {
            r.headers_mut()
                .set_cookie(cookie(str2, str2, "/", false, true, None).unwrap());
            r.send_headers(None).unwrap();
            after_send.push((
                SetCookie {
                    name: PhpString::from(str2),
                    value: PhpString::from(str2),
                    path: PhpString::from("/"),
                    ..SetCookie::default()
                },
                true,
            ));
        }
        "cookie_samesite_lax" => {
            r.headers_mut().set_cookie(
                cookie(
                    "CookieSamesiteLaxTest",
                    "LaxValue",
                    "/",
                    true,
                    false,
                    Some("lax"),
                )
                .unwrap(),
            );
            r.send_headers(None).unwrap();
        }
        "cookie_samesite_strict" => {
            r.headers_mut().set_cookie(
                cookie(
                    "CookieSamesiteStrictTest",
                    "StrictValue",
                    "/",
                    true,
                    false,
                    Some("strict"),
                )
                .unwrap(),
            );
            r.send_headers(None).unwrap();
        }
        "invalid_cookie_name" => {
            match Cookie::new(
                "Hello + world",
                Options {
                    value: Some(PhpString::from("hodor")),
                    path: None,
                    secure: None,
                    http_only: false,
                    raw: true,
                    ..Options::default()
                },
            ) {
                Ok(c) => r.headers_mut().set_cookie(c),
                Err(e) => sapi.echo(e.message().as_bytes()),
            }
        }
        other => panic!("no fixture {other}"),
    }
    for call in with_output(|o| o.calls.clone()) {
        sapi.apply(&call);
    }
    for (cookie, raw) in after_send {
        sapi.setcookie(&cookie, raw).unwrap();
    }
    // register_shutdown_function()
    sapi.echo(b"\n");
    let list = sapi.headers_list();
    sapi.echo(&print_r(&list));
    sapi.echo(b"shutdown\n");
    take_globals();
    sapi.end().body
}

/// `assertStringMatchesFormatFile()` for formats without placeholders
/// other than `%%`.
fn matches_format(format: &[u8], actual: &[u8]) -> bool {
    let mut expected = Vec::new();
    let mut at = 0;
    while at < format.len() {
        if format[at] == b'%' && format.get(at + 1) == Some(&b'%') {
            expected.push(b'%');
            at += 2;
        } else {
            // PHPUnit's placeholders; any other `%` is itself.
            assert!(
                !(format[at] == b'%'
                    && format
                        .get(at + 1)
                        .is_some_and(|b| b"esSaAwidxfc".contains(b))),
                "a format placeholder this port does not read"
            );
            expected.push(format[at]);
            at += 1;
        }
    }
    // PHPUnit compares with the trailing newlines of both trimmed.
    let trim = |s: &[u8]| {
        s.iter()
            .rposition(|b| !matches!(b, b'\n' | b'\r'))
            .map_or(0, |e| e + 1)
    };
    expected[..trim(&expected)] == actual[..trim(actual)]
}

/// ResponseFunctionalTest::testCookie for every fixture but
/// `deleted_cookie` (see the module documentation).
#[test]
fn cookie_fixtures_without_a_session() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../upstream/symfony/http-foundation/Tests/Fixtures/response-functional");
    if !dir.is_dir() {
        eprintln!("skipped: upstream/ is not fetched (scripts/fetch-upstream.sh)");
        return;
    }
    for fixture in [
        "cookie_raw_urlencode",
        "cookie_samesite_lax",
        "cookie_samesite_strict",
        "cookie_urlencode",
        "invalid_cookie_name",
    ] {
        let result = response_fixture(fixture);
        // preg_replace_callback('/expires=[^;]++/', ...): no fixture here
        // sets an expiry.
        assert!(!result.windows(8).any(|w| w == b"expires="));
        let expected = std::fs::read(dir.join(format!("{fixture}.expected"))).unwrap();
        assert!(
            matches_format(&expected, &result),
            "{fixture}:\n{}",
            String::from_utf8_lossy(&result)
        );
    }
}
