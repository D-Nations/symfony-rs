//! `RedirectableUrlMatcherInterface::redirect_in`: the context PHP's
//! `$this->context` is while `redirect()` runs. Not an upstream test:
//! PHP's `redirect()` reads the property; this pins that the port passes
//! the context of the match, with the scheme changed on a scheme retry.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::sync::Mutex;

use php_value::{PhpArray, PhpString, Value};
use symfony_routing::{
    exception::Error,
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    route::Route,
    route_collection::RouteCollection,
};

/// Records the scheme and HTTP port of each context it is given.
#[derive(Default)]
struct Recording(Mutex<Vec<(String, i64)>>);

impl RedirectableUrlMatcherInterface for Recording {
    fn redirect(&self, _: &[u8], _: &PhpString, _: Option<&[u8]>) -> Result<PhpArray, Error> {
        panic!("redirect_in() is called")
    }

    fn redirect_in(
        &self,
        _: &[u8],
        _: &PhpString,
        _: Option<&[u8]>,
        context: &RequestContext,
    ) -> Result<PhpArray, Error> {
        let scheme = String::from_utf8_lossy(context.scheme().as_bytes()).into_owned();
        self.0.lock().unwrap().push((scheme, context.http_port()));
        Ok(PhpArray::new())
    }
}

fn matcher(route: Route) -> RedirectableUrlMatcher<Recording> {
    let mut routes = RouteCollection::new();
    routes.add("foo", route, 0);
    let mut context = RequestContext::default();
    context.set_http_port(8080);
    RedirectableUrlMatcher::new(routes, context, Recording::default())
}

#[test]
fn a_trailing_slash_redirect_sees_the_context_of_the_match() {
    let matcher = matcher(Route::new("/foo/").unwrap());
    matcher.match_path(b"/foo").unwrap();
    assert_eq!(
        vec![("http".to_owned(), 8080)],
        *matcher.redirector().0.lock().unwrap()
    );
}

#[test]
fn a_scheme_redirect_sees_the_scheme_it_retried_with() {
    let mut route = Route::new("/foo").unwrap();
    route.set_schemes(["https"]);
    let matcher = matcher(route);
    matcher.match_path(b"/foo").unwrap();
    assert_eq!(
        vec![("https".to_owned(), 8080)],
        *matcher.redirector().0.lock().unwrap()
    );
    // The matcher's own context is unchanged.
    assert_eq!(b"http", matcher.context().scheme().as_bytes());
}

#[test]
fn the_default_redirect_in_calls_redirect() {
    struct Plain;
    impl RedirectableUrlMatcherInterface for Plain {
        fn redirect(
            &self,
            path: &[u8],
            _: &PhpString,
            _: Option<&[u8]>,
        ) -> Result<PhpArray, Error> {
            let mut attributes = PhpArray::new();
            attributes.insert("path", Value::String(PhpString::from(path)));
            Ok(attributes)
        }
    }
    let mut routes = RouteCollection::new();
    routes.add("foo", Route::new("/foo/").unwrap(), 0);
    let matcher = RedirectableUrlMatcher::new(routes, RequestContext::default(), Plain);
    let attributes = matcher.match_path(b"/foo").unwrap();
    assert_eq!(
        Some(&Value::string("/foo/")),
        attributes.get(&php_value::Key::from("path"))
    );
}
