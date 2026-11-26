//! The `RequestMatcher` classes and `ChainRequestMatcher` against PHP 8.4.27
//! running the pinned symfony/http-foundation (v8.1.11) and
//! symfony/expression-language, recorded by
//! `oracle/recorders/http_foundation_request_matchers.php`
//! (`scripts/record.sh http-foundation-request-matchers`): 1,500 generated
//! scripts.
//!
//! Each script builds a request from pools (URI, method, host, port as an
//! int or a string, HTTPS, client address, headers, body, attributes,
//! locale). It then asks six matchers built from pools whether they
//! match: every class, lists as arrays or comma-separated text, patterns
//! that do not compile, expressions over the request object, and chains.
//! Compared: each answer, or the exception's class and message.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, object::Closure, vectors};
use serde_json::Value as Json;
use symfony_expression_language::ExpressionLanguage;
use symfony_http_foundation::{
    chain_request_matcher::ChainRequestMatcher,
    exception::Error,
    request::{Content, Create, Request},
    request_matcher::{
        attributes_request_matcher::AttributesRequestMatcher,
        expression_request_matcher::{ExpressionRequestMatcher, ExpressionSource},
        header_request_matcher::HeaderRequestMatcher,
        host_request_matcher::HostRequestMatcher,
        ips_request_matcher::IpsRequestMatcher,
        is_json_request_matcher::IsJsonRequestMatcher,
        method_request_matcher::MethodRequestMatcher,
        path_request_matcher::PathRequestMatcher,
        port_request_matcher::PortRequestMatcher,
        query_parameter_request_matcher::QueryParameterRequestMatcher,
        scheme_request_matcher::SchemeRequestMatcher,
    },
    request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/request_matchers.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(text(json)?)
}

fn items(json: &Json) -> Result<&Vec<Json>, TestError> {
    json.as_array().ok_or_else(|| "expected a list".into())
}

fn list(json: &Json) -> Result<Vec<Vec<u8>>, TestError> {
    items(json)?.iter().map(bytes).collect()
}

/// The recorder's `$build($spec)`.
fn build(
    spec: &Json,
    language: &Arc<ExpressionLanguage>,
) -> Result<Arc<dyn RequestMatcherInterface>, TestError> {
    Ok(match text(&spec[0])? {
        "attributes" => {
            // array_combine(): a key given twice keeps its first position
            // and its last pattern.
            let mut regexps: Vec<(String, Vec<u8>)> = Vec::new();
            for pair in items(&spec[1])? {
                let (key, regexp) = (text(&pair[0])?.to_owned(), bytes(&pair[1])?);
                match regexps.iter_mut().find(|(k, _)| *k == key) {
                    Some(entry) => entry.1 = regexp,
                    None => regexps.push((key, regexp)),
                }
            }
            Arc::new(AttributesRequestMatcher::new(regexps))
        }
        "header" => Arc::new(HeaderRequestMatcher::new(list(&spec[1])?)),
        "host" => Arc::new(HostRequestMatcher::new(bytes(&spec[1])?)),
        "ips" => Arc::new(IpsRequestMatcher::new(list(&spec[1])?)),
        "is_json" => Arc::new(IsJsonRequestMatcher),
        "method" => Arc::new(MethodRequestMatcher::new(list(&spec[1])?)),
        "path" => Arc::new(PathRequestMatcher::new(bytes(&spec[1])?)),
        "port" => Arc::new(PortRequestMatcher::new(spec[1].as_i64().ok_or("port")?)),
        "query" => Arc::new(QueryParameterRequestMatcher::new(list(&spec[1])?)),
        "scheme" => Arc::new(SchemeRequestMatcher::new(list(&spec[1])?)),
        "expression" => Arc::new(ExpressionRequestMatcher::new(
            Arc::clone(language),
            ExpressionSource::Source(PhpString::from(bytes(&spec[1])?)),
        )),
        "chain" => Arc::new(ChainRequestMatcher::new(
            items(&spec[1])?
                .iter()
                .map(|s| build(s, language))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        other => return Err(format!("unknown matcher {other}").into()),
    })
}

fn scalar(json: &Json) -> Value {
    match json {
        Json::Number(n) => n.as_i64().map_or(Value::Null, Value::Int),
        Json::String(s) => Value::string(s.as_str()),
        _ => Value::Null,
    }
}

/// The recorder's request, built afresh for each matcher.
fn request(spec: &Json) -> Result<Request, TestError> {
    let mut server = PhpArray::new();
    for pair in items(&spec["server"])? {
        server.insert(text(&pair[0])?, scalar(&pair[1]));
    }
    let content = match &spec["content"] {
        Json::Null => Content::Null,
        other => Content::String(PhpString::from(bytes(other)?)),
    };
    let mut request = Request::create_with(
        bytes(&spec["uri"])?,
        Create {
            method: Some(PhpString::from(text(&spec["method"])?)),
            server,
            content,
            ..Create::default()
        },
    )?;
    for pair in items(&spec["attributes"])? {
        let value = match &pair[1] {
            Json::String(s) if s == "closure" => Closure::object(|_| Value::Null),
            other => scalar(other),
        };
        request.attributes_mut().set(text(&pair[0])?, value);
    }
    if let Some(locale) = spec["locale"].as_str() {
        request.set_locale(locale);
    }
    Ok(request)
}

fn thrown(error: &Throwable) -> (String, Vec<u8>) {
    if let Some(error) = error.downcast_ref::<Error>() {
        return (
            error.class().name().to_owned(),
            error.message().as_bytes().to_vec(),
        );
    }
    if let Some(error) = error.downcast_ref::<symfony_expression_language::Error>() {
        return (
            error.class().name().to_owned(),
            error.message().as_bytes().to_vec(),
        );
    }
    ("unknown".to_owned(), error.to_string().into_bytes())
}

#[test]
fn request_matchers_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = items(&data["scripts"])?;
    assert_eq!(1500, scripts.len());
    let language = Arc::new(ExpressionLanguage::new());
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        for (i, (spec, recorded)) in items(&script["matchers"])?
            .iter()
            .zip(items(&script["results"])?)
            .enumerate()
        {
            answers += 1;
            let matcher = build(spec, &language)?;
            let mut request = request(&script["request"])?;
            let got = matcher.matches(&mut request);
            let same = match &got {
                Ok(b) => {
                    recorded[0] != "throws"
                        && vectors::decode(recorded)?.identical(&Value::Bool(*b))
                }
                Err(error) => {
                    let (class, message) = thrown(error);
                    recorded[0] == "throws"
                        && recorded[1] == class.as_str()
                        && bytes(&recorded[2])? == message
                }
            };
            if !same {
                let shown = got.map_err(|e| {
                    let (class, message) = thrown(&e);
                    format!("{class}: {}", String::from_utf8_lossy(&message))
                });
                let expected = if recorded[0] == "throws" {
                    format!(
                        "{}: {}",
                        recorded[1],
                        String::from_utf8_lossy(&bytes(&recorded[2])?)
                    )
                } else {
                    recorded.to_string()
                };
                failures.push(format!(
                    "script {n} matcher {i} {spec}: expected {expected}, got {shown:?}"
                ));
            }
        }
    }
    let total = failures.len();
    failures.truncate(15);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    eprintln!("{answers} answers");
    Ok(())
}
