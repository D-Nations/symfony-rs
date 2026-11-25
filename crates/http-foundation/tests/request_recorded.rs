//! `Request` against PHP 8.4.27 running the pinned symfony/http-foundation
//! (v8.1.11), recorded by `oracle/recorders/http_foundation_request.php`
//! (`scripts/record.sh http-foundation-request`): 2,500 generated
//! `$_SERVER` arrays through `new Request()` and 1,500 generated URIs
//! through `Request::create()`, each under a generated configuration and
//! answering the recorder's 42 calls in its order; then `getFormat()`,
//! `normalizeQueryString()`, JSON bodies, and `getPort()` on 23
//! `SERVER_PORT` values of every type. A value is compared with
//! `===`; an exception by class and message.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::Error,
    php::{Globals, set_globals, with_globals},
    request::{Content, Create, Parameters, Request},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/request.json");

const RELATIVE: [&str; 7] = [
    "/foo/bar",
    "/foo/",
    "/",
    "/a:b/c",
    "rel",
    "/foo/bar/baz/qux",
    "",
];
const LOCALES: [Option<&[&str]>; 5] = [
    None,
    Some(&["en"]),
    Some(&["fr_FR", "en_US"]),
    Some(&["zh_Hans", "de"]),
    Some(&["pt_BR", "pt"]),
];

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(json.as_str().ok_or("expected base64")?)
}

fn array(json: &Json) -> Result<PhpArray, TestError> {
    match vectors::decode(json)? {
        Value::Array(a) => Ok(a),
        other => Err(format!("expected an array, got {other:?}").into()),
    }
}

fn strings(json: &Json) -> Result<Vec<String>, TestError> {
    json.as_array()
        .ok_or("expected a list")?
        .iter()
        .map(|s| Ok(s.as_str().ok_or("expected a string")?.to_owned()))
        .collect()
}

/// `[proxies, header set, hosts, parameter override, allowed, remote]`.
fn configure(config: &Json) -> Result<(), TestError> {
    Request::reset_statics();
    let mut globals = Globals::default();
    if let Some(remote) = config[5].as_str() {
        globals.server.insert("REMOTE_ADDR", Value::string(remote));
    }
    set_globals(globals);
    Request::set_trusted_proxies(strings(&config[0])?, config[1].as_i64().ok_or("set")?)?;
    Request::set_trusted_hosts(strings(&config[2])?)?;
    if config[3].as_bool().ok_or("override")? {
        Request::enable_http_method_parameter_override();
    }
    let allowed = match &config[4] {
        Json::Null => None,
        list => Some(strings(list)?),
    };
    Request::set_allowed_http_method_override(allowed)?;
    Ok(())
}

/// A call's outcome, as both sides record it.
#[derive(Debug, PartialEq)]
enum Outcome {
    Ok(Value),
    Throws(String, Vec<u8>),
}

impl Outcome {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ok(a), Self::Ok(b)) => a.identical(b),
            (a, b) => a == b,
        }
    }
}

fn recorded(json: &Json) -> Result<Outcome, TestError> {
    Ok(if json[0] == "ok" {
        Outcome::Ok(vectors::decode(&json[1])?)
    } else {
        Outcome::Throws(
            json[1].as_str().ok_or("class")?.to_owned(),
            bytes(&json[2])?,
        )
    })
}

fn got<T: Into<Value>>(result: Result<T, Error>) -> Outcome {
    match result {
        Ok(v) => Outcome::Ok(v.into()),
        Err(e) => Outcome::Throws(e.class().name().to_owned(), e.message().as_bytes().to_vec()),
    }
}

fn string(s: PhpString) -> Value {
    Value::String(s)
}

fn optional(s: Option<PhpString>) -> Value {
    s.map_or(Value::Null, Value::String)
}

fn list(items: Vec<PhpString>) -> Value {
    Value::Array(PhpArray::list(items.into_iter().map(Value::String)))
}

