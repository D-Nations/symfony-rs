//! Selected assertions from Symfony Routing's `UrlMatcherTest` and
//! `RedirectableUrlMatcherTest` at f3fefc6cad3748d538be970ca81b71f06d7aa8b3.
//! These tests cover the typed subset (`Definition`, `Segment`), not the
//! omitted PCRE/host APIs. The full upstream test classes run on the ported
//! matchers in `url_matcher_test.rs` and `redirectable_url_matcher_test.rs`;
//! the functions here are named `typed_*` after the upstream tests they follow.
//!
//! Upstream: symfony/routing Tests/Matcher/UrlMatcherTest.php Tests/Matcher/RedirectableUrlMatcherTest.php

use symfony_routing::{
    Definition, Match, Parameters, Requirement, RouteCollection, Segment, Target,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Route {
    name: &'static str,
    methods: Option<&'static [&'static str]>,
    bar: Option<String>,
}
impl Target for Route {
    fn methods(&self) -> Option<&'static [&'static str]> {
        self.methods
    }
}

fn definition(
    name: &'static str,
    segments: Vec<Segment>,
    methods: Option<&'static [&'static str]>,
) -> Definition<Route> {
    Definition::new(segments, move |p: &Parameters| {
        Some(Route {
            name,
            methods,
            bar: p.get("bar").map(str::to_owned),
        })
    })
}
fn foo(methods: Option<&'static [&'static str]>) -> RouteCollection<Route> {
    let mut routes = RouteCollection::default();
    routes.add(definition("foo", vec![Segment::Literal("foo")], methods));
    routes
}
fn found(result: Match<Route>) -> Option<Route> {
    if let Match::Found(route) = result {
        Some(route)
    } else {
        None
    }
}

#[test]
fn typed_zero() {
    assert_eq!(foo(None).match_request("GET", "0"), Match::NotFound);
}
#[test]
fn typed_no_method_so_allowed() {
    assert!(found(foo(None).match_request("GET", "/foo")).is_some());
}
#[test]
fn typed_method_not_allowed() {
    assert_eq!(
        foo(Some(&["POST"])).match_request("GET", "/foo"),
        Match::MethodNotAllowed {
            allow: vec!["POST"]
        }
    );
}
#[test]
fn typed_method_not_allowed_on_root() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![Segment::Literal("")],
        Some(&["GET"]),
    ));
    assert_eq!(
        routes.match_request("POST", "/"),
        Match::MethodNotAllowed { allow: vec!["GET"] }
    );
}
#[test]
fn typed_head_allowed_when_requirement_contains_get() {
    assert!(found(foo(Some(&["GET"])).match_request("HEAD", "/foo")).is_some());
}
#[test]
fn typed_head_allowed_when_requirement_contains_head() {
    let routes = foo(Some(&["HEAD"]));
    assert_eq!(
        found(routes.match_request("HEAD", "/foo")).map(|r| r.name),
        Some("foo")
    );
    assert_eq!(
        routes.match_request("GET", "/foo"),
        Match::MethodNotAllowed {
            allow: vec!["HEAD"]
        }
    );
}
#[test]
fn typed_method_not_allowed_aggregates_allowed_methods() {
    let mut routes = foo(Some(&["POST"]));
    routes.add(definition(
        "foo2",
        vec![Segment::Literal("foo")],
        Some(&["PUT", "DELETE"]),
    ));
    assert_eq!(
        routes.match_request("GET", "/foo"),
        Match::MethodNotAllowed {
            allow: vec!["POST", "PUT", "DELETE"]
        }
    );
}
#[test]
fn typed_pattern_match_and_parameter_return() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![
            Segment::Literal("foo"),
            Segment::Parameter("bar", Requirement::NonEmpty),
        ],
        None,
    ));
    assert_eq!(routes.match_request("GET", "/no-match"), Match::NotFound);
    assert_eq!(
        found(routes.match_request("GET", "/foo/baz")),
        Some(Route {
            name: "foo",
            methods: None,
            bar: Some("baz".into())
        })
    );
}
#[test]
fn typed_trailing_encoded_newline_is_not_overlooked() {
    assert_eq!(foo(None).match_request("GET", "/foo%0a"), Match::NotFound);
}
#[test]
fn typed_decode_once() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![
            Segment::Literal("foo"),
            Segment::Parameter("bar", Requirement::NonEmpty),
        ],
        None,
    ));
    assert_eq!(
        found(routes.match_request("GET", "/foo/bar%2523")).and_then(|r| r.bar),
        Some("bar%23".into())
    );
}
#[test]
fn typed_missing_trailing_slash() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![Segment::Literal("foo"), Segment::Literal("")],
        None,
    ));
    assert!(
        matches!(routes.match_request("GET", "/foo"), Match::TrailingSlashRedirect { path, .. } if path == "/foo/")
    );
}
#[test]
fn typed_extra_trailing_slash() {
    assert!(
        matches!(foo(None).match_request("GET", "/foo/"), Match::TrailingSlashRedirect { path, .. } if path == "/foo")
    );
}
#[test]
fn typed_redirect_when_no_slash_for_non_safe_method() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![Segment::Literal("foo"), Segment::Literal("")],
        None,
    ));
    assert_eq!(routes.match_request("POST", "/foo"), Match::NotFound);
}
#[test]
fn typed_redirect_preserves_url_encoding() {
    let mut routes = RouteCollection::default();
    routes.add(definition(
        "foo",
        vec![Segment::Literal("foo:bar"), Segment::Literal("")],
        None,
    ));
    assert!(
        matches!(routes.match_request("GET", "/foo%3Abar"), Match::TrailingSlashRedirect { path, .. } if path == "/foo%3Abar/")
    );
}
#[test]
fn typed_fallback_page() {
    for trailing in [false, true] {
        let mut routes = RouteCollection::default();
        let mut first = vec![Segment::Literal("foo")];
        let mut fallback = vec![Segment::Parameter("bar", Requirement::NonEmpty)];
        if trailing {
            first.push(Segment::Literal(""));
        } else {
            fallback.push(Segment::Literal(""));
        }
        routes.add(definition("foo", first, None));
        routes.add(definition("bar", fallback, None));
        let path = if trailing { "/foo" } else { "/foo/" };
        assert!(matches!(
            routes.match_request("GET", path),
            Match::TrailingSlashRedirect {
                target: Route { name: "foo", .. },
                ..
            }
        ));
    }
}
#[test]
fn typed_extra_trailing_slash_for_head_only_route() {
    assert!(matches!(
        foo(Some(&["HEAD"])).match_request("HEAD", "/foo/"),
        Match::TrailingSlashRedirect {
            target: Route { name: "foo", .. },
            ..
        }
    ));
}

