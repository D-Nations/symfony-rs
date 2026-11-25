//! Port of Symfony\Component\Routing\Tests\Matcher\RedirectableUrlMatcherTest.php
//! on [`RedirectableUrlMatcher`]. The class extends `UrlMatcherTest`
//! upstream: the inherited tests run here too, from `url_matcher/mod.rs`,
//! with a `redirect()` that returns `[]` (the test's anonymous subclass);
//! the four this class overrides use its own versions.
//!
//! `getUrlMatcher(..., true)` is a PHPUnit mock of `redirect()`: [`Mock`]
//! records the calls and checks `expects()`, `with()` and `willReturn()`;
//! expectations set after a first match apply from then on, as PHPUnit's do.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Tests/Matcher/RedirectableUrlMatcherTest.php
#![allow(non_snake_case)]

mod url_matcher;

use std::cell::RefCell;

use php_value::{PhpArray, PhpString};
use symfony_http_foundation::request::Request;
use symfony_routing::{
    exception::Error,
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    route_collection::RouteCollection,
};
use url_matcher::{
    Matcher, RESOURCE_NOT_FOUND, TestResult, assert_equals, assert_same, coll, expect_exception,
    method, route, strings,
};

/// The anonymous subclass: `redirect()` returns `[]`.
struct Empty;

impl RedirectableUrlMatcherInterface for Empty {
    fn redirect(&self, _: &[u8], _: &PhpString, _: Option<&[u8]>) -> Result<PhpArray, Error> {
        Ok(PhpArray::new())
    }
}

impl<R: RedirectableUrlMatcherInterface> Matcher for RedirectableUrlMatcher<R> {
    fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Error> {
        RedirectableUrlMatcher::match_path(self, pathinfo)
    }

    fn match_request(&self, request: &mut Request) -> Result<PhpArray, Error> {
        RedirectableUrlMatcher::match_request(self, request)
    }

    fn context(&self) -> &RequestContext {
        RedirectableUrlMatcher::context(self)
    }
}

/// `getUrlMatcher($routes, $context)`.
fn get_url_matcher(routes: RouteCollection, context: Option<RequestContext>) -> Box<dyn Matcher> {
    Box::new(RedirectableUrlMatcher::new(
        routes,
        context.unwrap_or_default(),
        Empty,
    ))
}

/// `->expects(...)`.
#[derive(Debug, Clone, Copy)]
enum Times {
    Any,
    Once,
    Never,
    Exactly(usize),
}

/// `->with($path, $route, $scheme)`: the arguments given are checked.
#[derive(Debug, Clone, Default)]
struct With {
    path: Option<&'static str>,
    route: Option<&'static str>,
    scheme: Option<Option<&'static str>>,
}

#[derive(Debug)]
struct Expectation {
    times: Times,
    with: With,
    returns: PhpArray,
    calls: usize,
}

/// A PHPUnit mock of `redirect()`.
#[derive(Debug)]
struct Mock(RefCell<Expectation>);

impl Mock {
    fn new() -> Self {
        Self(RefCell::new(Expectation {
            times: Times::Any,
            with: With::default(),
            returns: PhpArray::new(),
            calls: 0,
        }))
    }

    /// `$matcher->expects($times)->method('redirect')->with(...)->willReturn($returns)`.
    fn expects(&self, times: Times, with: With, returns: PhpArray) {
        let mut e = self.0.borrow_mut();
        e.times = times;
        e.with = with;
        e.returns = returns;
        e.calls = 0;
    }

    /// PHPUnit's check after the test.
    fn verify(&self) {
        let e = self.0.borrow();
        match e.times {
            Times::Any => {}
            Times::Once => assert_eq!(e.calls, 1, "redirect() expected once"),
            Times::Never => assert_eq!(e.calls, 0, "redirect() expected never"),
            Times::Exactly(n) => assert_eq!(e.calls, n, "redirect() expected {n} times"),
        }
    }
}