/// The recorder's `calls()`, in its order.
fn calls(r: &mut Request, i: usize) -> Vec<Outcome> {
    let relative = RELATIVE[i % RELATIVE.len()];
    let locales = LOCALES[i % LOCALES.len()];
    vec![
        got(r.get_method().map(string)),
        got(r.get_real_method().map(string)),
        got(r.get_request_uri().map(string)),
        got(r.get_path_info().map(string)),
        got(r.get_base_url().map(string)),
        got(r.get_base_path().map(string)),
        got(r.get_script_name().map(string)),
        got(r.get_query_string().map(optional)),
        got(r.is_from_trusted_proxy().map(Value::Bool)),
        got(r.get_scheme().map(Value::string)),
        got(r.is_secure().map(Value::Bool)),
        got(r.get_port()),
        got(r.get_host().map(string)),
        got(r.get_http_host().map(string)),
        got(r.get_host().map(string)),
        got(r.get_scheme_and_http_host().map(string)),
        got(r.get_uri().map(string)),
        got(r.get_uri_for_path(b"/x").map(string)),
        got(r.get_relative_uri_for_path(relative.as_bytes()).map(string)),
        got(r
            .get_client_ips()
            .map(|ips| Value::Array(PhpArray::list(ips.into_iter().map(optional))))),
        got(r.get_client_ip().map(optional)),
        got(r.get_protocol_version().map(optional)),
        Outcome::Ok(optional(r.get_user())),
        Outcome::Ok(optional(r.get_password())),
        Outcome::Ok(optional(r.get_user_info())),
        got(r.get_languages().map(Value::Array)),
        got(r.get_preferred_language(locales).map(optional)),
        Outcome::Ok(list(r.get_charsets())),
        Outcome::Ok(list(r.get_encodings())),
        got(r.get_acceptable_content_types().map(list)),
        got(r.get_preferred_format(Some("html")).map(optional)),
        got(r.get_request_format(Some("html")).map(optional)),
        Outcome::Ok(optional(r.get_content_type_format())),
        Outcome::Ok(list(r.get_etags())),
        Outcome::Ok(Value::Bool(r.is_no_cache())),
        Outcome::Ok(Value::Bool(r.is_xml_http_request())),
        got(r.prefer_safe_content().map(Value::Bool)),
        got(r.is_method_safe().map(Value::Bool)),
        got(r.is_method_idempotent().map(Value::Bool)),
        got(r.is_method_cacheable().map(Value::Bool)),
        got(r.is_method("post").map(Value::Bool)),
        got(r.to_php_string().map(string)),
    ]
}

fn compare(
    label: &str,
    want: &[Json],
    have: &[Outcome],
    failures: &mut Vec<String>,
) -> Result<(), TestError> {
    if want.len() != have.len() {
        failures.push(format!(
            "{label}: {} calls recorded, {} made",
            want.len(),
            have.len()
        ));
        return Ok(());
    }
    for (n, (w, h)) in want.iter().zip(have).enumerate() {
        let w = recorded(w)?;
        if !w.same(h) {
            failures.push(format!("{label} call {n}: PHP {w:?}, Rust {h:?}"));
        }
    }
    Ok(())
}

