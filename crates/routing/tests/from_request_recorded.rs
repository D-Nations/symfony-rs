//! `RequestContext::from_request()` and both matchers' `match_request()`
//! against PHP 8.4.27's answers recorded by
//! `oracle/recorders/routing_from_request.php` (`scripts/record.sh
//! routing-from-request`): the requests of http-foundation's
//! `request.json`, replayed under their configuration, and 304 requests
//! whose `SERVER_PORT` and `QUERY_STRING` are values of every type. Each
//! request answers `fromRequest()` on a clone of one of three contexts,
//! then `matchRequest()` on `UrlMatcher` and on a `RedirectableUrlMatcher`
//! over the same routes, in that order on the same Request; then the
//! matchers' contexts, which must be unchanged. A value is compared with
//! `===`; an exception by class and message.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_http_foundation::{
    php::{Globals, set_globals, with_globals},
    request::{Create, Parameters, Request},
};
use symfony_routing::{
    exception::Error,
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_matcher::UrlMatcher,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/from_request.json");
const REQUESTS: &str = include_str!("../../http-foundation/tests/data/request.json");

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

/// The recorder's `apply_config()`: `[proxies, header set, hosts,
/// parameter override, allowed, remote]`.
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

/// The recorder's `CONTEXTS`.
fn context(i: usize) -> RequestContext {
    match i % 3 {
        0 => RequestContext::new("", "GET", "localhost", "http", 80, 443, "/", "", None),
        1 => RequestContext::new("", "POST", "example.com", "http", 8080, 8443, "/", "", None),
        _ => RequestContext::new(
            "/base", "PUT", "ctx.test", "https", 81, 444, "/p", "q=1", None,
        ),
    }
}

fn route(path: &str, f: impl FnOnce(&mut Arguments)) -> Result<Route, TestError> {
    let mut arguments = Arguments::default();
    f(&mut arguments);
    Ok(Route::with(path, arguments)?)
}

fn list(items: &[&str]) -> Vec<PhpString> {
    items.iter().map(|s| PhpString::from(*s)).collect()
}

fn pairs(items: &[(&str, &str)]) -> PhpArray {
    items.iter().map(|(k, v)| (*k, Value::string(*v))).collect()
}

/// The recorder's `routes()`.
fn routes() -> Result<RouteCollection, TestError> {
    let mut routes = RouteCollection::new();
    routes.add("home", route("/", |_| {})?, 0);
    routes.add("foo_get", route("/foo", |a| a.methods = list(&["GET"]))?, 0);
    routes.add(
        "foo_post",
        route("/foo", |a| a.methods = list(&["POST", "PUT"]))?,
        0,
    );
    routes.add("slash", route("/foo/bar/", |_| {})?, 0);
    routes.add(
        "secure",
        route("/secure", |a| a.schemes = list(&["https"]))?,
        0,
    );
    routes.add(
        "plain",
        route("/plain", |a| a.schemes = list(&["http"]))?,
        0,
    );
    routes.add(
        "host",
        route("/h", |a| a.host = "{sub}.example.com".into())?,
        0,
    );
    routes.add(
        "trusted",
        route("/t", |a| a.host = "trusted.test".into())?,
        0,
    );
    routes.add(
        "var",
        route("/a/{x}", |a| {
            a.defaults = pairs(&[("x", "d")]);
            a.requirements = pairs(&[("x", "[^/]+")]);
        })?,
        0,
    );
    routes.add(
        "index",
        route("/index.php/{rest}", |a| {
            a.requirements = pairs(&[("rest", ".*")]);
        })?,
        0,
    );
    Ok(routes)
}

/// The recorder's `Redirecting::redirect()`.
struct Redirecting;

impl RedirectableUrlMatcherInterface for Redirecting {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, Error> {
        let mut ret = PhpArray::new();
        ret.insert("_redirect", Value::string(path));
        ret.insert("_redirect_route", Value::String(route.clone()));
        ret.insert(
            "_redirect_scheme",
            scheme.map_or(Value::Null, Value::string),
        );
        Ok(ret)
    }
}

fn state(c: &RequestContext) -> Value {
    let s = |v: &PhpString| Value::String(v.clone());
    Value::Array(PhpArray::list([
        s(c.base_url()),
        s(c.path_info()),
        s(c.method()),
        s(c.host()),
        s(c.scheme()),
        Value::Int(c.http_port()),
        Value::Int(c.https_port()),
        s(c.query_string()),
    ]))
}

/// A call's outcome, as both sides record it.
#[derive(PartialEq)]
enum Outcome {
    Ok(Value),
    Throws(String, Vec<u8>),
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok(v) => write!(f, "Ok({v:?})"),
            Self::Throws(class, message) => {
                write!(f, "Throws({class}, {:?})", String::from_utf8_lossy(message))
            }
        }
    }
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