impl RedirectableUrlMatcherInterface for Mock {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, Error> {
        let mut e = self.0.borrow_mut();
        e.calls += 1;
        if let Some(expected) = e.with.path {
            assert_eq!(path, expected.as_bytes(), "redirect() path");
        }
        if let Some(expected) = e.with.route {
            assert_eq!(route.as_bytes(), expected.as_bytes(), "redirect() route");
        }
        if let Some(expected) = e.with.scheme {
            assert_eq!(scheme, expected.map(str::as_bytes), "redirect() scheme");
        }
        Ok(e.returns.clone())
    }
}

/// `getUrlMatcher($routes, $context, true)`.
fn get_mock(
    routes: RouteCollection,
    context: Option<RequestContext>,
) -> RedirectableUrlMatcher<Mock> {
    RedirectableUrlMatcher::new(routes, context.unwrap_or_default(), Mock::new())
}

fn with(path: &'static str, route: &'static str, scheme: Option<&'static str>) -> With {
    With {
        path: Some(path),
        route: Some(route),
        scheme: Some(scheme),
    }
}

fn with_path(path: &'static str) -> With {
    With {
        path: Some(path),
        ..With::default()
    }
}

fn with_path_route(path: &'static str, route: &'static str) -> With {
    With {
        path: Some(path),
        route: Some(route),
        scheme: None,
    }
}

