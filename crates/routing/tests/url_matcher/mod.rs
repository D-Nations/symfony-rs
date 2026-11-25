//! The test methods of upstream's `UrlMatcherTest`, as functions of the
//! matcher under test: `url_matcher_test.rs` runs them on `UrlMatcher`,
//! `redirectable_url_matcher_test.rs` on `RedirectableUrlMatcher` (whose
//! upstream test class extends `UrlMatcherTest`). Same names and
//! assertions; `assertEquals` on arrays ignores key order, `assertSame`
//! does not.
//!
//! Not here (pending in the manifest): the tests of route conditions,
//! which need ExpressionLanguage (testCondition, testZeroCondition,
//! testZeroConditionNextToOtherConditions, testRequestCondition,
//! testRouteParametersCondition, testSiblingRoutes).
//!
//! `assertSame($originalContext, $matcher->getContext())` compares by
//! value: the matcher owns its context.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Tests/Matcher/UrlMatcherTest.php
#![allow(dead_code, non_snake_case)]

use php_pcre::preg_quote;
use php_value::{Key, PhpArray, PhpString, Value, url::rawurlencode};
use symfony_http_foundation::request::{Create, Request};
use symfony_routing::{
    exception::{Detail, Error},
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub const RESOURCE_NOT_FOUND: &str =
    "Symfony\\Component\\Routing\\Exception\\ResourceNotFoundException";
pub const METHOD_NOT_ALLOWED: &str =
    "Symfony\\Component\\Routing\\Exception\\MethodNotAllowedException";
pub const NO_CONFIGURATION: &str =
    "Symfony\\Component\\Routing\\Exception\\NoConfigurationException";

/// What the tests call on a matcher.
pub trait Matcher {
    fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Error>;
    fn match_request(&self, request: &mut Request) -> Result<PhpArray, Error>;
    fn context(&self) -> &RequestContext;
}

/// `Request::create($uri, $method)`.
pub fn request(uri: &str, method: &str) -> Result<Request, Box<dyn std::error::Error>> {
    Ok(Request::create_with(
        uri,
        Create {
            method: Some(PhpString::from(method)),
            ..Create::default()
        },
    )?)
}

/// `getUrlMatcher($routes, $context)` of the test class.
pub type GetUrlMatcher = fn(RouteCollection, Option<RequestContext>) -> Box<dyn Matcher>;

/// `new Route($path, ...)` with upstream's argument order as builder calls.
pub struct R {
    path: String,
    arguments: Arguments,
}

pub fn route(path: &str) -> R {
    R {
        path: path.to_owned(),
        arguments: Arguments::default(),
    }
}

impl R {
    pub fn defaults(mut self, pairs: &[(&str, &str)]) -> Self {
        self.arguments.defaults = strings(pairs);
        self
    }

    pub fn requirements(mut self, pairs: &[(&str, &str)]) -> Self {
        self.arguments.requirements = strings(pairs);
        self
    }

    /// `['utf8' => $utf8]` as the options.
    pub fn utf8(mut self, utf8: bool) -> Self {
        self.arguments.options = [("utf8", Value::Bool(utf8))].into_iter().collect();
        self
    }

    pub fn host(mut self, host: &str) -> Self {
        self.arguments.host = PhpString::from(host);
        self
    }

    pub fn schemes(mut self, schemes: &[&str]) -> Self {
        self.arguments.schemes = schemes.iter().map(|s| PhpString::from(*s)).collect();
        self
    }

    pub fn methods(mut self, methods: &[&str]) -> Self {
        self.arguments.methods = methods.iter().map(|s| PhpString::from(*s)).collect();
        self
    }

    pub fn build(self) -> Result<Route, Error> {
        Route::with(self.path, self.arguments)
    }
}

/// `new RouteCollection()` with `add($name, $route)` for each pair.
pub fn coll(routes: Vec<(&str, R)>) -> Result<RouteCollection, Error> {
    let mut collection = RouteCollection::new();
    for (name, r) in routes {
        collection.add(name, r.build()?, 0);
    }
    Ok(collection)
}

pub fn strings(pairs: &[(&str, &str)]) -> PhpArray {
    pairs.iter().map(|(k, v)| (*k, Value::string(*v))).collect()
}

/// `new RequestContext($baseUrl, $method, $host)`.
pub fn ctx(base_url: &str, method: &str, host: &str) -> RequestContext {
    RequestContext::new(base_url, method, host, "http", 80, 443, "/", "", None)
}

/// `new RequestContext('', $method)`.
pub fn method(method: &str) -> RequestContext {
    ctx("", method, "localhost")
}

/// `$this->assertEquals($expected, $actual)` for an array of strings:
/// the same keys with equal values, in any order.
pub fn assert_equals(expected: &[(&str, &str)], actual: &PhpArray) {
    let expected: PhpArray = strings(expected);
    assert_equals_array(&expected, actual);
}

pub fn assert_equals_array(expected: &PhpArray, actual: &PhpArray) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "expected {expected:?}, got {actual:?}"
    );
    for (key, value) in expected.iter() {
        assert_eq!(
            actual.get(key),
            Some(value),
            "key {key:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

/// `$this->assertSame($expected, $actual)`: also in the same order.
pub fn assert_same(expected: &[(&str, &str)], actual: &PhpArray) {
    let expected: PhpArray = strings(expected);
    assert_eq!(&expected, actual);
}

/// `$actual[$key]`.
pub fn at<'a>(actual: &'a PhpArray, key: &str) -> Option<&'a Value> {
    actual.get(&Key::from_bytes(key.as_bytes()))
}