#[test]
fn matches_the_installed_php_matcher() -> Result<(), Box<dyn std::error::Error>> {
    use Segment::{Literal as L, Parameter as P};
    let mut routes = foo(Some(&["POST"]));
    for (name, segments, methods) in [
        ("foo2", vec![L("foo")], Some(&["PUT", "DELETE"][..])),
        (
            "item",
            vec![L("item"), P("bar", Requirement::AsciiSlug)],
            Some(&["GET"][..]),
        ),
        ("with", vec![L("with"), L("")], Some(&["GET"][..])),
        ("colon", vec![L("foo:bar"), L("")], Some(&["GET"][..])),
        ("head", vec![L("head")], Some(&["HEAD"][..])),
        (
            "static",
            vec![L("fallback"), L("foo"), L("")],
            Some(&["GET"][..]),
        ),
        (
            "fallback",
            vec![L("fallback"), P("bar", Requirement::NonEmpty)],
            Some(&["GET"][..]),
        ),
        (
            "param",
            vec![L("param"), P("bar", Requirement::NonEmpty)],
            None,
        ),
        ("root", vec![L("")], None),
    ] {
        routes.add(definition(name, segments, methods));
    }
    let vectors: Vec<serde_json::Value> = serde_json::from_str(include_str!("data/matcher.json"))?;
    assert_eq!(vectors.len(), 90);
    for vector in vectors {
        let method = vector["method"].as_str().ok_or("missing method")?;
        let path = vector["path"].as_str().ok_or("missing path")?;
        let actual = match routes.match_request(method, path) {
            Match::Found(route) => {
                serde_json::json!({"kind":"found", "route":route.name, "bar":route.bar})
            }
            Match::TrailingSlashRedirect { path, target } => {
                serde_json::json!({"kind":"redirect", "route":target.name, "bar":target.bar, "path":path})
            }
            Match::MethodNotAllowed { allow } => {
                serde_json::json!({"kind":"method_not_allowed", "allow":allow})
            }
            Match::NotFound => serde_json::json!({"kind":"not_found"}),
        };
        assert_eq!(actual, vector["expected"], "{method} {path}");
    }
    Ok(())
}