#[test]
fn matches_recorded_requests() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = data["cases"].as_array().ok_or("cases")?;
    assert!(cases.len() >= 4000, "{} cases", cases.len());
    let mut failures = Vec::new();
    let (mut server_cases, mut create_cases) = (0, 0);
    for (i, case) in cases.iter().enumerate() {
        configure(&case[0])?;
        let input = &case[2];
        let results = &case[3];
        let index = if case[1] == "server" {
            server_cases
        } else {
            create_cases
        };
        if case[1] == "server" {
            server_cases += 1;
            let mut request = Request::new(Parameters {
                server: array(&input[0])?,
                request: array(&input[1])?,
                query: array(&input[2])?,
                ..Parameters::default()
            })?;
            let have = calls(&mut request, index);
            compare(
                &format!("case {i} (server)"),
                results.as_array().ok_or("results")?,
                &have,
                &mut failures,
            )?;
        } else {
            create_cases += 1;
            let uri = bytes(&input[0])?;
            let created = Request::create_with(
                &uri,
                Create {
                    method: Some(PhpString::from(input[1].as_str().ok_or("method")?)),
                    parameters: array(&input[2])?,
                    server: array(&input[3])?,
                    ..Create::default()
                },
            );
            let label = format!("case {i} (create {:?})", String::from_utf8_lossy(&uri));
            match (created, results[0].as_str()) {
                (Err(e), Some("throws")) => {
                    let want = recorded(results)?;
                    let have = got::<Value>(Err(e));
                    if want != have {
                        failures.push(format!("{label}: PHP {want:?}, Rust {have:?}"));
                    }
                }
                (Ok(mut request), Some("throws")) => {
                    let _ = &mut request;
                    failures.push(format!("{label}: PHP throws {}, Rust creates", results[1]));
                }
                (Err(e), _) => failures.push(format!("{label}: Rust throws {e}")),
                (Ok(mut request), _) => {
                    let have = calls(&mut request, index);
                    compare(
                        &label,
                        results.as_array().ok_or("results")?,
                        &have,
                        &mut failures,
                    )?;
                }
            }
        }
    }
    Request::reset_statics();

    let request = Request::default();
    for case in data["formats"].as_array().ok_or("formats")? {
        let mime = vectors::decode(&case[0])?;
        let mime = mime.as_string().map(PhpString::as_bytes);
        for (n, fallback) in [(1, false), (2, true)] {
            let want = recorded(&case[n])?;
            let have = Outcome::Ok(optional(request.get_format(mime, fallback)));
            if !want.same(&have) {
                failures.push(format!(
                    "getFormat({mime:?}, {fallback}): PHP {want:?}, Rust {have:?}"
                ));
            }
        }
    }
    for case in data["normalized"].as_array().ok_or("normalized")? {
        let qs = bytes(&case[0])?;
        let want = recorded(&case[1])?;
        let have = got(Request::normalize_query_string(Some(&qs)).map(string));
        if !want.same(&have) {
            failures.push(format!(
                "normalizeQueryString({:?}): PHP {want:?}, Rust {have:?}",
                String::from_utf8_lossy(&qs)
            ));
        }
    }
    for case in data["payloads"].as_array().ok_or("payloads")? {
        let body = bytes(&case[0])?;
        let new = || {
            Request::new(Parameters {
                content: Content::String(PhpString::from(body.clone())),
                ..Parameters::default()
            })
        };
        let to_array = got(new()?.to_array().map(Value::Array));
        let payload = got(new()?
            .get_payload()
            .map(|bag| Value::Array(bag.all().clone())));
        let previous = match new()?.to_array() {
            Ok(_) => Value::Null,
            Err(e) => Value::Array(PhpArray::list([
                Value::Int(e.code()),
                e.previous()
                    .map_or(Value::Null, |p| Value::string(p.class().name())),
                e.previous()
                    .map_or(Value::Null, |p| Value::String(p.message().clone())),
            ])),
        };
        for (n, have) in [(1, to_array), (2, payload), (3, Outcome::Ok(previous))] {
            let want = recorded(&case[n])?;
            if !want.same(&have) {
                failures.push(format!(
                    "body {:?} part {n}: PHP {want:?}, Rust {have:?}",
                    String::from_utf8_lossy(&body)
                ));
            }
        }
    }
    let ports = data["ports"].as_array().ok_or("ports")?;
    assert_eq!(23, ports.len());
    for case in ports {
        let port = vectors::decode(&case[0])?;
        let mut server = PhpArray::new();
        server.insert("SERVER_PORT", port.clone());
        let mut request = Request::new(Parameters {
            server,
            ..Parameters::default()
        })?;
        let want = recorded(&case[1])?;
        let have = got(request.get_port());
        if !want.same(&have) {
            failures.push(format!(
                "getPort() of SERVER_PORT {port:?}: PHP {want:?}, Rust {have:?}"
            ));
        }
    }
    with_globals(|g| *g = Globals::default());

    assert!(
        failures.is_empty(),
        "{} answers differ:\n{}",
        failures.len(),
        failures[..failures.len().min(30)].join("\n")
    );
    Ok(())
}