/// `$this->expectException($class)` around one call.
pub fn expect_exception(result: Result<PhpArray, Error>, class: &str) -> Error {
    match result {
        Ok(ret) => panic!("expected {class}, got {ret:?}"),
        Err(e) => {
            assert!(e.is_a(class), "expected {class}, got {e:?}");
            e
        }
    }
}

/// `$e->getAllowedMethods()`.
pub fn allowed_methods(e: &Error) -> Vec<String> {
    match e.detail() {
        Detail::AllowedMethods(methods) => methods
            .iter()
            .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
            .collect(),
        other => panic!("no allowed methods: {other:?}"),
    }
}

pub fn testZero(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("index", route("/"))])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"0"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testNoMethodSoAllowed(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo"))])?;
    let matcher = get(coll, None);
    matcher.match_path(b"/foo")?;
    Ok(())
}

pub fn testMethodNotAllowed(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").methods(&["post"]))])?;
    let matcher = get(coll, None);
    let e = expect_exception(matcher.match_path(b"/foo"), METHOD_NOT_ALLOWED);
    assert_eq!(allowed_methods(&e), ["POST"]);
    Ok(())
}

pub fn testMatchRequestHonorsTheRequestsMethodOverTheStaticContext(
    get: GetUrlMatcher,
) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").methods(&["GET"]))])?;
    let matcher = get(coll, Some(method("POST")));
    assert_same(
        &[("_route", "foo")],
        &matcher.match_request(&mut request("/foo", "GET")?)?,
    );
    Ok(())
}

pub fn testMatchRequestRestoresTheContextAfterwards(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").methods(&["GET"]))])?;
    let original_context = ctx("", "POST", "example.com");
    let matcher = get(coll, Some(original_context.clone()));
    matcher.match_request(&mut request("https://other.example/foo", "GET")?)?;

    assert_eq!(b"POST", matcher.context().method().as_bytes());
    assert_eq!(b"example.com", matcher.context().host().as_bytes());
    assert_eq!(&original_context, matcher.context());
    Ok(())
}

pub fn testMethodNotAllowedOnRoot(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/").methods(&["GET"]))])?;
    let matcher = get(coll, Some(method("POST")));
    let e = expect_exception(matcher.match_path(b"/"), METHOD_NOT_ALLOWED);
    assert_eq!(allowed_methods(&e), ["GET"]);
    Ok(())
}

pub fn testHeadAllowedWhenRequirementContainsGet(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").methods(&["get"]))])?;
    let matcher = get(coll, Some(method("head")));
    matcher.match_path(b"/foo")?;
    Ok(())
}

pub fn testHeadAllowedWhenRequirementContainsHead(get: GetUrlMatcher) -> TestResult {
    let routes = || coll(vec![("foo", route("/foo").methods(&["head"]))]);
    let matcher = get(routes()?, Some(method("head")));
    assert_same(&[("_route", "foo")], &matcher.match_path(b"/foo")?);

    let matcher = get(routes()?, None);
    let e = expect_exception(matcher.match_path(b"/foo"), METHOD_NOT_ALLOWED);
    assert_eq!(allowed_methods(&e), ["HEAD"]);
    Ok(())
}

pub fn testMethodNotAllowedAggregatesAllowedMethods(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("foo1", route("/foo").methods(&["post"])),
        ("foo2", route("/foo").methods(&["put", "delete"])),
    ])?;
    let matcher = get(coll, None);
    let e = expect_exception(matcher.match_path(b"/foo"), METHOD_NOT_ALLOWED);
    assert_eq!(allowed_methods(&e), ["POST", "PUT", "DELETE"]);
    Ok(())
}

pub fn testPatternMatchAndParameterReturn(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![("foo", route("/foo/{bar}"))])?;
    let matcher = get(collection, None);
    expect_exception(matcher.match_path(b"/no-match"), RESOURCE_NOT_FOUND);
    assert_equals(
        &[("_route", "foo"), ("bar", "baz")],
        &matcher.match_path(b"/foo/baz")?,
    );
    Ok(())
}

pub fn testDefaultsAreMerged(get: GetUrlMatcher) -> TestResult {
    // test that defaults are merged
    let collection = coll(vec![(
        "foo",
        route("/foo/{bar}").defaults(&[("def", "test")]),
    )])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "foo"), ("bar", "baz"), ("def", "test")],
        &matcher.match_path(b"/foo/baz")?,
    );
    Ok(())
}

pub fn testMethodIsIgnoredIfNoMethodGiven(get: GetUrlMatcher) -> TestResult {
    // test that route "method" is ignored if no method is given in the context
    let collection = || coll(vec![("foo", route("/foo").methods(&["get", "head"]))]);
    let matcher = get(collection()?, None);
    matcher.match_path(b"/foo")?;

    // route does not match with POST method context
    let matcher = get(collection()?, Some(method("post")));
    expect_exception(matcher.match_path(b"/foo"), METHOD_NOT_ALLOWED);

    // route does match with GET or HEAD method context
    let matcher = get(collection()?, None);
    matcher.match_path(b"/foo")?;
    let matcher = get(collection()?, Some(method("head")));
    matcher.match_path(b"/foo")?;
    Ok(())
}

pub fn testRouteWithOptionalVariableAsFirstSegment(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "bar",
        route("/{bar}/foo")
            .defaults(&[("bar", "bar")])
            .requirements(&[("bar", "foo|bar")]),
    )])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "bar"), ("bar", "bar")],
        &matcher.match_path(b"/bar/foo")?,
    );
    assert_equals(
        &[("_route", "bar"), ("bar", "foo")],
        &matcher.match_path(b"/foo/foo")?,
    );

    let collection = coll(vec![(
        "bar",
        route("/{bar}")
            .defaults(&[("bar", "bar")])
            .requirements(&[("bar", "foo|bar")]),
    )])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "bar"), ("bar", "foo")],
        &matcher.match_path(b"/foo")?,
    );
    assert_equals(
        &[("_route", "bar"), ("bar", "bar")],
        &matcher.match_path(b"/")?,
    );
    Ok(())
}