#[test]
fn testMissingTrailingSlash() -> TestResult {
    let coll = coll(vec![("foo", route("/foo/"))])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Once, With::default(), PhpArray::new());
    matcher.match_path(b"/foo")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testExtraTrailingSlash() -> TestResult {
    let coll = coll(vec![("foo", route("/foo"))])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Once, With::default(), PhpArray::new());
    matcher.match_path(b"/foo/")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testRedirectWhenNoSlashForNonSafeMethod() -> TestResult {
    let coll = coll(vec![("foo", route("/foo/"))])?;
    let mut context = RequestContext::default();
    context.set_method("POST");
    let matcher = get_url_matcher(coll, Some(context));
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

#[test]
fn testSchemeRedirectRedirectsToFirstScheme() -> TestResult {
    let coll = coll(vec![("foo", route("/foo").schemes(&["FTP", "HTTPS"]))])?;
    let matcher = get_mock(coll, None);
    matcher.redirector().expects(
        Times::Once,
        with("/foo", "foo", Some("ftp")),
        strings(&[("_route", "foo")]),
    );
    matcher.match_path(b"/foo")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testNoSchemaRedirectIfOneOfMultipleSchemesMatches() -> TestResult {
    let coll = coll(vec![("foo", route("/foo").schemes(&["https", "http"]))])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Never, With::default(), PhpArray::new());
    matcher.match_path(b"/foo")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testSchemeRedirectWithParams() -> TestResult {
    let coll = coll(vec![("foo", route("/foo/{bar}").schemes(&["https"]))])?;
    let matcher = get_mock(coll, None);
    matcher.redirector().expects(
        Times::Once,
        with("/foo/baz", "foo", Some("https")),
        strings(&[("redirect", "value")]),
    );
    assert_equals(
        &[("_route", "foo"), ("bar", "baz"), ("redirect", "value")],
        &matcher.match_path(b"/foo/baz")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testSchemeRedirectForRoot() -> TestResult {
    let coll = coll(vec![("foo", route("/").schemes(&["https"]))])?;
    let matcher = get_mock(coll, None);
    matcher.redirector().expects(
        Times::Once,
        with("/", "foo", Some("https")),
        strings(&[("redirect", "value")]),
    );
    assert_equals(
        &[("_route", "foo"), ("redirect", "value")],
        &matcher.match_path(b"/")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testSlashRedirectWithParams() -> TestResult {
    let coll = coll(vec![("foo", route("/foo/{bar}/"))])?;
    let matcher = get_mock(coll, None);
    matcher.redirector().expects(
        Times::Once,
        with("/foo/baz/", "foo", None),
        strings(&[("redirect", "value")]),
    );
    assert_equals(
        &[("_route", "foo"), ("bar", "baz"), ("redirect", "value")],
        &matcher.match_path(b"/foo/baz")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testRedirectPreservesUrlEncoding() -> TestResult {
    let coll = coll(vec![("foo", route("/foo:bar/"))])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Once, with_path("/foo%3Abar/"), PhpArray::new());
    matcher.match_path(b"/foo%3Abar")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testSchemeRequirement() -> TestResult {
    let coll = coll(vec![("foo", route("/foo").schemes(&["https"]))])?;
    let matcher = get_mock(coll, Some(RequestContext::default()));
    matcher.redirector().expects(
        Times::Once,
        with("/foo", "foo", Some("https")),
        PhpArray::new(),
    );
    assert_same(&[("_route", "foo")], &matcher.match_path(b"/foo")?);
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testFallbackPage() -> TestResult {
    let coll1 = coll(vec![("foo", route("/foo/")), ("bar", route("/{name}"))])?;
    let matcher = get_mock(coll1, None);
    matcher.redirector().expects(
        Times::Once,
        with_path_route("/foo/", "foo"),
        strings(&[("_route", "foo")]),
    );
    assert_same(&[("_route", "foo")], &matcher.match_path(b"/foo")?);
    matcher.redirector().verify();

    let coll2 = coll(vec![("foo", route("/foo")), ("bar", route("/{name}/"))])?;
    let matcher = get_mock(coll2, None);
    matcher.redirector().expects(
        Times::Once,
        with_path_route("/foo", "foo"),
        strings(&[("_route", "foo")]),
    );
    assert_same(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testMissingTrailingSlashAndScheme() -> TestResult {
    let coll = coll(vec![("foo", route("/foo/").schemes(&["https"]))])?;
    let matcher = get_mock(coll, None);
    matcher.redirector().expects(
        Times::Once,
        with("/foo/", "foo", Some("https")),
        PhpArray::new(),
    );
    matcher.match_path(b"/foo")?;
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testExtraTrailingSlashForHeadOnlyRoute() -> TestResult {
    let coll = coll(vec![("foo", route("/foo").methods(&["HEAD"]))])?;
    let matcher = get_mock(coll, Some(method("HEAD")));
    matcher
        .redirector()
        .expects(Times::Once, with_path_route("/foo", "foo"), PhpArray::new());
    assert_same(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testMissingTrailingSlashAndSchemeWhenBothVariantsExist() -> TestResult {
    let coll1 = coll(vec![
        ("a", route("/foo/").schemes(&["https"])),
        ("b", route("/foo")),
    ])?;
    let matcher = get_mock(coll1, None);
    matcher.redirector().expects(
        Times::Exactly(2),
        with("/foo/", "a", Some("https")),
        PhpArray::new(),
    );
    assert_same(&[("_route", "a")], &matcher.match_path(b"/foo")?);
    assert_same(&[("_route", "a")], &matcher.match_path(b"/foo/")?);
    matcher.redirector().verify();

    let coll2 = coll(vec![
        ("a", route("/foo/{bar}/").schemes(&["https"])),
        ("b", route("/foo/{bar}")),
    ])?;
    let matcher = get_mock(coll2, None);
    matcher.redirector().expects(
        Times::Exactly(2),
        with("/foo/baz/", "a", Some("https")),
        PhpArray::new(),
    );
    assert_equals(
        &[("_route", "a"), ("bar", "baz")],
        &matcher.match_path(b"/foo/baz")?,
    );
    assert_equals(
        &[("_route", "a"), ("bar", "baz")],
        &matcher.match_path(b"/foo/baz/")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testSlashAndVerbPrecedenceWithRedirection() -> TestResult {
    let coll = coll(vec![
        (
            "a",
            route("/api/customers/{customerId}/contactpersons").methods(&["post"]),
        ),
        (
            "b",
            route("/api/customers/{customerId}/contactpersons/").methods(&["get"]),
        ),
    ])?;
    let matcher = get_mock(coll, None);
    let expected = [("_route", "b"), ("customerId", "123")];
    assert_equals(
        &expected,
        &matcher.match_path(b"/api/customers/123/contactpersons/")?,
    );

    matcher.redirector().expects(
        Times::Once,
        with_path("/api/customers/123/contactpersons/"),
        PhpArray::new(),
    );
    assert_equals(
        &expected,
        &matcher.match_path(b"/api/customers/123/contactpersons")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testNonGreedyTrailingRequirement() -> TestResult {
    let coll = coll(vec![("a", route("/{a}").requirements(&[("a", "\\d+")]))])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Once, with_path("/123"), PhpArray::new());
    assert_equals(
        &[("_route", "a"), ("a", "123")],
        &matcher.match_path(b"/123/")?,
    );
    matcher.redirector().verify();
    Ok(())
}

#[test]
fn testTrailingRequirementWithDefaultA() -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/fr-fr/{a}")
            .defaults(&[("a", "aaa")])
            .requirements(&[("a", ".+")]),
    )])?;
    let matcher = get_mock(coll, None);
    matcher
        .redirector()
        .expects(Times::Once, with_path("/fr-fr"), PhpArray::new());
    assert_equals(
        &[("_route", "a"), ("a", "aaa")],
        &matcher.match_path(b"/fr-fr/")?,
    );
    matcher.redirector().verify();
    Ok(())
}

// The tests inherited from UrlMatcherTest.

#[test]
fn testZero() -> TestResult {
    url_matcher::testZero(get_url_matcher)
}

#[test]
fn testNoMethodSoAllowed() -> TestResult {
    url_matcher::testNoMethodSoAllowed(get_url_matcher)
}

#[test]
fn testMethodNotAllowed() -> TestResult {
    url_matcher::testMethodNotAllowed(get_url_matcher)
}

#[test]
fn testMatchRequestHonorsTheRequestsMethodOverTheStaticContext() -> TestResult {
    url_matcher::testMatchRequestHonorsTheRequestsMethodOverTheStaticContext(get_url_matcher)
}

#[test]
fn testMatchRequestRestoresTheContextAfterwards() -> TestResult {
    url_matcher::testMatchRequestRestoresTheContextAfterwards(get_url_matcher)
}

#[test]
fn testMethodNotAllowedOnRoot() -> TestResult {
    url_matcher::testMethodNotAllowedOnRoot(get_url_matcher)
}

#[test]
fn testHeadAllowedWhenRequirementContainsGet() -> TestResult {
    url_matcher::testHeadAllowedWhenRequirementContainsGet(get_url_matcher)
}

#[test]
fn testHeadAllowedWhenRequirementContainsHead() -> TestResult {
    url_matcher::testHeadAllowedWhenRequirementContainsHead(get_url_matcher)
}

#[test]
fn testMethodNotAllowedAggregatesAllowedMethods() -> TestResult {
    url_matcher::testMethodNotAllowedAggregatesAllowedMethods(get_url_matcher)
}

#[test]
fn testPatternMatchAndParameterReturn() -> TestResult {
    url_matcher::testPatternMatchAndParameterReturn(get_url_matcher)
}

#[test]
fn testDefaultsAreMerged() -> TestResult {
    url_matcher::testDefaultsAreMerged(get_url_matcher)
}

#[test]
fn testMethodIsIgnoredIfNoMethodGiven() -> TestResult {
    url_matcher::testMethodIsIgnoredIfNoMethodGiven(get_url_matcher)
}

#[test]
fn testRouteWithOptionalVariableAsFirstSegment() -> TestResult {
    url_matcher::testRouteWithOptionalVariableAsFirstSegment(get_url_matcher)
}

#[test]
fn testRouteWithOnlyOptionalVariables() -> TestResult {
    url_matcher::testRouteWithOnlyOptionalVariables(get_url_matcher)
}

#[test]
fn testMatchWithPrefixes() -> TestResult {
    url_matcher::testMatchWithPrefixes(get_url_matcher)
}

#[test]
fn testMatchWithDynamicPrefix() -> TestResult {
    url_matcher::testMatchWithDynamicPrefix(get_url_matcher)
}

#[test]
fn testMatchSpecialRouteName() -> TestResult {
    url_matcher::testMatchSpecialRouteName(get_url_matcher)
}

#[test]
fn testMatchImportantVariable() -> TestResult {
    url_matcher::testMatchImportantVariable(get_url_matcher)
}

#[test]
fn testShortPathDoesNotMatchImportantVariable() -> TestResult {
    url_matcher::testShortPathDoesNotMatchImportantVariable(get_url_matcher)
}

#[test]
fn testTrailingEncodedNewlineIsNotOverlooked() -> TestResult {
    url_matcher::testTrailingEncodedNewlineIsNotOverlooked(get_url_matcher)
}

#[test]
fn testMatchNonAlpha() -> TestResult {
    url_matcher::testMatchNonAlpha(get_url_matcher)
}

#[test]
fn testMatchWithDotMetacharacterInRequirements() -> TestResult {
    url_matcher::testMatchWithDotMetacharacterInRequirements(get_url_matcher)
}

#[test]
fn testMatchOverriddenRoute() -> TestResult {
    url_matcher::testMatchOverriddenRoute(get_url_matcher)
}

#[test]
fn testMatchRegression() -> TestResult {
    url_matcher::testMatchRegression(get_url_matcher)
}

#[test]
fn testMultipleParams() -> TestResult {
    url_matcher::testMultipleParams(get_url_matcher)
}

#[test]
fn testDefaultRequirementForOptionalVariables() -> TestResult {
    url_matcher::testDefaultRequirementForOptionalVariables(get_url_matcher)
}

#[test]
fn testMatchingIsEager() -> TestResult {
    url_matcher::testMatchingIsEager(get_url_matcher)
}

#[test]
fn testAdjacentVariables() -> TestResult {
    url_matcher::testAdjacentVariables(get_url_matcher)
}

#[test]
fn testOptionalVariableWithNoRealSeparator() -> TestResult {
    url_matcher::testOptionalVariableWithNoRealSeparator(get_url_matcher)
}

#[test]
fn testRequiredVariableWithNoRealSeparator() -> TestResult {
    url_matcher::testRequiredVariableWithNoRealSeparator(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariable() -> TestResult {
    url_matcher::testDefaultRequirementOfVariable(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariableDisallowsSlash() -> TestResult {
    url_matcher::testDefaultRequirementOfVariableDisallowsSlash(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariableDisallowsNextSeparator() -> TestResult {
    url_matcher::testDefaultRequirementOfVariableDisallowsNextSeparator(get_url_matcher)
}

#[test]
fn testMissingTrailingSlashForNonSafeMethod() -> TestResult {
    url_matcher::testMissingTrailingSlashForNonSafeMethod(get_url_matcher)
}

#[test]
fn testExtraTrailingSlashForNonSafeMethod() -> TestResult {
    url_matcher::testExtraTrailingSlashForNonSafeMethod(get_url_matcher)
}

#[test]
fn testSchemeRequirementForNonSafeMethod() -> TestResult {
    url_matcher::testSchemeRequirementForNonSafeMethod(get_url_matcher)
}

#[test]
fn testSamePathWithDifferentScheme() -> TestResult {
    url_matcher::testSamePathWithDifferentScheme(get_url_matcher)
}

#[test]
fn testDecodeOnce() -> TestResult {
    url_matcher::testDecodeOnce(get_url_matcher)
}

#[test]
fn testCannotRelyOnPrefix() -> TestResult {
    url_matcher::testCannotRelyOnPrefix(get_url_matcher)
}

#[test]
fn testWithHost() -> TestResult {
    url_matcher::testWithHost(get_url_matcher)
}

#[test]
fn testWithHostOnRouteCollection() -> TestResult {
    url_matcher::testWithHostOnRouteCollection(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHosts() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHosts(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsInReverse(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsAndVariable() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsAndVariable(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsAndVariableInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsAndVariableInReverse(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithMethodsInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithMethodsInReverse(get_url_matcher)
}

#[test]
fn testVariableVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testVariableVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testVariableVariationInTrailingSlashWithMethodsInReverse() -> TestResult {
    url_matcher::testVariableVariationInTrailingSlashWithMethodsInReverse(get_url_matcher)
}

#[test]
fn testMixOfStaticAndVariableVariationInTrailingSlashWithHosts() -> TestResult {
    url_matcher::testMixOfStaticAndVariableVariationInTrailingSlashWithHosts(get_url_matcher)
}

#[test]
fn testMixOfStaticAndVariableVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testMixOfStaticAndVariableVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testWithOutHostHostDoesNotMatch() -> TestResult {
    url_matcher::testWithOutHostHostDoesNotMatch(get_url_matcher)
}

#[test]
fn testPathIsCaseSensitive() -> TestResult {
    url_matcher::testPathIsCaseSensitive(get_url_matcher)
}

#[test]
fn testHostIsCaseInsensitive() -> TestResult {
    url_matcher::testHostIsCaseInsensitive(get_url_matcher)
}

#[test]
fn testNoConfiguration() -> TestResult {
    url_matcher::testNoConfiguration(get_url_matcher)
}

#[test]
fn testNestedCollections() -> TestResult {
    url_matcher::testNestedCollections(get_url_matcher)
}

#[test]
fn testSchemeAndMethodMismatch() -> TestResult {
    url_matcher::testSchemeAndMethodMismatch(get_url_matcher)
}

#[test]
fn testUnicodeRoute() -> TestResult {
    url_matcher::testUnicodeRoute(get_url_matcher)
}

#[test]
fn testRequirementWithCapturingGroup() -> TestResult {
    url_matcher::testRequirementWithCapturingGroup(get_url_matcher)
}

#[test]
fn testDotAllWithCatchAll() -> TestResult {
    url_matcher::testDotAllWithCatchAll(get_url_matcher)
}

#[test]
fn testHostPattern() -> TestResult {
    url_matcher::testHostPattern(get_url_matcher)
}

#[test]
fn testUtf8Prefix() -> TestResult {
    url_matcher::testUtf8Prefix(get_url_matcher)
}

#[test]
fn testUtf8AndMethodMatching() -> TestResult {
    url_matcher::testUtf8AndMethodMatching(get_url_matcher)
}

#[test]
fn testHostWithDot() -> TestResult {
    url_matcher::testHostWithDot(get_url_matcher)
}

#[test]
fn testHostVariableDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testHostVariableWithPermissiveRequirementDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableWithPermissiveRequirementDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testHostVariableWithPermissiveRequirementMatchesTheHostOnly() -> TestResult {
    url_matcher::testHostVariableWithPermissiveRequirementMatchesTheHostOnly(get_url_matcher)
}

#[test]
fn testHostVariableWithNamedGroupInRequirementDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableWithNamedGroupInRequirementDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testStaticRouteWithHostVariableDoesNotMatchThePath() -> TestResult {
    url_matcher::testStaticRouteWithHostVariableDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testSlashVariant() -> TestResult {
    url_matcher::testSlashVariant(get_url_matcher)
}

#[test]
fn testSlashVariant2() -> TestResult {
    url_matcher::testSlashVariant2(get_url_matcher)
}

#[test]
fn testSlashWithVerb() -> TestResult {
    url_matcher::testSlashWithVerb(get_url_matcher)
}

#[test]
fn testSlashAndVerbPrecedence() -> TestResult {
    url_matcher::testSlashAndVerbPrecedence(get_url_matcher)
}

#[test]
fn testGreedyTrailingRequirement() -> TestResult {
    url_matcher::testGreedyTrailingRequirement(get_url_matcher)
}

#[test]
fn testTrailingRequirementWithDefault() -> TestResult {
    url_matcher::testTrailingRequirementWithDefault(get_url_matcher)
}

#[test]
fn testTrailingRequirementWithDefaultB() -> TestResult {
    url_matcher::testTrailingRequirementWithDefaultB(get_url_matcher)
}

#[test]
fn testRestrictiveTrailingRequirementWithStaticRouteAfter() -> TestResult {
    url_matcher::testRestrictiveTrailingRequirementWithStaticRouteAfter(get_url_matcher)
}

#[test]
fn testUtf8VarName() -> TestResult {
    url_matcher::testUtf8VarName(get_url_matcher)
}

#[test]
fn testParameterWithRequirementWithDefault() -> TestResult {
    url_matcher::testParameterWithRequirementWithDefault(get_url_matcher)
}

#[test]
fn testMapping() -> TestResult {
    url_matcher::testMapping(get_url_matcher)
}

#[test]
fn testMappingwithAlias() -> TestResult {
    url_matcher::testMappingwithAlias(get_url_matcher)
}
