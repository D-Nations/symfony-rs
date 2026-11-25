//! Replays what PHP 8.4.27 made of query strings, form bodies, Cookie
//! headers and form content types (`tests/data/{query,cookie,content_type}
//! _vectors.json`, recorded by `scripts/record.sh request-vars`).
//!
//! Upstream: none

use base64::Engine as _;
use serde_json::Value;
use symfony_http_foundation::{
    cookies::Cookies,
    form_fields,
    query::{Query, QueryValue},
};

/// A parsed value: text, or an array of `(key, value)` in order. Bytes
/// that are not UTF-8 are replaced, as the Rust API stores them.
#[derive(Debug, PartialEq)]
enum Tree {
    Text(String),
    Array(Vec<(String, Tree)>),
}

fn bytes(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|error| error.to_string())
}

fn lossy(text: &str) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&bytes(text)?).into_owned())
}

/// PHP's answer: strings in base64, arrays as `[key, value]` pairs with
/// int keys as numbers and string keys in base64.
fn from_php(value: &Value) -> Result<Tree, String> {
    match value {
        Value::String(text) => Ok(Tree::Text(lossy(text)?)),
        Value::Array(pairs) => pairs
            .iter()
            .map(|pair| match pair.as_array().map(Vec::as_slice) {
                Some([Value::Number(key), value]) => Ok((key.to_string(), from_php(value)?)),
                Some([Value::String(key), value]) => Ok((lossy(key)?, from_php(value)?)),
                _ => Err(format!("not a [key, value] pair: {pair}")),
            })
            .collect::<Result<_, _>>()
            .map(Tree::Array),
        other => Err(format!("unexpected value {other}")),
    }
}

fn from_rust(entries: &[(String, QueryValue)]) -> Tree {
    Tree::Array(
        entries
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    QueryValue::Str(text) => Tree::Text(text.clone()),
                    QueryValue::Map(children) => from_rust(children),
                };
                (key.clone(), value)
            })
            .collect(),
    )
}

/// The nesting-depth vectors are deeper than `serde_json`'s default limit.
fn vectors(json: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    use serde::Deserialize as _;
    let mut deserializer = serde_json::Deserializer::from_str(json);
    deserializer.disable_recursion_limit();
    Ok(Vec::<Value>::deserialize(&mut deserializer)?)
}

fn check(failures: &mut Vec<String>, label: String, expected: &Value, got: Tree) {
    match from_php(expected) {
        Ok(expected) if expected == got => {}
        Ok(expected) => failures.push(format!("{label}:\n  php  {expected:?}\n  rust {got:?}")),
        Err(error) => failures.push(format!("{label}: {error}")),
    }
}

fn report(failures: &[String], total: usize) {
    assert!(
        failures.is_empty(),
        "{} of {total} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn query_strings_and_form_bodies_read_as_php_does() -> Result<(), Box<dyn std::error::Error>> {
    let vectors = vectors(include_str!("data/query_vectors.json"))?;
    assert!(vectors.len() > 150);
    let mut failures = Vec::new();
    for vector in &vectors {
        let name = vector["name"].as_str().unwrap_or("?");
        let raw = bytes(vector["raw"].as_str().unwrap_or_default())?;
        if !vector["get"].is_null() {
            let text = String::from_utf8(raw.clone())?;
            check(
                &mut failures,
                format!("{name} GET"),
                &vector["get"],
                from_rust(Query::parse(&text).all()),
            );
        }
        check(
            &mut failures,
            format!("{name} POST"),
            &vector["post"],
            from_rust(Query::parse_body(&raw).all()),
        );
    }
    report(&failures, vectors.len());
    Ok(())
}

#[test]
fn cookie_headers_read_as_php_does() -> Result<(), Box<dyn std::error::Error>> {
    let vectors = vectors(include_str!("data/cookie_vectors.json"))?;
    assert!(vectors.len() > 50);
    let mut failures = Vec::new();
    for vector in &vectors {
        let name = vector["name"].as_str().unwrap_or("?");
        let header = String::from_utf8(bytes(vector["raw"].as_str().unwrap_or_default())?)?;
        let cookies = Cookies::parse([header.as_str()]);
        check(
            &mut failures,
            name.to_owned(),
            &vector["cookie"],
            from_rust(cookies.all()),
        );
    }
    report(&failures, vectors.len());
    Ok(())
}

#[test]
fn form_content_types_select_the_parser_as_php_does() -> Result<(), Box<dyn std::error::Error>> {
    let vectors = vectors(include_str!("data/content_type_vectors.json"))?;
    let mut failures = Vec::new();
    for vector in &vectors {
        let name = vector["name"].as_str().unwrap_or("?");
        let content_type = vector["content_type"].as_str().unwrap_or_default();
        let fields = form_fields("POST", content_type.as_bytes(), b"a=1&b[]=2");
        check(
            &mut failures,
            name.to_owned(),
            &vector["post"],
            from_rust(fields.all()),
        );
    }
    report(&failures, vectors.len());
    Ok(())
}