pub fn testRouteWithOnlyOptionalVariables(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "bar",
        route("/{foo}/{bar}").defaults(&[("foo", "foo"), ("bar", "bar")]),
    )])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "bar"), ("foo", "foo"), ("bar", "bar")],
        &matcher.match_path(b"/")?,
    );
    assert_equals(
        &[("_route", "bar"), ("foo", "a"), ("bar", "bar")],
        &matcher.match_path(b"/a")?,
    );
    assert_equals(
        &[("_route", "bar"), ("foo", "a"), ("bar", "b")],
        &matcher.match_path(b"/a/b")?,
    );
    Ok(())
}

pub fn testMatchWithPrefixes(get: GetUrlMatcher) -> TestResult {
    let mut collection = coll(vec![("foo", route("/{foo}"))])?;
    collection.add_prefix(b"/b", &PhpArray::new(), &PhpArray::new())?;
    collection.add_prefix(b"/a", &PhpArray::new(), &PhpArray::new())?;

    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "foo"), ("foo", "foo")],
        &matcher.match_path(b"/a/b/foo")?,
    );
    Ok(())
}

pub fn testMatchWithDynamicPrefix(get: GetUrlMatcher) -> TestResult {
    let mut collection = coll(vec![("foo", route("/{foo}"))])?;
    collection.add_prefix(b"/b", &PhpArray::new(), &PhpArray::new())?;
    collection.add_prefix(b"/{_locale}", &PhpArray::new(), &PhpArray::new())?;

    let matcher = get(collection, None);
    assert_equals(
        &[("_locale", "fr"), ("_route", "foo"), ("foo", "foo")],
        &matcher.match_path(b"/fr/b/foo")?,
    );
    Ok(())
}

pub fn testMatchSpecialRouteName(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![("$péß^a|", route("/bar"))])?;
    let matcher = get(collection, None);
    assert_equals(&[("_route", "$péß^a|")], &matcher.match_path(b"/bar")?);
    Ok(())
}

pub fn testMatchImportantVariable(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "index",
        route("/index.{!_format}").defaults(&[("_format", "xml")]),
    )])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "index"), ("_format", "xml")],
        &matcher.match_path(b"/index.xml")?,
    );
    Ok(())
}

pub fn testShortPathDoesNotMatchImportantVariable(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "index",
        route("/index.{!_format}").defaults(&[("_format", "xml")]),
    )])?;
    let matcher = get(collection, None);
    expect_exception(matcher.match_path(b"/index"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testTrailingEncodedNewlineIsNotOverlooked(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![("foo", route("/foo"))])?;
    let matcher = get(collection, None);
    expect_exception(matcher.match_path(b"/foo%0a"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testMatchNonAlpha(get: GetUrlMatcher) -> TestResult {
    let chars =
        "!\"$%éà &'()*+,./:;<=>@ABCDEFGHIJKLMNOPQRSTUVWXYZ\\[]^_`abcdefghijklmnopqrstuvwxyz{|}~-";
    let requirement = [b"[".as_slice(), &preg_quote(chars.as_bytes(), None), b"]+"].concat();
    let requirement = String::from_utf8(requirement)?;
    let collection = coll(vec![(
        "foo",
        route("/{foo}/bar")
            .requirements(&[("foo", &requirement)])
            .utf8(true),
    )])?;
    let matcher = get(collection, None);
    let encoded = [b"/".as_slice(), &rawurlencode(chars.as_bytes()), b"/bar"].concat();
    assert_equals(
        &[("_route", "foo"), ("foo", chars)],
        &matcher.match_path(&encoded)?,
    );
    let escaped = format!("/{}/bar", chars.replace('%', "%25"));
    assert_equals(
        &[("_route", "foo"), ("foo", chars)],
        &matcher.match_path(escaped.as_bytes())?,
    );
    Ok(())
}

pub fn testMatchWithDotMetacharacterInRequirements(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "foo",
        route("/{foo}/bar").requirements(&[("foo", ".+")]),
    )])?;
    let matcher = get(collection, None);
    // linefeed character is matched
    assert_equals(
        &[("_route", "foo"), ("foo", "\n")],
        &matcher.match_path(b"/%0A/bar")?,
    );
    Ok(())
}

pub fn testMatchOverriddenRoute(get: GetUrlMatcher) -> TestResult {
    let mut collection = coll(vec![("foo", route("/foo"))])?;
    let collection1 = coll(vec![("foo", route("/foo1"))])?;
    collection.add_collection(collection1);

    let matcher = get(collection, None);
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo1")?);
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testMatchRegression(get: GetUrlMatcher) -> TestResult {
    let coll1 = coll(vec![
        ("foo", route("/foo/{foo}")),
        ("bar", route("/foo/bar/{foo}")),
    ])?;
    let matcher = get(coll1, None);
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/foo/bar/bar")?,
    );

    let collection = coll(vec![("foo", route("/{bar}"))])?;
    let matcher = get(collection, None);
    expect_exception(matcher.match_path(b"/"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testMultipleParams(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("foo1", route("/foo/{a}/{b}")),
        ("foo2", route("/foo/{a}/test/test/{b}")),
        ("foo3", route("/foo/{a}/{b}/{c}/{d}")),
    ])?;
    let ret = get(coll, None).match_path(b"/foo/test/test/test/bar")?;
    assert_eq!(at(&ret, "_route"), Some(&Value::string("foo2")));
    Ok(())
}

pub fn testDefaultRequirementForOptionalVariables(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "test",
        route("/{page}.{_format}").defaults(&[("page", "index"), ("_format", "html")]),
    )])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("page", "my-page"), ("_format", "xml"), ("_route", "test")],
        &matcher.match_path(b"/my-page.xml")?,
    );
    Ok(())
}

