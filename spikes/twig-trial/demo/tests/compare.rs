//! Renders blog/_post.html.twig for every case in tests/data/cases.json
//! and compares the output byte for byte with what PHP Twig 3.30 printed
//! for the same case (tests/data/php.json, scripts/record.sh twig-trial).
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::fs;

use php_value::{
    PhpArray,
    datetime::{DateTimeImmutable, Fields, TimeZone},
};
use serde_json::Value as Json;
use twig_trial_demo::{
    model::{App, Collection, InputBag, Post, Request, Tag, User},
    render,
};
use twig_trial_runtime::{Obj, Value};

fn load(path: &str) -> Json {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn str_of<'a>(json: &'a Json, key: &str) -> &'a str {
    json[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string {key}"))
}

/// "Y-m-d H:i:s" in UTC, as the fixtures write it.
fn date(text: &str) -> DateTimeImmutable {
    let n: Vec<i64> = text
        .split(['-', ' ', ':'])
        .map(|p| p.parse().expect("date field"))
        .collect();
    let fields = Fields {
        year: n[0],
        month: n[1],
        day: n[2],
        hour: n[3],
        minute: n[4],
        second: n[5],
        microsecond: 0,
    };
    DateTimeImmutable::from_fields(fields, TimeZone::Utc)
}

fn context(case: &Json) -> PhpArray {
    let p = &case["post"];
    let tags = p["tags"]
        .as_array()
        .expect("tags")
        .iter()
        .map(|t| Obj::value(Tag(t.as_str().expect("tag").to_owned())))
        .collect();
    let post = Post {
        title: str_of(p, "title").to_owned(),
        slug: str_of(p, "slug").to_owned(),
        summary: str_of(p, "summary").to_owned(),
        published_at: date(str_of(p, "publishedAt")),
        author: Obj::value(User(str_of(p, "author").to_owned())),
        tags: Obj::value(Collection(tags)),
    };
    let mut query = PhpArray::new();
    for (k, v) in case["query"].as_object().expect("query") {
        query.insert(k.as_str(), Value::string(v.as_str().expect("query value")));
    }
    let request = Obj::value(Request {
        query: Obj::value(InputBag(query)),
    });
    let mut context = PhpArray::new();
    context.insert("post", Obj::value(post));
    context.insert("app", Obj::value(App { request }));
    context
}

#[test]
fn post_template_matches_php_twig() {
    let input = load("tests/data/cases.json");
    let php = load("tests/data/php.json");
    let cases = input["cases"].as_array().expect("cases");
    let expected = php["cases"].as_array().expect("php cases");
    assert_eq!(cases.len(), expected.len(), "re-record tests/data/php.json");
    let mut failures = Vec::new();
    for (case, want) in cases.iter().zip(expected) {
        assert_eq!(case["name"], want["name"]);
        let got =
            render("blog/_post.html.twig", &mut context(case)).unwrap_or_else(|e| panic!("{e}"));
        if got != str_of(want, "output") {
            failures.push(format!(
                "--- {}\nPHP:\n{}\nRust:\n{got}",
                case["name"],
                str_of(want, "output")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn unknown_template_is_an_error() {
    let err = render("blog/missing.html.twig", &mut PhpArray::new()).expect_err("missing template");
    assert_eq!(
        err.message,
        "Unable to find template \"blog/missing.html.twig\"."
    );
}

/// JSON as `json_decode($json, true)` gives it.
fn php(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n
            .as_i64()
            .map_or_else(|| Value::Float(n.as_f64().unwrap_or(0.0)), Value::Int),
        Json::String(s) => Value::string(s.as_str()),
        Json::Array(items) => Value::Array(PhpArray::list(items.iter().map(php))),
        Json::Object(map) => {
            let mut array = PhpArray::new();
            for (k, v) in map {
                array.insert(k.as_str(), php(v));
            }
            Value::Array(array)
        }
    }
}

#[test]
fn language_snippets_match_php_twig() {
    let input = load("tests/data/snippets.json");
    let php_out = load("tests/data/php.json");
    let Value::Array(context) = php(&input["context"]) else {
        panic!("context must be a mapping")
    };
    let expected = php_out["snippets"].as_array().expect("php snippets");
    let mut files: Vec<_> = fs::read_dir("tests/snippets")
        .expect("tests/snippets")
        .map(|e| e.expect("entry").file_name())
        .collect();
    files.sort();
    assert_eq!(files.len(), expected.len(), "re-record tests/data/php.json");
    let mut failures = Vec::new();
    for want in expected {
        let name = str_of(want, "name");
        let got = render(name, &mut context.clone()).unwrap_or_else(|e| panic!("{e}"));
        if got != str_of(want, "output") {
            failures.push(format!(
                "--- {name}\nPHP:  {:?}\nRust: {got:?}",
                str_of(want, "output")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} snippets differ:\n{}",
        failures.len(),
        expected.len(),
        failures.join("\n")
    );
}
