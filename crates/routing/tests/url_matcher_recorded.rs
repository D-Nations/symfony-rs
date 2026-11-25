//! `UrlMatcher` and `RedirectableUrlMatcher` against PHP 8.4.27 running
//! the pinned symfony/routing, recorded by
//! `oracle/recorders/routing_url_matcher.php` (`scripts/record.sh
//! routing-url-matcher`): 2,500 generated collections and 10,000 requests
//! in generated contexts, each matched by both matchers. A match is
//! compared as the whole attributes array (keys, order and value types);
//! an exception by class, message and allowed methods.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_routing::{
    exception::{Detail, Error},
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_matcher::UrlMatcher,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/url_matcher.json");

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(json.as_str().ok_or("expected base64")?)
}

fn array(json: &Json) -> Result<PhpArray, TestError> {
    match vectors::decode(json)? {
        Value::Array(a) => Ok(a),
        other => Err(format!("expected an array, got {other:?}").into()),
    }
}

fn strings(json: &Json) -> Result<Vec<PhpString>, TestError> {
    json.as_array()
        .ok_or("expected a list")?
        .iter()
        .map(|s| bytes(s).map(PhpString::from))
        .collect()
}

fn collection(entries: &Json) -> Result<RouteCollection, TestError> {
    let mut routes = RouteCollection::new();
    for entry in entries.as_array().ok_or("entries")? {
        let route = Route::with(
            bytes(&entry[1])?,
            Arguments {
                defaults: array(&entry[2])?,
                requirements: array(&entry[3])?,
                options: array(&entry[4])?,
                host: PhpString::from(bytes(&entry[5])?),
                schemes: strings(&entry[6])?,
                methods: strings(&entry[7])?,
                ..Arguments::default()
            },
        )?;
        routes.add(
            PhpString::from(bytes(&entry[0])?),
            route,
            entry[8].as_i64().ok_or("priority")?,
        );
    }
    Ok(routes)
}

/// The recorder's subclass: `['_redirect' => [$path, $route, $scheme]]`.
struct Redirecting;

impl RedirectableUrlMatcherInterface for Redirecting {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, Error> {
        let target = PhpArray::list([
            Value::string(path.to_vec()),
            Value::String(route.clone()),
            scheme.map_or(Value::Null, |s| Value::string(s.to_vec())),
        ]);
        Ok([("_redirect", Value::Array(target))].into_iter().collect())
    }
}

/// What a match came to, in a form both sides produce.
#[derive(Debug, PartialEq)]
enum Outcome {
    Ok(PhpArray),
    Throws {
        class: String,
        message: Vec<u8>,
        allowed: Vec<Vec<u8>>,
    },
}

fn actual(result: Result<PhpArray, Error>) -> Outcome {
    match result {
        Ok(attributes) => Outcome::Ok(attributes),
        Err(e) => Outcome::Throws {
            class: e.class().name().to_owned(),
            message: e.message().as_bytes().to_vec(),
            allowed: match e.detail() {
                Detail::AllowedMethods(methods) => {
                    methods.iter().map(|m| m.as_bytes().to_vec()).collect()
                }
                _ => Vec::new(),
            },
        },
    }
}

fn expected(json: &Json) -> Result<Outcome, TestError> {
    Ok(if json[0] == "ok" {
        Outcome::Ok(array(&json[1])?)
    } else {
        Outcome::Throws {
            class: json[1].as_str().ok_or("class")?.to_owned(),
            message: bytes(&json[2])?,
            allowed: json[3]
                .as_array()
                .ok_or("allowed")?
                .iter()
                .map(bytes)
                .collect::<Result<_, _>>()?,
        }
    })
}

#[test]
fn matches_recorded_answers() -> Result<(), TestError> {
    let cases: Json = serde_json::from_str(DATA)?;
    let cases = cases.as_array().ok_or("cases")?;
    let mut failures = Vec::new();
    let mut requests = 0;
    for (index, case) in cases.iter().enumerate() {
        let routes = collection(&case[0])?;
        for request in case[1].as_array().ok_or("requests")? {
            let path = bytes(&request[0])?;
            let context = RequestContext::new(
                bytes(&request[4])?,
                request[1].as_str().ok_or("method")?,
                request[2].as_str().ok_or("host")?,
                request[3].as_str().ok_or("scheme")?,
                80,
                443,
                "/",
                "",
                None,
            );
            let plain = UrlMatcher::new(routes.clone(), context.clone());
            let redirecting = RedirectableUrlMatcher::new(routes.clone(), context, Redirecting);
            for (kind, got, want) in [
                ("UrlMatcher", actual(plain.match_path(&path)), &request[5]),
                (
                    "RedirectableUrlMatcher",
                    actual(redirecting.match_path(&path)),
                    &request[6],
                ),
            ] {
                let want = expected(want)?;
                if got != want {
                    failures.push(format!(
                        "case {index} {kind} {:?}: got {got:?}, want {want:?}",
                        String::from_utf8_lossy(&path)
                    ));
                }
            }
            requests += 1;
        }
    }
    assert!(requests >= 10_000, "{requests} requests");
    assert!(
        failures.is_empty(),
        "{} of {} answers differ:\n{}",
        failures.len(),
        requests * 2,
        failures[..failures.len().min(20)].join("\n")
    );
    Ok(())
}