pub fn testMatchingIsEager(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "test",
        route("/{foo}-{bar}-").requirements(&[("foo", ".+"), ("bar", ".+")]),
    )])?;
    let matcher = get(coll, None);
    assert_equals(
        &[
            ("foo", "text1-text2-text3"),
            ("bar", "text4"),
            ("_route", "test"),
        ],
        &matcher.match_path(b"/text1-text2-text3-text4-")?,
    );
    Ok(())
}

pub fn testAdjacentVariables(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "test",
        route("/{w}{x}{y}{z}.{_format}")
            .defaults(&[("z", "default-z"), ("_format", "html")])
            .requirements(&[("y", "y|Y")]),
    )])?;
    let matcher = get(coll, None);
    // 'w' eagerly matches as much as possible and the other variables match the remaining chars.
    // This also shows that the variables w-z must all exclude the separating char (the dot '.' in this case) by default requirement.
    // Otherwise they would also consume '.xml' and _format would never match as it's an optional variable.
    assert_equals(
        &[
            ("w", "wwwww"),
            ("x", "x"),
            ("y", "Y"),
            ("z", "Z"),
            ("_format", "xml"),
            ("_route", "test"),
        ],
        &matcher.match_path(b"/wwwwwxYZ.xml")?,
    );
    // As 'y' has custom requirement and can only be of value 'y|Y', it will leave  'ZZZ' to variable z.
    // So with carefully chosen requirements adjacent variables, can be useful.
    assert_equals(
        &[
            ("w", "wwwww"),
            ("x", "x"),
            ("y", "y"),
            ("z", "ZZZ"),
            ("_format", "html"),
            ("_route", "test"),
        ],
        &matcher.match_path(b"/wwwwwxyZZZ")?,
    );
    // z and _format are optional.
    assert_equals(
        &[
            ("w", "wwwww"),
            ("x", "x"),
            ("y", "y"),
            ("z", "default-z"),
            ("_format", "html"),
            ("_route", "test"),
        ],
        &matcher.match_path(b"/wwwwwxy")?,
    );

    expect_exception(matcher.match_path(b"/wxy.html"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testOptionalVariableWithNoRealSeparator(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "test",
        route("/get{what}").defaults(&[("what", "All")]),
    )])?;
    let matcher = get(coll, None);

    assert_equals(
        &[("what", "All"), ("_route", "test")],
        &matcher.match_path(b"/get")?,
    );
    assert_equals(
        &[("what", "Sites"), ("_route", "test")],
        &matcher.match_path(b"/getSites")?,
    );

    // Usually the character in front of an optional parameter can be left out, e.g. with pattern '/get/{what}' just '/get' would match.
    // But here the 't' in 'get' is not a separating character, so it makes no sense to match without it.
    expect_exception(matcher.match_path(b"/ge"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testRequiredVariableWithNoRealSeparator(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("test", route("/get{what}Suffix"))])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("what", "Sites"), ("_route", "test")],
        &matcher.match_path(b"/getSitesSuffix")?,
    );
    Ok(())
}

pub fn testDefaultRequirementOfVariable(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("test", route("/{page}.{_format}"))])?;
    let matcher = get(coll, None);
    assert_equals(
        &[
            ("page", "index"),
            ("_format", "mobile.html"),
            ("_route", "test"),
        ],
        &matcher.match_path(b"/index.mobile.html")?,
    );
    Ok(())
}

pub fn testDefaultRequirementOfVariableDisallowsSlash(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("test", route("/{page}.{_format}"))])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/index.sl/ash"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testDefaultRequirementOfVariableDisallowsNextSeparator(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "test",
        route("/{page}.{_format}").requirements(&[("_format", "html|xml")]),
    )])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/do.t.html"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testMissingTrailingSlash(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo/"))])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testExtraTrailingSlash(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo"))])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/foo/"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testMissingTrailingSlashForNonSafeMethod(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo/"))])?;
    let mut context = RequestContext::default();
    context.set_method("POST");
    let matcher = get(coll, Some(context));
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testExtraTrailingSlashForNonSafeMethod(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo"))])?;
    let mut context = RequestContext::default();
    context.set_method("POST");
    let matcher = get(coll, Some(context));
    expect_exception(matcher.match_path(b"/foo/"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testSchemeRequirement(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").schemes(&["https"]))])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testSchemeRequirementForNonSafeMethod(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo").schemes(&["https"]))])?;
    let mut context = RequestContext::default();
    context.set_method("POST");
    let matcher = get(coll, Some(context));
    expect_exception(matcher.match_path(b"/foo"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testSamePathWithDifferentScheme(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("https_route", route("/").schemes(&["https"])),
        ("http_route", route("/").schemes(&["http"])),
    ])?;
    let matcher = get(coll, None);
    assert_equals(&[("_route", "http_route")], &matcher.match_path(b"/")?);
    Ok(())
}

pub fn testDecodeOnce(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("foo", route("/foo/{foo}"))])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("foo", "bar%23"), ("_route", "foo")],
        &matcher.match_path(b"/foo/bar%2523")?,
    );
    Ok(())
}