fn got(result: Result<Value, Error>) -> Outcome {
    match result {
        Ok(v) => Outcome::Ok(v),
        Err(e) => Outcome::Throws(e.class().name().to_owned(), e.message().as_bytes().to_vec()),
    }
}

/// The recorder's `answers()`, in its order.
fn answers(request: &mut Request, i: usize, routes: &RouteCollection) -> Vec<Outcome> {
    let mut from = context(i);
    let from = got(from.from_request(request).map(|c| state(c)));
    let url = UrlMatcher::new(routes.clone(), context(i));
    let redirecting = RedirectableUrlMatcher::new(routes.clone(), context(i), Redirecting);
    vec![
        from,
        got(url.match_request(request).map(Value::Array)),
        got(redirecting.match_request(request).map(Value::Array)),
        Outcome::Ok(Value::Array(PhpArray::list([
            state(url.context()),
            state(redirecting.context()),
        ]))),
    ]
}

fn compare(
    label: &str,
    want: &Json,
    have: &[Outcome],
    failures: &mut Vec<String>,
) -> Result<(), TestError> {
    let want = want.as_array().ok_or("answers")?;
    assert_eq!(want.len(), have.len(), "{label}");
    for (n, (w, h)) in want.iter().zip(have).enumerate() {
        // The last answer is a plain typed value, not ["ok", value].
        let w = if n == 3 {
            Outcome::Ok(vectors::decode(w)?)
        } else {
            recorded(w)?
        };
        if !w.same(h) {
            failures.push(format!("{label} answer {n}: PHP {w:?}, Rust {h:?}"));
        }
    }
    Ok(())
}

#[test]
fn matches_recorded_requests() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let requests: Json = serde_json::from_str(REQUESTS)?;
    let requests = requests["cases"].as_array().ok_or("request cases")?;
    let routes = routes()?;
    let mut failures = Vec::new();

    let cases = data["cases"].as_array().ok_or("cases")?;
    assert!(cases.len() >= 3500, "{} cases", cases.len());
    for case in cases {
        let i = usize::try_from(case[0].as_u64().ok_or("index")?)?;
        let source = requests.get(i).ok_or("no such request")?;
        configure(&source[0])?;
        let input = &source[2];
        let mut request = if source[1] == "server" {
            Request::new(Parameters {
                server: array(&input[0])?,
                request: array(&input[1])?,
                query: array(&input[2])?,
                ..Parameters::default()
            })?
        } else {
            Request::create_with(
                bytes(&input[0])?,
                Create {
                    method: Some(PhpString::from(input[1].as_str().ok_or("method")?)),
                    parameters: array(&input[2])?,
                    server: array(&input[3])?,
                    ..Create::default()
                },
            )?
        };
        let have = answers(&mut request, i, &routes);
        compare(&format!("request {i}"), &case[1], &have, &mut failures)?;
    }

    configure(&serde_json::json!([[], -1, [], false, null, null]))?;
    let edges = data["edges"].as_array().ok_or("edges")?;
    assert_eq!(304, edges.len());
    for (i, case) in edges.iter().enumerate() {
        let server = array(&case[0])?;
        let mut request = Request::new(Parameters {
            server: server.clone(),
            ..Parameters::default()
        })?;
        let have = answers(&mut request, i, &routes);
        compare(
            &format!("edge {i} {server:?}"),
            &case[1],
            &have,
            &mut failures,
        )?;
    }
    Request::reset_statics();
    with_globals(|g| *g = Globals::default());

    assert!(
        failures.is_empty(),
        "{} answers differ:\n{}",
        failures.len(),
        failures[..failures.len().min(30)].join("\n")
    );
    Ok(())
}