pub fn testCannotRelyOnPrefix(get: GetUrlMatcher) -> TestResult {
    let mut coll1 = RouteCollection::new();

    let mut sub_coll = coll(vec![("bar", route("/bar"))])?;
    sub_coll.add_prefix(b"/prefix", &PhpArray::new(), &PhpArray::new())?;
    // overwrite the pattern, so the prefix is not valid anymore for this route in the collection
    if let Some(bar) = sub_coll.get_mut(b"bar")? {
        bar.set_path("/new")?;
    }

    coll1.add_collection(sub_coll);

    let matcher = get(coll1, None);
    assert_equals(&[("_route", "bar")], &matcher.match_path(b"/new")?);
    Ok(())
}

pub fn testWithHost(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "foo",
        route("/foo/{foo}").host("{locale}.example.com"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "en.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo"), ("locale", "en")],
        &matcher.match_path(b"/foo/bar")?,
    );
    Ok(())
}

pub fn testWithHostOnRouteCollection(get: GetUrlMatcher) -> TestResult {
    let routes = || -> Result<RouteCollection, Error> {
        let mut coll = coll(vec![
            ("foo", route("/foo/{foo}")),
            ("bar", route("/bar/{foo}").host("{locale}.example.net")),
        ])?;
        coll.set_host(b"{locale}.example.com", &PhpArray::new(), &PhpArray::new())?;
        Ok(coll)
    };

    let matcher = get(routes()?, Some(ctx("", "GET", "en.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo"), ("locale", "en")],
        &matcher.match_path(b"/foo/bar")?,
    );

    let matcher = get(routes()?, Some(ctx("", "GET", "en.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar"), ("locale", "en")],
        &matcher.match_path(b"/bar/bar")?,
    );
    Ok(())
}

pub fn testVariationInTrailingSlashWithHosts(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/foo/").host("foo.example.com")),
            ("bar", route("/foo").host("bar.example.com")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "foo.example.com")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(ctx("", "GET", "bar.example.com")));
    assert_equals(&[("_route", "bar")], &matcher.match_path(b"/foo")?);
    Ok(())
}

pub fn testVariationInTrailingSlashWithHostsInReverse(get: GetUrlMatcher) -> TestResult {
    // The order should not matter
    let routes = || {
        coll(vec![
            ("bar", route("/foo").host("bar.example.com")),
            ("foo", route("/foo/").host("foo.example.com")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "foo.example.com")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(ctx("", "GET", "bar.example.com")));
    assert_equals(&[("_route", "bar")], &matcher.match_path(b"/foo")?);
    Ok(())
}

pub fn testVariationInTrailingSlashWithHostsAndVariable(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/{foo}/").host("foo.example.com")),
            ("bar", route("/{foo}").host("bar.example.com")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "foo.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo")],
        &matcher.match_path(b"/bar/")?,
    );

    let matcher = get(routes()?, Some(ctx("", "GET", "bar.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    Ok(())
}

pub fn testVariationInTrailingSlashWithHostsAndVariableInReverse(get: GetUrlMatcher) -> TestResult {
    // The order should not matter
    let routes = || {
        coll(vec![
            ("bar", route("/{foo}").host("bar.example.com")),
            ("foo", route("/{foo}/").host("foo.example.com")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "foo.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo")],
        &matcher.match_path(b"/bar/")?,
    );

    let matcher = get(routes()?, Some(ctx("", "GET", "bar.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    Ok(())
}

pub fn testVariationInTrailingSlashWithMethods(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/foo/").methods(&["POST"])),
            ("bar", route("/foo").methods(&["GET"])),
        ])
    };
    let matcher = get(routes()?, Some(method("POST")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(method("GET")));
    assert_equals(&[("_route", "bar")], &matcher.match_path(b"/foo")?);
    Ok(())
}

pub fn testVariationInTrailingSlashWithMethodsInReverse(get: GetUrlMatcher) -> TestResult {
    // The order should not matter
    let routes = || {
        coll(vec![
            ("bar", route("/foo").methods(&["GET"])),
            ("foo", route("/foo/").methods(&["POST"])),
        ])
    };
    let matcher = get(routes()?, Some(method("POST")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(method("GET")));
    assert_equals(&[("_route", "bar")], &matcher.match_path(b"/foo")?);
    Ok(())
}

pub fn testVariableVariationInTrailingSlashWithMethods(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/{foo}/").methods(&["POST"])),
            ("bar", route("/{foo}").methods(&["GET"])),
        ])
    };
    let matcher = get(routes()?, Some(method("POST")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo")],
        &matcher.match_path(b"/bar/")?,
    );

    let matcher = get(routes()?, Some(method("GET")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    Ok(())
}

pub fn testVariableVariationInTrailingSlashWithMethodsInReverse(get: GetUrlMatcher) -> TestResult {
    // The order should not matter
    let routes = || {
        coll(vec![
            ("bar", route("/{foo}").methods(&["GET"])),
            ("foo", route("/{foo}/").methods(&["POST"])),
        ])
    };
    let matcher = get(routes()?, Some(method("POST")));
    assert_equals(
        &[("foo", "bar"), ("_route", "foo")],
        &matcher.match_path(b"/bar/")?,
    );

    let matcher = get(routes()?, Some(method("GET")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    Ok(())
}

pub fn testMixOfStaticAndVariableVariationInTrailingSlashWithHosts(
    get: GetUrlMatcher,
) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/foo/").host("foo.example.com")),
            ("bar", route("/{foo}").host("bar.example.com")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "foo.example.com")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(ctx("", "GET", "bar.example.com")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    Ok(())
}

pub fn testMixOfStaticAndVariableVariationInTrailingSlashWithMethods(
    get: GetUrlMatcher,
) -> TestResult {
    let routes = || {
        coll(vec![
            ("foo", route("/foo/").methods(&["POST"])),
            ("bar", route("/{foo}").methods(&["GET"])),
        ])
    };
    let matcher = get(routes()?, Some(method("POST")));
    assert_equals(&[("_route", "foo")], &matcher.match_path(b"/foo/")?);

    let matcher = get(routes()?, Some(method("GET")));
    assert_equals(
        &[("foo", "bar"), ("_route", "bar")],
        &matcher.match_path(b"/bar")?,
    );
    assert_equals(
        &[("foo", "foo"), ("_route", "bar")],
        &matcher.match_path(b"/foo")?,
    );
    Ok(())
}

pub fn testWithOutHostHostDoesNotMatch(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "foo",
        route("/foo/{foo}").host("{locale}.example.com"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "example.com")));
    expect_exception(matcher.match_path(b"/foo/bar"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testPathIsCaseSensitive(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "foo",
        route("/locale").requirements(&[("locale", "EN|FR|DE")]),
    )])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/en"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testHostIsCaseInsensitive(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "foo",
        route("/")
            .requirements(&[("locale", "EN|FR|DE")])
            .host("{locale}.example.com"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "en.example.com")));
    assert_equals(
        &[("_route", "foo"), ("locale", "en")],
        &matcher.match_path(b"/")?,
    );
    Ok(())
}

pub fn testNoConfiguration(get: GetUrlMatcher) -> TestResult {
    let matcher = get(RouteCollection::new(), None);
    expect_exception(matcher.match_path(b"/"), NO_CONFIGURATION);
    Ok(())
}

pub fn testNestedCollections(get: GetUrlMatcher) -> TestResult {
    let mut coll1 = RouteCollection::new();

    let mut sub_coll = coll(vec![
        ("a", route("/a")),
        ("b", route("/b")),
        ("c", route("/c")),
    ])?;
    sub_coll.add_prefix(b"/p", &PhpArray::new(), &PhpArray::new())?;
    coll1.add_collection(sub_coll);

    coll1.add("baz", route("/{baz}").build()?, 0);

    let mut sub_coll = coll(vec![("buz", route("/buz"))])?;
    sub_coll.add_prefix(b"/prefix", &PhpArray::new(), &PhpArray::new())?;
    coll1.add_collection(sub_coll);

    let matcher = get(coll1, None);
    assert_equals(&[("_route", "a")], &matcher.match_path(b"/p/a")?);
    assert_equals(
        &[("_route", "baz"), ("baz", "p")],
        &matcher.match_path(b"/p")?,
    );
    assert_equals(&[("_route", "buz")], &matcher.match_path(b"/prefix/buz")?);
    Ok(())
}

pub fn testSchemeAndMethodMismatch(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "foo",
        route("/").schemes(&["https"]).methods(&["POST"]),
    )])?;
    let matcher = get(coll, None);
    let e = expect_exception(matcher.match_path(b"/"), RESOURCE_NOT_FOUND);
    assert_eq!(e.message().as_bytes(), b"No routes found for \"/\".");
    Ok(())
}

pub fn testUnicodeRoute(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("a", route("/{a}").requirements(&[("a", ".")]).utf8(false)),
        ("b", route("/{a}").requirements(&[("a", ".")]).utf8(true)),
    ])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "b"), ("a", "é")],
        &matcher.match_path("/é".as_bytes())?,
    );
    Ok(())
}

pub fn testRequirementWithCapturingGroup(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/{a}/{b}").requirements(&[("a", "(a|b)")]),
    )])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "a"), ("a", "a"), ("b", "b")],
        &matcher.match_path(b"/a/b")?,
    );
    Ok(())
}

pub fn testDotAllWithCatchAll(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("a", route("/{id}.html").requirements(&[("id", ".+")])),
        ("b", route("/{all}").requirements(&[("all", ".+")])),
    ])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "a"), ("id", "foo/bar")],
        &matcher.match_path(b"/foo/bar.html")?,
    );
    Ok(())
}

pub fn testHostPattern(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/{app}/{action}/{unused}").host("{host}"),
    )])?;
    let expected = [
        ("_route", "a"),
        ("app", "an_app"),
        ("action", "an_action"),
        ("unused", "unused"),
        ("host", "foo"),
    ];
    let matcher = get(coll, Some(ctx("", "GET", "foo")));
    assert_equals(&expected, &matcher.match_path(b"/an_app/an_action/unused")?);
    Ok(())
}

pub fn testUtf8Prefix(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("a", route("/é{foo}").utf8(true)),
        ("b", route("/è{bar}").utf8(true)),
    ])?;
    let matcher = get(coll, None);
    let ret = matcher.match_path("/éo".as_bytes())?;
    assert_eq!(at(&ret, "_route"), Some(&Value::string("a")));
    Ok(())
}

pub fn testUtf8AndMethodMatching(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        (
            "a",
            route("/admin/api/list/{shortClassName}/{id}.{_format}")
                .utf8(true)
                .methods(&["PUT"]),
        ),
        (
            "b",
            route("/admin/api/package.{_format}").methods(&["POST"]),
        ),
        (
            "c",
            route("/admin/api/package.{_format}")
                .defaults(&[("_format", "json")])
                .methods(&["GET"]),
        ),
    ])?;
    let matcher = get(coll, None);
    let ret = matcher.match_path(b"/admin/api/package.json")?;
    assert_eq!(at(&ret, "_route"), Some(&Value::string("c")));
    Ok(())
}

pub fn testHostWithDot(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("a", route("/foo").host("foo.example.com")),
        ("b", route("/bar/{baz}")),
    ])?;
    let matcher = get(coll, None);
    let ret = matcher.match_path(b"/bar/abc.123")?;
    assert_eq!(at(&ret, "_route"), Some(&Value::string("b")));
    Ok(())
}

pub fn testHostVariableDoesNotMatchThePath(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![(
            "a",
            route("/admin/report/{id}").host("app.{domain}"),
        )])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "app.localhost")));
    assert_equals(
        &[("_route", "a"), ("domain", "localhost"), ("id", "1")],
        &matcher.match_path(b"/admin/report/1")?,
    );

    let matcher = get(routes()?, Some(ctx("", "GET", "app")));
    expect_exception(
        matcher.match_path(b"/x./admin/report/1"),
        RESOURCE_NOT_FOUND,
    );
    Ok(())
}

pub fn testHostVariableWithPermissiveRequirementDoesNotMatchThePath(
    get: GetUrlMatcher,
) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/admin/report/{id}")
            .requirements(&[("domain", ".+")])
            .host("app.{domain}"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "app.example.com")));
    assert_equals(
        &[("_route", "a"), ("domain", "example.com"), ("id", "1")],
        &matcher.match_path(b"/admin/report/1")?,
    );

    expect_exception(
        matcher.match_path(b"/x./admin/report/1"),
        RESOURCE_NOT_FOUND,
    );
    Ok(())
}

pub fn testHostVariableWithPermissiveRequirementMatchesTheHostOnly(
    get: GetUrlMatcher,
) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/{path}")
            .requirements(&[("domain", ".+"), ("path", ".+")])
            .host("app.{domain}"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "app.example.com")));
    assert_equals(
        &[
            ("_route", "a"),
            ("domain", "example.com"),
            ("path", "foo./bar"),
        ],
        &matcher.match_path(b"/foo./bar")?,
    );
    Ok(())
}

pub fn testHostVariableWithNamedGroupInRequirementDoesNotMatchThePath(
    get: GetUrlMatcher,
) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/admin/report/{id}")
            .requirements(&[("domain", "(?<name>.+)")])
            .host("app.{domain}"),
    )])?;
    let matcher = get(coll, Some(ctx("", "GET", "app.example.com")));
    let parameters = matcher.match_path(b"/admin/report/1")?;
    assert_eq!(at(&parameters, "_route"), Some(&Value::string("a")));
    assert_eq!(
        at(&parameters, "domain"),
        Some(&Value::string("example.com"))
    );

    expect_exception(
        matcher.match_path(b"/x./admin/report/1"),
        RESOURCE_NOT_FOUND,
    );
    Ok(())
}

pub fn testStaticRouteWithHostVariableDoesNotMatchThePath(get: GetUrlMatcher) -> TestResult {
    let routes = || {
        coll(vec![
            (
                "a",
                route("/admin/report/{id}")
                    .host("app.{domain}")
                    .methods(&["POST"]),
            ),
            ("b", route("/admin/report/latest").host("app.{domain}")),
        ])
    };
    let matcher = get(routes()?, Some(ctx("", "GET", "app.localhost")));
    assert_equals(
        &[("_route", "b"), ("domain", "localhost")],
        &matcher.match_path(b"/admin/report/latest")?,
    );

    let matcher = get(routes()?, Some(ctx("", "GET", "app")));
    expect_exception(
        matcher.match_path(b"/x./admin/report/latest"),
        RESOURCE_NOT_FOUND,
    );
    Ok(())
}

pub fn testSlashVariant(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/foo/{bar}").requirements(&[("bar", ".*")]),
    )])?;
    let matcher = get(coll, None);
    let ret = matcher.match_path(b"/foo/")?;
    assert_eq!(at(&ret, "_route"), Some(&Value::string("a")));
    Ok(())
}

pub fn testSlashVariant2(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/foo/{bär}/")
            .requirements(&[("bär", ".*")])
            .utf8(true),
    )])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "a"), ("bär", "bar")],
        &matcher.match_path(b"/foo/bar/")?,
    );
    Ok(())
}

pub fn testSlashWithVerb(get: GetUrlMatcher) -> TestResult {
    let coll1 = coll(vec![
        ("a", route("/{foo}").methods(&["put", "delete"])),
        ("b", route("/bar/")),
    ])?;
    let matcher = get(coll1, None);
    assert_same(&[("_route", "b")], &matcher.match_path(b"/bar/")?);

    let coll2 = coll(vec![(
        "a",
        route("/dav/{foo}")
            .requirements(&[("foo", ".*")])
            .methods(&["GET", "OPTIONS"]),
    )])?;
    let matcher = get(coll2, Some(method("OPTIONS")));
    let expected = [("_route", "a"), ("foo", "files/bar/")];
    assert_equals(&expected, &matcher.match_path(b"/dav/files/bar/")?);
    Ok(())
}

pub fn testSlashAndVerbPrecedence(get: GetUrlMatcher) -> TestResult {
    let coll1 = coll(vec![
        (
            "a",
            route("/api/customers/{customerId}/contactpersons/").methods(&["post"]),
        ),
        (
            "b",
            route("/api/customers/{customerId}/contactpersons").methods(&["get"]),
        ),
    ])?;
    let matcher = get(coll1, None);
    let expected = [("_route", "b"), ("customerId", "123")];
    assert_equals(
        &expected,
        &matcher.match_path(b"/api/customers/123/contactpersons")?,
    );

    let coll2 = coll(vec![
        (
            "a",
            route("/api/customers/{customerId}/contactpersons/").methods(&["get"]),
        ),
        (
            "b",
            route("/api/customers/{customerId}/contactpersons").methods(&["post"]),
        ),
    ])?;
    let matcher = get(coll2, Some(method("POST")));
    let expected = [("_route", "b"), ("customerId", "123")];
    assert_equals(
        &expected,
        &matcher.match_path(b"/api/customers/123/contactpersons")?,
    );
    Ok(())
}

pub fn testGreedyTrailingRequirement(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![("a", route("/{a}").requirements(&[("a", ".+")]))])?;
    let matcher = get(coll, None);

    assert_equals(
        &[("_route", "a"), ("a", "foo")],
        &matcher.match_path(b"/foo")?,
    );
    assert_equals(
        &[("_route", "a"), ("a", "foo/")],
        &matcher.match_path(b"/foo/")?,
    );
    Ok(())
}

pub fn testTrailingRequirementWithDefault(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        (
            "a",
            route("/fr-fr/{a}")
                .defaults(&[("a", "aaa")])
                .requirements(&[("a", ".+")]),
        ),
        (
            "b",
            route("/en-en/{b}")
                .defaults(&[("b", "bbb")])
                .requirements(&[("b", ".*")]),
        ),
    ])?;
    let matcher = get(coll, None);

    assert_equals(
        &[("_route", "a"), ("a", "aaa")],
        &matcher.match_path(b"/fr-fr")?,
    );
    assert_equals(
        &[("_route", "a"), ("a", "AAA")],
        &matcher.match_path(b"/fr-fr/AAA")?,
    );
    assert_equals(
        &[("_route", "b"), ("b", "bbb")],
        &matcher.match_path(b"/en-en")?,
    );
    assert_equals(
        &[("_route", "b"), ("b", "BBB")],
        &matcher.match_path(b"/en-en/BBB")?,
    );
    Ok(())
}

pub fn testTrailingRequirementWithDefaultA(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "a",
        route("/fr-fr/{a}")
            .defaults(&[("a", "aaa")])
            .requirements(&[("a", ".+")]),
    )])?;
    let matcher = get(coll, None);
    expect_exception(matcher.match_path(b"/fr-fr/"), RESOURCE_NOT_FOUND);
    Ok(())
}

pub fn testTrailingRequirementWithDefaultB(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![(
        "b",
        route("/en-en/{b}")
            .defaults(&[("b", "bbb")])
            .requirements(&[("b", ".*")]),
    )])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "b"), ("b", "")],
        &matcher.match_path(b"/en-en/")?,
    );
    Ok(())
}

pub fn testRestrictiveTrailingRequirementWithStaticRouteAfter(get: GetUrlMatcher) -> TestResult {
    let coll = coll(vec![
        ("a", route("/hello{_}").requirements(&[("_", "/(?!/)")])),
        ("b", route("/hello")),
    ])?;
    let matcher = get(coll, None);
    assert_equals(
        &[("_route", "a"), ("_", "/")],
        &matcher.match_path(b"/hello/")?,
    );
    Ok(())
}

pub fn testUtf8VarName(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![("foo", route("/foo/{bär}/{bäz?foo}").utf8(true))])?;
    let matcher = get(collection, None);
    assert_equals(
        &[("_route", "foo"), ("bär", "baz"), ("bäz", "foo")],
        &matcher.match_path(b"/foo/baz")?,
    );
    Ok(())
}

pub fn testParameterWithRequirementWithDefault(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "test",
        route("/test/{foo}")
            .defaults(&[("foo", "foo-")])
            .requirements(&[("foo", "\\w+")]),
    )])?;
    let matcher = get(collection, None);

    let result = matcher.match_path(b"/test/foo")?;
    assert_eq!(at(&result, "_route"), Some(&Value::string("test")));
    assert_eq!(at(&result, "foo"), Some(&Value::string("foo")));

    if let Err(e) = matcher.match_path(b"/test/foo-") {
        if e.is_a(RESOURCE_NOT_FOUND) {
            let message = String::from_utf8_lossy(e.message().as_bytes()).into_owned();
            assert!(message.contains("No routes found"), "{message}");
        } else {
            return Err(Box::new(e));
        }
    }

    let result = matcher.match_path(b"/test")?;
    assert_eq!(at(&result, "_route"), Some(&Value::string("test")));
    assert_eq!(at(&result, "foo"), Some(&Value::string("foo-")));
    Ok(())
}

pub fn testMapping(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![("a", route("/conference/{slug:conference}"))])?;
    let matcher = get(collection, None);

    let mut expected = strings(&[("_route", "a"), ("slug", "vienna-2024")]);
    expected.insert(
        Key::from_bytes(b"_route_mapping"),
        Value::Array(strings(&[("slug", "conference")])),
    );
    assert_equals_array(&expected, &matcher.match_path(b"/conference/vienna-2024")?);
    Ok(())
}

pub fn testMappingwithAlias(get: GetUrlMatcher) -> TestResult {
    let collection = coll(vec![(
        "a",
        route("/conference/{conferenceSlug:conference.slug}"),
    )])?;
    let matcher = get(collection, None);

    let mut expected = strings(&[("_route", "a"), ("conferenceSlug", "vienna-2024")]);
    let alias = PhpArray::list([Value::string("conference"), Value::string("slug")]);
    expected.insert(
        Key::from_bytes(b"_route_mapping"),
        Value::Array(
            [("conferenceSlug", Value::Array(alias))]
                .into_iter()
                .collect(),
        ),
    );
    assert_equals_array(&expected, &matcher.match_path(b"/conference/vienna-2024")?);
    Ok(())
}
