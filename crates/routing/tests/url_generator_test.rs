//! UrlGeneratorTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's; a data provider's cases run in a loop. The test classes
//! with `__toString()` or public properties are PhpObject impls here, and
//! the logger mock counts `error()` calls.
//!
//! Upstream: symfony/routing Tests/Generator/UrlGeneratorTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::{Arc, Mutex, OnceLock};

use common::{TestResult, s, strings, values};
use php_value::{
    PhpArray, PhpObject, PhpString, Value,
    object::{GenericObject, StdClass, StringableObject},
};
use psr_log::{Context, LogLevel, LoggerInterface};
use symfony_deprecation_contracts::collect;
use symfony_routing::{
    exception::Error,
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_generator::{
        ConfigurableRequirementsInterface, ReferenceType, UrlGenerator, UrlGeneratorInterface,
    },
};

const INVALID_PARAMETER: &str = "Symfony\\Component\\Routing\\Exception\\InvalidParameterException";
const MISSING_PARAMETERS: &str =
    "Symfony\\Component\\Routing\\Exception\\MissingMandatoryParametersException";
const ROUTE_NOT_FOUND: &str = "Symfony\\Component\\Routing\\Exception\\RouteNotFoundException";
const CIRCULAR: &str = "Symfony\\Component\\Routing\\Exception\\RouteCircularReferenceException";

use ReferenceType::{AbsolutePath, AbsoluteUrl, NetworkPath, RelativePath};

/// `getGenerator($routes, $parameters, $logger, $defaultLocale)`: the
/// context is `new RequestContext('/app.php')`, then `$setup` calls its
/// setters.
fn generator_with(
    routes: RouteCollection,
    setup: impl FnOnce(&mut RequestContext),
    logger: Option<Arc<dyn LoggerInterface>>,
    default_locale: Option<&str>,
) -> UrlGenerator {
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    setup(&mut context);
    UrlGenerator::new(routes, context, logger, default_locale.map(PhpString::from))
}

fn generator(routes: RouteCollection) -> UrlGenerator {
    generator_with(routes, |_| {}, None, None)
}

/// `getRoutes($name, $route)`.
fn routes(name: &str, route: Route) -> RouteCollection {
    let mut routes = RouteCollection::new();
    routes.add(name, route, 0);
    routes
}

/// `new Route($path, $defaults, $requirements)`.
fn route(path: &str, defaults: PhpArray, requirements: PhpArray) -> Result<Route, Error> {
    Route::with(
        path,
        Arguments {
            defaults,
            requirements,
            ..Arguments::default()
        },
    )
}

fn none() -> PhpArray {
    PhpArray::new()
}

fn generate(
    generator: &UrlGenerator,
    name: &str,
    parameters: PhpArray,
    reference_type: ReferenceType,
) -> Result<String, Error> {
    let url = generator.generate(name.as_bytes(), parameters, reference_type)?;
    Ok(url.to_string_lossy().into_owned())
}

/// `expectException($class)` on a call.
fn throws(result: Result<String, Error>, class: &str) -> Error {
    match result {
        Ok(url) => panic!("expected {class}, generated {url:?}"),
        Err(e) => {
            assert!(e.is_a(class), "expected {class}, got {e}");
            e
        }
    }
}

#[test]
fn testAbsoluteUrlWithPort80() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    let url = generate(&generator(routes), "test", none(), AbsoluteUrl)?;
    assert_eq!(url, "http://localhost/app.php/testing");
    Ok(())
}

#[test]
fn testAbsoluteSecureUrlWithPort443() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    let generator = generator_with(
        routes,
        |c| {
            c.set_scheme("https");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "https://localhost/app.php/testing"
    );
    Ok(())
}

#[test]
fn testAbsoluteUrlWithNonStandardPort() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    let generator = generator_with(
        routes,
        |c| {
            c.set_http_port(8080);
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "http://localhost:8080/app.php/testing"
    );
    Ok(())
}

#[test]
fn testAbsoluteSecureUrlWithNonStandardPort() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    let generator = generator_with(
        routes,
        |c| {
            c.set_https_port(8080).set_scheme("https");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "https://localhost:8080/app.php/testing"
    );
    Ok(())
}

#[test]
fn testRelativeUrlWithoutParameters() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    assert_eq!(
        generate(&generator(routes), "test", none(), AbsolutePath)?,
        "/app.php/testing"
    );
    Ok(())
}

#[test]
fn testRelativeUrlWithParameter() -> TestResult {
    let routes = routes("test", Route::new("/testing/{foo}")?);
    let url = generate(
        &generator(routes),
        "test",
        strings(&[("foo", "bar")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/testing/bar");
    Ok(())
}

#[test]
fn testRelativeUrlWithNullParameter() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing.{format}",
            values(vec![("format", Value::Null)]),
            none(),
        )?,
    );
    assert_eq!(
        generate(&generator(routes), "test", none(), AbsolutePath)?,
        "/app.php/testing"
    );
    Ok(())
}

#[test]
fn testRelativeUrlWithNullParameterButNotOptional() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing/{foo}/bar",
            values(vec![("foo", Value::Null)]),
            none(),
        )?,
    );
    // This must raise an exception because the default requirement for "foo" is "[^/]+" which is not met with these params.
    // Generating path "/testing//bar" would be wrong as matching this route would fail.
    throws(
        generate(&generator(routes), "test", none(), AbsolutePath),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testRelativeUrlWithOptionalZeroParameter() -> TestResult {
    let routes = routes("test", Route::new("/testing/{page}")?);
    let url = generate(
        &generator(routes),
        "test",
        values(vec![("page", Value::Int(0))]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/testing/0");
    Ok(())
}

#[test]
fn testNotPassedOptionalParameterInBetween() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/{slug}/{page}",
            values(vec![("slug", "index".into()), ("page", Value::Int(0))]),
            none(),
        )?,
    );
    let generator = generator(routes);
    assert_eq!(
        generate(
            &generator,
            "test",
            values(vec![("page", Value::Int(1))]),
            AbsolutePath
        )?,
        "/app.php/index/1"
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/"
    );
    Ok(())
}

/// `StringableObjectWithPublicProperty`: `$foo = 'property'` and
/// `__toString()` giving `'bar'`.
#[derive(Debug)]
struct StringableObjectWithPublicProperty;

impl PhpObject for StringableObjectWithPublicProperty {
    fn class(&self) -> &str {
        "Symfony\\Component\\Routing\\Tests\\Generator\\StringableObjectWithPublicProperty"
    }

    fn properties(&self) -> PhpArray {
        strings(&[("foo", "property")])
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(s("bar"))
    }
}

fn stringable_object() -> Value {
    StringableObject::object(
        "Symfony\\Component\\Routing\\Tests\\Generator\\StringableObject",
        "bar",
    )
}

fn object(o: impl PhpObject) -> Value {
    Value::Object(Arc::new(o))
}

/// `valuesProvider()`.
fn values_provider() -> Vec<(&'static str, &'static str, &'static str, Value)> {
    let std_class = StdClass::object(strings(&[("baz", "bar")]));
    let nested_std_class = StdClass::object(values(vec![("nested", std_class.clone())]));
    let shared_leaf = StdClass::object(strings(&[("baz", "bar")]));
    let shared_object = StdClass::object(values(vec![
        ("left", shared_leaf.clone()),
        ("right", shared_leaf),
    ]));
    let non_stringable = |properties| {
        object(GenericObject::new(
            "Symfony\\Component\\Routing\\Tests\\Generator\\NonStringableObject",
            properties,
        ))
    };
    vec![
        ("null", "", "foo", Value::Null),
        ("string", "?foo=bar", "foo", "bar".into()),
        ("boolean-false", "?foo=0", "foo", Value::Bool(false)),
        ("boolean-true", "?foo=1", "foo", Value::Bool(true)),
        (
            "object implementing __toString()",
            "?foo=bar",
            "foo",
            stringable_object(),
        ),
        (
            "object implementing __toString() but has public property",
            "?foo%5Bfoo%5D=property",
            "foo",
            object(StringableObjectWithPublicProperty),
        ),
        (
            "object implementing __toString() in nested array",
            "?foo%5Bbaz%5D=bar",
            "foo",
            Value::Array(values(vec![("baz", stringable_object())])),
        ),
        (
            "object implementing __toString() in nested array but has public property",
            "?foo%5Bbaz%5D%5Bfoo%5D=property",
            "foo",
            Value::Array(values(vec![(
                "baz",
                object(StringableObjectWithPublicProperty),
            )])),
        ),
        ("stdClass", "?foo%5Bbaz%5D=bar", "foo", std_class),
        (
            "stdClass in nested stdClass",
            "?foo%5Bnested%5D%5Bbaz%5D=bar",
            "foo",
            nested_std_class,
        ),
        ("non stringable object", "", "foo", non_stringable(none())),
        (
            "non stringable object but has public property",
            "?foo%5Bfoo%5D=property",
            "foo",
            non_stringable(strings(&[("foo", "property")])),
        ),
        ("numeric key", "?123=foo", "123", "foo".into()),
        // a shared (acyclic) reference must not be mistaken for a circular one
        (
            "object with a shared acyclic reference",
            "?foo%5Bleft%5D%5Bbaz%5D=bar&foo%5Bright%5D%5Bbaz%5D=bar",
            "foo",
            shared_object,
        ),
    ]
}

#[test]
fn testRelativeUrlWithExtraParameters() -> TestResult {
    for (case, expected_query_string, parameter, value) in values_provider() {
        let routes = routes("test", Route::new("/testing")?);
        let url = generate(
            &generator(routes),
            "test",
            values(vec![(parameter, value)]),
            AbsolutePath,
        )?;
        assert_eq!(
            url,
            format!("/app.php/testing{expected_query_string}"),
            "{case}"
        );
    }
    Ok(())
}

#[test]
fn testAbsoluteUrlWithExtraParameters() -> TestResult {
    for (case, expected_query_string, parameter, value) in values_provider() {
        let routes = routes("test", Route::new("/testing")?);
        let url = generate(
            &generator(routes),
            "test",
            values(vec![(parameter, value)]),
            AbsoluteUrl,
        )?;
        assert_eq!(
            url,
            format!("http://localhost/app.php/testing{expected_query_string}"),
            "{case}"
        );
    }
    Ok(())
}

/// A `stdClass` whose properties are set after it is created, so two
/// can point at each other.
#[derive(Debug, Default)]
struct Node(OnceLock<PhpArray>);

impl PhpObject for Node {
    fn class(&self) -> &str {
        "stdClass"
    }

    fn properties(&self) -> PhpArray {
        self.0.get().cloned().unwrap_or_default()
    }
}

#[test]
fn testGenerateWithCircularObjectReference() -> TestResult {
    let a = Arc::new(Node::default());
    let b = Arc::new(Node::default());
    let _ = a.0.set(values(vec![("b", Value::Object(b.clone()))]));
    let _ = b.0.set(values(vec![("a", Value::Object(a.clone()))]));

    let routes = routes("test", Route::new("/testing")?);

    let e = throws(
        generate(
            &generator(routes),
            "test",
            values(vec![("foo", Value::Object(a.clone()))]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    assert!(
        e.message()
            .as_bytes()
            .starts_with(b"Parameters for route \"test\" cannot contain a circular reference")
    );
    Ok(())
}

#[test]
fn testUrlWithExtraParametersFromGlobals() -> TestResult {
    let routes = routes("test", Route::new("/testing")?);
    let mut generator = generator(routes);
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    context.set_parameter(b"bar", "bar".into());
    generator.set_context(context);
    let url = generate(&generator, "test", strings(&[("foo", "bar")]), AbsolutePath)?;
    assert_eq!(url, "/app.php/testing?foo=bar");
    Ok(())
}

#[test]
fn testUrlWithGlobalParameter() -> TestResult {
    let routes = routes("test", Route::new("/testing/{foo}")?);
    let mut generator = generator(routes);
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    context.set_parameter(b"foo", "bar".into());
    generator.set_context(context);
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/testing/bar"
    );
    Ok(())
}

#[test]
fn testGlobalParameterHasHigherPriorityThanDefault() -> TestResult {
    let routes = routes(
        "test",
        route("/{_locale}", strings(&[("_locale", "en")]), none())?,
    );
    let mut generator = generator(routes);
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    context.set_parameter(b"_locale", "de".into());
    generator.set_context(context);
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/de"
    );
    Ok(())
}

/// The `hr` (`/foo`) and `en` (`/bar`) routes of the locale tests.
fn localized_routes() -> Result<RouteCollection, Error> {
    let mut routes = RouteCollection::new();
    let route = Route::new("")?;
    let name = "test";
    for (locale, path) in [("hr", "/foo"), ("en", "/bar")] {
        let mut localized_route = route.clone();
        localized_route.set_default(b"_locale", locale.into())?;
        localized_route.set_requirement(b"_locale", locale.as_bytes())?;
        localized_route.set_default(b"_canonical_route", name.into())?;
        localized_route.set_path(path)?;
        routes.add(format!("{name}.{locale}"), localized_route, 0);
    }
    Ok(routes)
}

#[test]
fn testGenerateWithDefaultLocale() -> TestResult {
    let generator = generator_with(localized_routes()?, |_| {}, None, Some("hr"));
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "http://localhost/app.php/foo"
    );
    Ok(())
}

#[test]
fn testGenerateWithOverriddenParameterLocale() -> TestResult {
    let generator = generator_with(localized_routes()?, |_| {}, None, Some("hr"));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("_locale", "en")]),
            AbsoluteUrl
        )?,
        "http://localhost/app.php/bar"
    );
    Ok(())
}

#[test]
fn testGenerateWithOverriddenParameterLocaleFromRequestContext() -> TestResult {
    let mut generator = generator_with(localized_routes()?, |_| {}, None, Some("hr"));
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    context.set_parameter(b"_locale", "en".into());
    generator.set_context(context);
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "http://localhost/app.php/bar"
    );
    Ok(())
}

/// `(new Route($path))->setDefault('_locale', $locale)
/// ->setDefault('_canonical_route', $name)->setRequirement('_locale', $locale)`.
fn canonical(path: &str, locale: &str, name: &str) -> Result<Route, Error> {
    let mut route = Route::new(path)?;
    route
        .set_default(b"_locale", locale.into())?
        .set_default(b"_canonical_route", name.into())?
        .set_requirement(b"_locale", locale.as_bytes())?;
    Ok(route)
}

#[test]
fn testDumpWithLocalizedRoutesPreserveTheGoodLocaleInTheUrl() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("foo.en", canonical("/{_locale}/fork", "en", "foo")?, 0);
    route_collection.add(
        "foo.fr",
        canonical("/{_locale}/fourchette", "fr", "foo")?,
        0,
    );
    route_collection.add("fun.en", canonical("/fun", "en", "fun")?, 0);
    route_collection.add("fun.fr", canonical("/amusant", "fr", "fun")?, 0);

    let mut url_generator = generator(route_collection);
    url_generator
        .context_mut()
        .set_parameter(b"_locale", "fr".into());
    let g = |name: &str, parameters| generate(&url_generator, name, parameters, AbsolutePath);
    let en = || strings(&[("_locale", "en")]);

    assert_eq!(g("foo", none())?, "/app.php/fr/fourchette");
    assert_eq!(g("foo.en", none())?, "/app.php/en/fork");
    assert_eq!(g("foo", en())?, "/app.php/en/fork");
    assert_eq!(g("foo.fr", en())?, "/app.php/fr/fourchette");

    assert_eq!(g("fun", none())?, "/app.php/amusant");
    assert_eq!(g("fun.en", none())?, "/app.php/fun");
    assert_eq!(g("fun", en())?, "/app.php/fun");
    assert_eq!(g("fun.fr", en())?, "/app.php/amusant");
    Ok(())
}

#[test]
fn testLocalizedAliasRouteGeneratesCorrectUrlPerLocale() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("foo.en", canonical("/en/fork", "en", "foo")?, 0);
    route_collection.add("foo.fr", canonical("/fr/fourchette", "fr", "foo")?, 0);
    route_collection.add_alias(b"bar.en", b"foo.en")?;
    route_collection.add_alias(b"bar.fr", b"foo.fr")?;

    let mut url_generator = generator(route_collection);
    url_generator
        .context_mut()
        .set_parameter(b"_locale", "fr".into());

    assert_eq!(
        generate(&url_generator, "bar", none(), AbsolutePath)?,
        "/app.php/fr/fourchette"
    );
    assert_eq!(
        generate(
            &url_generator,
            "bar",
            strings(&[("_locale", "en")]),
            AbsolutePath
        )?,
        "/app.php/en/fork"
    );
    assert_eq!(
        generate(
            &url_generator,
            "bar",
            strings(&[("_locale", "fr")]),
            AbsolutePath
        )?,
        "/app.php/fr/fourchette"
    );
    Ok(())
}

#[test]
fn testGenerateWithoutRoutes() -> TestResult {
    let routes = routes("foo", Route::new("/testing/{foo}")?);
    throws(
        generate(&generator(routes), "test", none(), AbsoluteUrl),
        ROUTE_NOT_FOUND,
    );
    Ok(())
}

#[test]
fn testGenerateWithInvalidLocale() -> TestResult {
    let generator = generator_with(localized_routes()?, |_| {}, None, Some("fr"));
    throws(
        generate(&generator, "test", none(), AbsolutePath),
        ROUTE_NOT_FOUND,
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithoutMandatoryParameter() -> TestResult {
    let routes = routes("test", Route::new("/testing/{foo}")?);
    let e = throws(
        generate(&generator(routes), "test", none(), AbsoluteUrl),
        MISSING_PARAMETERS,
    );
    assert_eq!(
        e.message(),
        &s("Some mandatory parameters are missing (\"foo\") to generate a URL for route \"test\".")
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidOptionalParameter() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing/{foo}",
            strings(&[("foo", "1")]),
            strings(&[("foo", "d+")]),
        )?,
    );
    throws(
        generate(
            &generator(routes),
            "test",
            strings(&[("foo", "bar")]),
            AbsoluteUrl,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidParameter() -> TestResult {
    let routes = routes(
        "test",
        route("/testing/{foo}", none(), strings(&[("foo", "1|2")]))?,
    );
    throws(
        generate(
            &generator(routes),
            "test",
            strings(&[("foo", "0")]),
            AbsoluteUrl,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

/// GHSA-72xp-p242-47p9: `#^en|fr|vi|de$#` would match `vi` inside
/// `/evil.com`; the requirement is anchored as a whole.
#[test]
fn testGenerateForRouteWithAlternationRequirementRejectsSubstringMatch() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/{_locale}/blog",
            none(),
            strings(&[("_locale", "en|fr|vi|de")]),
        )?,
    );
    throws(
        generate(
            &generator(routes),
            "test",
            strings(&[("_locale", "/evil.com")]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidOptionalParameterNonStrict() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing/{foo}",
            strings(&[("foo", "1")]),
            strings(&[("foo", "d+")]),
        )?,
    );
    let mut generator = generator(routes);
    generator.set_strict_requirements(Some(false));
    assert_eq!(
        generate(&generator, "test", strings(&[("foo", "bar")]), AbsoluteUrl)?,
        ""
    );
    Ok(())
}

/// `createMock(LoggerInterface::class)`, counting `error()` calls.
#[derive(Default)]
struct Logger(Mutex<Vec<(LogLevel, String, Context)>>);

impl LoggerInterface for Logger {
    fn log(&self, level: LogLevel, message: &str, context: Context) {
        if let Ok(mut records) = self.0.lock() {
            records.push((level, message.to_owned(), context));
        }
    }
}

#[test]
fn testGenerateForRouteWithInvalidOptionalParameterNonStrictWithLogger() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing/{foo}",
            strings(&[("foo", "1")]),
            strings(&[("foo", "d+")]),
        )?,
    );
    let logger = Arc::new(Logger::default());
    let mut generator = generator_with(routes, |_| {}, Some(logger.clone()), None);
    generator.set_strict_requirements(Some(false));
    assert_eq!(
        generate(&generator, "test", strings(&[("foo", "bar")]), AbsoluteUrl)?,
        ""
    );
    let records = logger.0.lock().map_err(|e| e.to_string())?;
    let errors: Vec<_> = records.iter().filter(|r| r.0 == LogLevel::Error).collect();
    assert_eq!(errors.len(), 1);
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidParameterButDisabledRequirementsCheck() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/testing/{foo}",
            strings(&[("foo", "1")]),
            strings(&[("foo", "d+")]),
        )?,
    );
    let mut generator = generator(routes);
    generator.set_strict_requirements(None);
    assert_eq!(
        generate(&generator, "test", strings(&[("foo", "bar")]), AbsolutePath)?,
        "/app.php/testing/bar"
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidMandatoryParameter() -> TestResult {
    let routes = routes(
        "test",
        route("/testing/{foo}", none(), strings(&[("foo", "d+")]))?,
    );
    throws(
        generate(
            &generator(routes),
            "test",
            strings(&[("foo", "bar")]),
            AbsoluteUrl,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testGenerateForRouteWithInvalidUtf8Parameter() -> TestResult {
    let route = Route::with(
        "/testing/{foo}",
        Arguments {
            requirements: strings(&[("foo", "\\pL+")]),
            options: values(vec![("utf8", Value::Bool(true))]),
            ..Arguments::default()
        },
    )?;
    throws(
        generate(
            &generator(routes("test", route)),
            "test",
            strings(&[("foo", "abc123")]),
            AbsoluteUrl,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testRequiredParamAndEmptyPassed() -> TestResult {
    let routes = routes(
        "test",
        route("/{slug}", none(), strings(&[("slug", ".+")]))?,
    );
    throws(
        generate(
            &generator(routes),
            "test",
            strings(&[("slug", "")]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

/// `new Route($path, [], [], [], $host, $schemes)`.
fn host_route(
    path: &str,
    requirements: PhpArray,
    host: &str,
    schemes: &[&str],
) -> Result<Route, Error> {
    Route::with(
        path,
        Arguments {
            requirements,
            host: s(host),
            schemes: schemes.iter().map(|x| s(x)).collect(),
            ..Arguments::default()
        },
    )
}

#[test]
fn testSchemeRequirementDoesNothingIfSameCurrentScheme() -> TestResult {
    let routes1 = routes("test", host_route("/", none(), "", &["http"])?);
    assert_eq!(
        generate(&generator(routes1), "test", none(), AbsolutePath)?,
        "/app.php/"
    );

    let routes2 = routes("test", host_route("/", none(), "", &["https"])?);
    let generator = generator_with(
        routes2,
        |c| {
            c.set_scheme("https");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/"
    );
    Ok(())
}

#[test]
fn testSchemeRequirementForcesAbsoluteUrl() -> TestResult {
    let routes1 = routes("test", host_route("/", none(), "", &["https"])?);
    assert_eq!(
        generate(&generator(routes1), "test", none(), AbsolutePath)?,
        "https://localhost/app.php/"
    );

    let routes2 = routes("test", host_route("/", none(), "", &["http"])?);
    let generator = generator_with(
        routes2,
        |c| {
            c.set_scheme("https");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "http://localhost/app.php/"
    );
    Ok(())
}

#[test]
fn testSchemeRequirementCreatesUrlForFirstRequiredScheme() -> TestResult {
    let routes = routes("test", host_route("/", none(), "", &["Ftp", "https"])?);
    assert_eq!(
        generate(&generator(routes), "test", none(), AbsolutePath)?,
        "ftp://localhost/app.php/"
    );
    Ok(())
}

#[test]
fn testPathWithTwoStartingSlashes() -> TestResult {
    let routes = routes("test", Route::new("//path-and-not-domain")?);
    // this must not generate '//path-and-not-domain' because that would be a network path
    let generator = generator_with(
        routes,
        |c| {
            c.set_base_url("");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/path-and-not-domain"
    );
    Ok(())
}

#[test]
fn testNoTrailingSlashForMultipleOptionalParameters() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/category/{slug1}/{slug2}/{slug3}",
            values(vec![("slug2", Value::Null), ("slug3", Value::Null)]),
            none(),
        )?,
    );
    assert_eq!(
        generate(
            &generator(routes),
            "test",
            strings(&[("slug1", "foo")]),
            AbsolutePath
        )?,
        "/app.php/category/foo"
    );
    Ok(())
}

#[test]
fn testWithAnIntegerAsADefaultValue() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/{default}",
            values(vec![("default", Value::Int(0))]),
            none(),
        )?,
    );
    assert_eq!(
        generate(
            &generator(routes),
            "test",
            strings(&[("default", "foo")]),
            AbsolutePath
        )?,
        "/app.php/foo"
    );
    Ok(())
}

#[test]
fn testNullForOptionalParameterIsIgnored() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/test/{default}",
            values(vec![("default", Value::Int(0))]),
            none(),
        )?,
    );
    assert_eq!(
        generate(
            &generator(routes),
            "test",
            values(vec![("default", Value::Null)]),
            AbsolutePath
        )?,
        "/app.php/test"
    );
    Ok(())
}

#[test]
fn testQueryParamSameAsDefault() -> TestResult {
    let routes = routes(
        "test",
        route("/test", values(vec![("page", Value::Int(1))]), none())?,
    );
    let generator = generator(routes);
    let g = |parameters| generate(&generator, "test", parameters, AbsolutePath);

    assert_eq!(
        g(values(vec![("page", Value::Int(2))]))?,
        "/app.php/test?page=2"
    );
    assert_eq!(g(values(vec![("page", Value::Int(1))]))?, "/app.php/test");
    assert_eq!(g(strings(&[("page", "1")]))?, "/app.php/test");
    assert_eq!(g(none())?, "/app.php/test");
    Ok(())
}

#[test]
fn testArrayQueryParamSameAsDefault() -> TestResult {
    let list = |items: &[&str]| Value::Array(PhpArray::list(items.iter().map(|x| Value::from(*x))));
    let routes = routes(
        "test",
        route(
            "/test",
            values(vec![("array", list(&["foo", "bar"]))]),
            none(),
        )?,
    );
    let generator = generator(routes);
    let g = |parameters| generate(&generator, "test", parameters, AbsolutePath);

    assert_eq!(
        g(values(vec![("array", list(&["bar", "foo"]))]))?,
        "/app.php/test?array%5B0%5D=bar&array%5B1%5D=foo"
    );
    assert_eq!(
        g(values(vec![(
            "array",
            Value::Array(strings(&[("a", "foo"), ("b", "bar")]))
        )]))?,
        "/app.php/test?array%5Ba%5D=foo&array%5Bb%5D=bar"
    );
    assert_eq!(
        g(values(vec![("array", list(&["foo", "bar"]))]))?,
        "/app.php/test"
    );
    let reordered: PhpArray = [(1i64, Value::from("bar")), (0, Value::from("foo"))]
        .into_iter()
        .collect();
    assert_eq!(
        g(values(vec![("array", Value::Array(reordered))]))?,
        "/app.php/test"
    );
    assert_eq!(g(none())?, "/app.php/test");
    Ok(())
}

#[test]
fn testGenerateWithSpecialRouteName() -> TestResult {
    let routes = routes("$péß^a|", Route::new("/bar")?);
    assert_eq!(
        generate(&generator(routes), "$péß^a|", none(), AbsolutePath)?,
        "/app.php/bar"
    );
    Ok(())
}

#[test]
fn testUrlEncoding() -> TestResult {
    let expected_path = concat!(
        "/app.php/@:%5B%5D/%28%29*%27%22%20+,;-._~%26%24%3C%3E|%7B%7D%25%5C%5E%60!%3Ffoo=bar%23id",
        "/@:%5B%5D/%28%29*%27%22%20+,;-._~%26%24%3C%3E|%7B%7D%25%5C%5E%60!%3Ffoo=bar%23id",
        "?query=@:%5B%5D/%28%29*%27%22%20%2B,;-._~%26%24%3C%3E%7C%7B%7D%25%5C%5E%60!?foo%3Dbar%23id",
    );

    // This tests the encoding of reserved characters that are used for delimiting of URI components (defined in RFC 3986)
    // and other special ASCII chars. These chars are tested as static text path, variable path and query param.
    let chars = "@:[]/()*'\" +,;-._~&$<>|{}%\\^`!?foo=bar#id";
    let routes = routes(
        "test",
        route(
            &format!("/{chars}/{{varpath}}"),
            none(),
            strings(&[("varpath", ".+")]),
        )?,
    );
    let url = generate(
        &generator(routes),
        "test",
        strings(&[("varpath", chars), ("query", chars)]),
        AbsolutePath,
    )?;
    assert_eq!(url, expected_path);
    Ok(())
}

#[test]
fn testEncodingOfRelativePathSegments() -> TestResult {
    let g = |path: &str| -> Result<String, Error> {
        generate(
            &generator(routes("test", Route::new(path)?)),
            "test",
            none(),
            AbsolutePath,
        )
    };
    assert_eq!(g("/dir/../dir/..")?, "/app.php/dir/%2E%2E/dir/%2E%2E");
    assert_eq!(g("/dir/./dir/.")?, "/app.php/dir/%2E/dir/%2E");
    assert_eq!(g("/a./.a/a../..a/...")?, "/app.php/a./.a/a../..a/...");
    Ok(())
}

#[test]
fn testEncodingOfChainedRelativePathSegments() -> TestResult {
    let generator1 = generator(routes(
        "test",
        route("/foo/{path}/bar", none(), strings(&[("path", ".+")]))?,
    ));
    let g = |generator: &UrlGenerator, path: &str| {
        generate(generator, "test", strings(&[("path", path)]), AbsolutePath)
    };
    assert_eq!(
        g(&generator1, "../../..")?,
        "/app.php/foo/%2E%2E/%2E%2E/%2E%2E/bar"
    );
    assert_eq!(g(&generator1, "././.")?, "/app.php/foo/%2E/%2E/%2E/bar");
    assert_eq!(
        g(&generator1, "../././..")?,
        "/app.php/foo/%2E%2E/%2E/%2E/%2E%2E/bar"
    );

    let generator2 = generator(routes(
        "test",
        route("/foo/{path}", none(), strings(&[("path", ".+")]))?,
    ));
    assert_eq!(
        g(&generator2, "../../..")?,
        "/app.php/foo/%2E%2E/%2E%2E/%2E%2E"
    );
    assert_eq!(g(&generator2, "././.")?, "/app.php/foo/%2E/%2E/%2E");
    Ok(())
}

#[test]
fn testEncodingOfSlashInPath() -> TestResult {
    let routes = routes(
        "test",
        route("/dir/{path}/dir2", none(), strings(&[("path", ".+")]))?,
    );
    let url = generate(
        &generator(routes),
        "test",
        strings(&[("path", "foo/bar%2Fbaz")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/dir/foo/bar%2Fbaz/dir2");
    Ok(())
}

#[test]
fn testEncodingOfSlashInQueryParameters() -> TestResult {
    let generator = generator(routes("test", Route::new("/get")?));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("query", "foo/bar")]),
            AbsolutePath
        )?,
        "/app.php/get?query=foo/bar"
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("query", "foo%2Fbar")]),
            AbsolutePath
        )?,
        "/app.php/get?query=foo%2Fbar"
    );
    Ok(())
}

#[test]
fn testAdjacentVariables() -> TestResult {
    let routes = routes(
        "test",
        route(
            "/{x}{y}{z}.{_format}",
            strings(&[("z", "default-z"), ("_format", "html")]),
            strings(&[("y", "\\d+")]),
        )?,
    );
    let generator = generator(routes);
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("x", "foo"), ("y", "123")]),
            AbsolutePath
        )?,
        "/app.php/foo123"
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("x", "foo"), ("y", "123"), ("z", "bar"), ("_format", "xml")]),
            AbsolutePath
        )?,
        "/app.php/foo123bar.xml"
    );

    // The default requirement for 'x' should not allow the separator '.' in this case because it would otherwise match everything
    // and following optional variables like _format could never match.
    throws(
        generate(
            &generator,
            "test",
            strings(&[
                ("x", "do.t"),
                ("y", "123"),
                ("z", "bar"),
                ("_format", "xml"),
            ]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testOptionalVariableWithNoRealSeparator() -> TestResult {
    let generator = generator(routes(
        "test",
        route("/get{what}", strings(&[("what", "All")]), none())?,
    ));
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/get"
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("what", "Sites")]),
            AbsolutePath
        )?,
        "/app.php/getSites"
    );
    Ok(())
}

#[test]
fn testRequiredVariableWithNoRealSeparator() -> TestResult {
    let generator = generator(routes("test", Route::new("/get{what}Suffix")?));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("what", "Sites")]),
            AbsolutePath
        )?,
        "/app.php/getSitesSuffix"
    );
    Ok(())
}

#[test]
fn testDefaultRequirementOfVariable() -> TestResult {
    let generator = generator(routes("test", Route::new("/{page}.{_format}")?));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("page", "index"), ("_format", "mobile.html")]),
            AbsolutePath
        )?,
        "/app.php/index.mobile.html"
    );
    Ok(())
}

#[test]
fn testImportantVariable() -> TestResult {
    let mut route = Route::new("/{page}.{!_format}")?;
    route.add_defaults(&strings(&[("_format", "mobile.html")]))?;
    let generator = generator(routes("test", route));

    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("page", "index"), ("_format", "xml")]),
            AbsolutePath
        )?,
        "/app.php/index.xml"
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("page", "index"), ("_format", "mobile.html")]),
            AbsolutePath
        )?,
        "/app.php/index.mobile.html"
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("page", "index")]),
            AbsolutePath
        )?,
        "/app.php/index.mobile.html"
    );
    Ok(())
}

#[test]
fn testImportantVariableWithNoDefault() -> TestResult {
    let generator = generator(routes("test", Route::new("/{page}.{!_format}")?));
    let e = throws(
        generate(
            &generator,
            "test",
            strings(&[("page", "index")]),
            AbsolutePath,
        ),
        MISSING_PARAMETERS,
    );
    assert_eq!(
        e.message(),
        &s(
            "Some mandatory parameters are missing (\"_format\") to generate a URL for route \"test\"."
        )
    );
    Ok(())
}

#[test]
fn testDefaultRequirementOfVariableDisallowsSlash() -> TestResult {
    let generator = generator(routes("test", Route::new("/{page}.{_format}")?));
    throws(
        generate(
            &generator,
            "test",
            strings(&[("page", "index"), ("_format", "sl/ash")]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testDefaultRequirementOfVariableDisallowsNextSeparator() -> TestResult {
    let generator = generator(routes("test", Route::new("/{page}.{_format}")?));
    throws(
        generate(
            &generator,
            "test",
            strings(&[("page", "do.t"), ("_format", "html")]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testWithHostDifferentFromContext() -> TestResult {
    let generator = generator(routes(
        "test",
        host_route("/{name}", none(), "{locale}.example.com", &[])?,
    ));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("name", "Fabien"), ("locale", "fr")]),
            AbsolutePath
        )?,
        "//fr.example.com/app.php/Fabien"
    );
    Ok(())
}

#[test]
fn testWithHostSameAsContext() -> TestResult {
    let routes = routes(
        "test",
        host_route("/{name}", none(), "{locale}.example.com", &[])?,
    );
    let generator = generator_with(
        routes,
        |c| {
            c.set_host("fr.example.com");
        },
        None,
        None,
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("name", "Fabien"), ("locale", "fr")]),
            AbsolutePath
        )?,
        "/app.php/Fabien"
    );
    Ok(())
}

#[test]
fn testWithHostSameAsContextAndAbsolute() -> TestResult {
    let routes = routes(
        "test",
        host_route("/{name}", none(), "{locale}.example.com", &[])?,
    );
    let generator = generator_with(
        routes,
        |c| {
            c.set_host("fr.example.com");
        },
        None,
        None,
    );
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("name", "Fabien"), ("locale", "fr")]),
            AbsoluteUrl
        )?,
        "http://fr.example.com/app.php/Fabien"
    );
    Ok(())
}

#[test]
fn testUrlWithInvalidParameterInHost() -> TestResult {
    let generator = generator(routes(
        "test",
        host_route("/", strings(&[("foo", "bar")]), "{foo}.example.com", &[])?,
    ));
    throws(
        generate(&generator, "test", strings(&[("foo", "baz")]), AbsolutePath),
        INVALID_PARAMETER,
    );
    Ok(())
}

/// `new Route('/', ['foo' => $default], ['foo' => 'bar'], [], '{foo}.example.com')`.
fn host_route_with_default(default: &str) -> Result<Route, Error> {
    Route::with(
        "/",
        Arguments {
            defaults: strings(&[("foo", default)]),
            requirements: strings(&[("foo", "bar")]),
            host: s("{foo}.example.com"),
            ..Arguments::default()
        },
    )
}

#[test]
fn testUrlWithInvalidParameterInHostWhenParamHasADefaultValue() -> TestResult {
    let generator = generator(routes("test", host_route_with_default("bar")?));
    throws(
        generate(&generator, "test", strings(&[("foo", "baz")]), AbsolutePath),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testUrlWithInvalidParameterEqualsDefaultValueInHost() -> TestResult {
    let generator = generator(routes("test", host_route_with_default("baz")?));
    throws(
        generate(&generator, "test", strings(&[("foo", "baz")]), AbsolutePath),
        INVALID_PARAMETER,
    );
    Ok(())
}

#[test]
fn testUrlWithInvalidParameterInHostInNonStrictMode() -> TestResult {
    let mut generator = generator(routes(
        "test",
        host_route("/", strings(&[("foo", "bar")]), "{foo}.example.com", &[])?,
    ));
    generator.set_strict_requirements(Some(false));
    assert_eq!(
        generate(&generator, "test", strings(&[("foo", "baz")]), AbsolutePath)?,
        ""
    );
    Ok(())
}

#[test]
fn testHostIsCaseInsensitive() -> TestResult {
    let generator = generator(routes(
        "test",
        host_route(
            "/",
            strings(&[("locale", "en|de|fr")]),
            "{locale}.FooBar.com",
            &[],
        )?,
    ));
    assert_eq!(
        generate(
            &generator,
            "test",
            strings(&[("locale", "EN")]),
            NetworkPath
        )?,
        "//EN.FooBar.com/app.php/"
    );
    Ok(())
}

/// `new Route('/path', ['domain' => 'my.fallback.host'], ['domain' => '.+'], [], '{domain}')`.
fn fallback_host_route() -> Result<Route, Error> {
    Route::with(
        "/path",
        Arguments {
            defaults: strings(&[("domain", "my.fallback.host")]),
            requirements: strings(&[("domain", ".+")]),
            host: s("{domain}"),
            ..Arguments::default()
        },
    )
}

#[test]
fn testDefaultHostIsUsedWhenContextHostIsEmpty() -> TestResult {
    let mut generator = generator(routes("test", fallback_host_route()?));
    generator.context_mut().set_host("");
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "http://my.fallback.host/app.php/path"
    );
    Ok(())
}

#[test]
fn testDefaultHostIsUsedWhenContextHostIsEmptyAndPathReferenceType() -> TestResult {
    let mut generator = generator(routes("test", fallback_host_route()?));
    generator.context_mut().set_host("");
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "//my.fallback.host/app.php/path"
    );
    Ok(())
}

#[test]
fn testAbsoluteUrlFallbackToPathIfHostIsEmptyAndSchemeIsHttp() -> TestResult {
    let mut generator = generator(routes("test", Route::new("/route")?));
    generator.context_mut().set_host("");
    generator.context_mut().set_scheme("https");
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "/app.php/route"
    );
    Ok(())
}

#[test]
fn testAbsoluteUrlFallbackToNetworkIfSchemeIsEmptyAndHostIsNot() -> TestResult {
    let mut generator = generator(routes("test", Route::new("/path")?));
    generator.context_mut().set_host("example.com");
    generator.context_mut().set_scheme("");
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "//example.com/app.php/path"
    );
    Ok(())
}

#[test]
fn testAbsoluteUrlFallbackToPathIfSchemeAndHostAreEmpty() -> TestResult {
    let mut generator = generator(routes("test", Route::new("/path")?));
    generator.context_mut().set_host("");
    generator.context_mut().set_scheme("");
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "/app.php/path"
    );
    Ok(())
}

#[test]
fn testAbsoluteUrlWithNonHttpSchemeAndEmptyHost() -> TestResult {
    let mut generator = generator(routes("test", host_route("/path", none(), "", &["file"])?));
    generator.context_mut().set_base_url("");
    generator.context_mut().set_host("");
    assert_eq!(
        generate(&generator, "test", none(), AbsoluteUrl)?,
        "file:///path"
    );
    Ok(())
}

#[test]
fn testGenerateNetworkPath() -> TestResult {
    let network_routes = || {
        routes_from(
            "test",
            host_route("/{name}", none(), "{locale}.example.com", &["http"]),
        )
    };
    let parameters = || strings(&[("name", "Fabien"), ("locale", "fr")]);

    assert_eq!(
        generate(
            &generator(network_routes()?),
            "test",
            parameters(),
            NetworkPath
        )?,
        "//fr.example.com/app.php/Fabien",
        "network path with different host"
    );
    let same_host = generator_with(
        network_routes()?,
        |c| {
            c.set_host("fr.example.com");
        },
        None,
        None,
    );
    assert_eq!(
        generate(
            &same_host,
            "test",
            strings(&[("name", "Fabien"), ("locale", "fr"), ("query", "string")]),
            NetworkPath
        )?,
        "//fr.example.com/app.php/Fabien?query=string",
        "network path although host same as context"
    );
    let https = generator_with(
        network_routes()?,
        |c| {
            c.set_scheme("https");
        },
        None,
        None,
    );
    assert_eq!(
        generate(&https, "test", parameters(), NetworkPath)?,
        "http://fr.example.com/app.php/Fabien",
        "absolute URL because scheme requirement does not match context"
    );
    assert_eq!(
        generate(
            &generator(network_routes()?),
            "test",
            parameters(),
            AbsoluteUrl
        )?,
        "http://fr.example.com/app.php/Fabien",
        "absolute URL with same scheme because it is requested"
    );
    Ok(())
}

/// `routes(...)` with a `Result<Route>` built in place.
fn routes_from(name: &str, route: Result<Route, Error>) -> Result<RouteCollection, Error> {
    Ok(routes(name, route?))
}

#[test]
fn testGenerateRelativePath() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add("article", Route::new("/{author}/{article}/")?, 0);
    routes.add("comments", Route::new("/{author}/{article}/comments")?, 0);
    routes.add(
        "host",
        host_route("/{article}", none(), "{author}.example.com", &[])?,
        0,
    );
    routes.add(
        "scheme",
        host_route("/{author}/blog", none(), "", &["https"])?,
        0,
    );
    routes.add("unrelated", Route::new("/about")?, 0);

    let generator = generator_with(
        routes,
        |c| {
            c.set_host("example.com")
                .set_path_info("/fabien/symfony-is-great/");
        },
        None,
        None,
    );
    let g = |name: &str, parameters| generate(&generator, name, parameters, RelativePath);

    assert_eq!(
        g(
            "comments",
            strings(&[("author", "fabien"), ("article", "symfony-is-great")])
        )?,
        "comments"
    );
    assert_eq!(
        g(
            "comments",
            values(vec![
                ("author", "fabien".into()),
                ("article", "symfony-is-great".into()),
                ("page", Value::Int(2))
            ])
        )?,
        "comments?page=2"
    );
    assert_eq!(
        g(
            "article",
            strings(&[("author", "fabien"), ("article", "twig-is-great")])
        )?,
        "../twig-is-great/"
    );
    assert_eq!(
        g(
            "article",
            strings(&[("author", "bernhard"), ("article", "forms-are-great")])
        )?,
        "../../bernhard/forms-are-great/"
    );
    assert_eq!(
        g(
            "host",
            strings(&[("author", "bernhard"), ("article", "forms-are-great")])
        )?,
        "//bernhard.example.com/app.php/forms-are-great"
    );
    assert_eq!(
        g("scheme", strings(&[("author", "bernhard")]))?,
        "https://example.com/app.php/bernhard/blog"
    );
    assert_eq!(g("unrelated", none())?, "../../about");
    Ok(())
}

#[test]
fn testAliases() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add("a", Route::new("/foo")?, 0);
    routes.add_alias(b"b", b"a")?;
    routes.add_alias(b"c", b"b")?;

    let generator = generator(routes);

    assert_eq!(
        generate(&generator, "b", none(), AbsolutePath)?,
        "/app.php/foo"
    );
    assert_eq!(
        generate(&generator, "c", none(), AbsolutePath)?,
        "/app.php/foo"
    );
    Ok(())
}

#[test]
fn testAliasWhichTargetRouteDoesntExist() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add_alias(b"d", b"non-existent")?;
    throws(
        generate(&generator(routes), "d", none(), AbsolutePath),
        ROUTE_NOT_FOUND,
    );
    Ok(())
}

/// The messages of the deprecations `f` triggers.
fn deprecations(f: impl FnOnce() -> Result<String, Error>) -> Result<Vec<String>, Error> {
    let (result, deprecations) = collect(f);
    result?;
    Ok(deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect())
}

#[test]
fn testDeprecatedAlias() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add("a", Route::new("/foo")?, 0);
    routes
        .add_alias(b"b", b"a")?
        .set_deprecated("foo/bar", "1.0.0", "")?;

    let generator = generator(routes);
    assert_eq!(
        deprecations(|| generate(&generator, "b", none(), AbsolutePath))?,
        [
            "Since foo/bar 1.0.0: The \"b\" route alias is deprecated. You should stop using it, as it will be removed in the future."
        ]
    );
    Ok(())
}

#[test]
fn testDeprecatedAliasWithCustomMessage() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add("a", Route::new("/foo")?, 0);
    routes
        .add_alias(b"b", b"a")?
        .set_deprecated("foo/bar", "1.0.0", "foo %alias_id%.")?;

    let generator = generator(routes);
    assert_eq!(
        deprecations(|| generate(&generator, "b", none(), AbsolutePath))?,
        ["Since foo/bar 1.0.0: foo b."]
    );
    Ok(())
}

#[test]
fn testTargettingADeprecatedAliasShouldTriggerDeprecation() -> TestResult {
    let mut routes = RouteCollection::new();
    routes.add("a", Route::new("/foo")?, 0);
    routes
        .add_alias(b"b", b"a")?
        .set_deprecated("foo/bar", "1.0.0", "foo %alias_id%.")?;
    routes.add_alias(b"c", b"b")?;

    let generator = generator(routes);
    assert_eq!(
        deprecations(|| generate(&generator, "c", none(), AbsolutePath))?,
        ["Since foo/bar 1.0.0: foo b."]
    );
    Ok(())
}

fn circular(aliases: &[(&str, &str)], name: &str, message: &str) -> TestResult {
    let mut routes = RouteCollection::new();
    for (alias, target) in aliases {
        routes.add_alias(alias.as_bytes(), target.as_bytes())?;
    }
    let e = throws(
        generate(&generator(routes), name, none(), AbsolutePath),
        CIRCULAR,
    );
    assert_eq!(e.message(), &s(message));
    Ok(())
}

#[test]
fn testCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "a")],
        "b",
        "Circular reference detected for route \"b\", path: \"b -> a -> b\".",
    )
}

#[test]
fn testDeepCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "c"), ("c", "b")],
        "b",
        "Circular reference detected for route \"b\", path: \"b -> c -> b\".",
    )
}

#[test]
fn testIndirectCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "c"), ("c", "a")],
        "a",
        "Circular reference detected for route \"a\", path: \"a -> b -> c -> a\".",
    )
}

#[test]
fn testGetRelativePath() {
    // provideRelativePaths()
    let cases = [
        ("/same/dir/", "/same/dir/", ""),
        ("/same/file", "/same/file", ""),
        ("/", "/file", "file"),
        ("/", "/dir/file", "dir/file"),
        (
            "/dir/file.html",
            "/dir/different-file.html",
            "different-file.html",
        ),
        ("/same/dir/extra-file", "/same/dir/", "./"),
        ("/parent/dir/", "/parent/", "../"),
        ("/parent/dir/extra-file", "/parent/", "../"),
        ("/a/b/", "/x/y/z/", "../../x/y/z/"),
        ("/a/b/c/d/e", "/a/c/d", "../../../c/d"),
        ("/a/b/c//", "/a/b/c/", "../"),
        ("/a/b/c/", "/a/b/c//", ".//"),
        ("/root/a/b/c/", "/root/x/b/c/", "../../../x/b/c/"),
        ("/a/b/c/d/", "/a", "../../../../a"),
        (
            "/special-chars/sp%20ce/1€/mäh/e=mc²",
            "/special-chars/sp%20ce/1€/<µ>/e=mc²",
            "../<µ>/e=mc²",
        ),
        ("not-rooted", "dir/file", "dir/file"),
        ("//dir/", "", "../../"),
        ("/dir/", "/dir/file:with-colon", "./file:with-colon"),
        (
            "/dir/",
            "/dir/subdir/file:with-colon",
            "subdir/file:with-colon",
        ),
        ("/dir/", "/dir/:subdir/", "./:subdir/"),
    ];
    for (source_path, target_path, expected_path) in cases {
        let got = UrlGenerator::get_relative_path(source_path.as_bytes(), target_path.as_bytes());
        assert_eq!(
            String::from_utf8_lossy(&got),
            expected_path,
            "{source_path} -> {target_path}"
        );
    }
}

#[test]
fn testFragmentsCanBeAppendedToUrls() -> TestResult {
    let generator = generator(routes("test", Route::new("/testing")?));

    let url = generate(
        &generator,
        "test",
        strings(&[("_fragment", "frag ment")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/testing#frag%20ment");

    let url = generate(
        &generator,
        "test",
        strings(&[("_fragment", "0")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/testing#0");
    Ok(())
}

#[test]
fn testFragmentsDoNotEscapeValidCharacters() -> TestResult {
    let generator = generator(routes("test", Route::new("/testing")?));
    let url = generate(
        &generator,
        "test",
        strings(&[("_fragment", "?/")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/testing#?/");
    Ok(())
}

#[test]
fn testFragmentsCanBeDefinedAsDefaults() -> TestResult {
    let generator = generator(routes_from(
        "test",
        route("/testing", strings(&[("_fragment", "fragment")]), none()),
    )?);
    assert_eq!(
        generate(&generator, "test", none(), AbsolutePath)?,
        "/app.php/testing#fragment"
    );
    Ok(())
}

#[test]
fn testLookRoundRequirementsInPath() -> TestResult {
    // provideLookAroundRequirementsInPath()
    let cases = [
        (
            "/app.php/a/b/b%28ar/c/d/e",
            "/{foo}/b(ar/{baz}",
            ".+(?=/b\\(ar/)",
        ),
        ("/app.php/a/b/bar/c/d/e", "/{foo}/bar/{baz}", ".+(?!$)"),
        (
            "/app.php/bar/a/b/bam/c/d/e",
            "/bar/{foo}/bam/{baz}",
            "(?<=/bar/).+",
        ),
        (
            "/app.php/bar/a/b/bam/c/d/e",
            "/bar/{foo}/bam/{baz}",
            "(?<!^).+",
        ),
    ];
    for (expected, path, requirement) in cases {
        let routes = routes(
            "test",
            route(
                path,
                none(),
                strings(&[("foo", requirement), ("baz", ".+?")]),
            )?,
        );
        let url = generate(
            &generator(routes),
            "test",
            strings(&[("foo", "a/b"), ("baz", "c/d/e")]),
            AbsolutePath,
        )?;
        assert_eq!(url, expected, "{requirement}");
    }
    Ok(())
}

#[test]
fn testUtf8VarName() -> TestResult {
    let route = Route::with(
        "/foo/{bär}",
        Arguments {
            options: values(vec![("utf8", Value::Bool(true))]),
            ..Arguments::default()
        },
    )?;
    let url = generate(
        &generator(routes("test", route)),
        "test",
        strings(&[("bär", "baz")]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/foo/baz");
    Ok(())
}

#[test]
fn testQueryParameters() -> TestResult {
    let generator = generator(routes("user", Route::new("/user/{username}")?));
    let url = generate(
        &generator,
        "user",
        values(vec![
            ("username", "john".into()),
            ("a", "foo".into()),
            ("b", "bar".into()),
            ("c", "baz".into()),
            (
                "_query",
                Value::Array(strings(&[("a", "123"), ("d", "789")])),
            ),
        ]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/user/john?a=123&b=bar&c=baz&d=789");
    Ok(())
}

#[test]
fn testRouteHostParameterAndQueryParameterWithSameName() -> TestResult {
    let route = host_route(
        "/admin/stats",
        strings(&[("domain", ".+")]),
        "{siteCode}.{domain}",
        &[],
    )?;
    let generator = generator(routes("admin_stats", route));
    let url = generate(
        &generator,
        "admin_stats",
        values(vec![
            ("siteCode", "fr".into()),
            ("domain", "example.com".into()),
            ("_query", Value::Array(strings(&[("siteCode", "us")]))),
        ]),
        NetworkPath,
    )?;
    assert_eq!(url, "//fr.example.com/app.php/admin/stats?siteCode=us");
    Ok(())
}

#[test]
fn testRoutePathParameterAndQueryParameterWithSameName() -> TestResult {
    let generator = generator(routes("user", Route::new("/user/{id}")?));
    let url = generate(
        &generator,
        "user",
        values(vec![
            ("id", "123".into()),
            ("_query", Value::Array(strings(&[("id", "456")]))),
        ]),
        AbsolutePath,
    )?;
    assert_eq!(url, "/app.php/user/123?id=456");
    Ok(())
}

#[test]
fn testQueryParameterCannotSubstituteRouteParameter() -> TestResult {
    let generator = generator(routes("user", Route::new("/user/{id}")?));
    let e = throws(
        generate(
            &generator,
            "user",
            values(vec![("_query", Value::Array(strings(&[("id", "456")])))]),
            AbsolutePath,
        ),
        MISSING_PARAMETERS,
    );
    assert_eq!(
        e.message(),
        &s("Some mandatory parameters are missing (\"id\") to generate a URL for route \"user\".")
    );
    Ok(())
}

#[test]
fn testQueryParametersWithScalarValue() -> TestResult {
    let generator = generator(routes("user", Route::new("/user/{id}")?));
    throws(
        generate(
            &generator,
            "user",
            values(vec![("id", "123".into()), ("_query", "foo".into())]),
            AbsolutePath,
        ),
        INVALID_PARAMETER,
    );
    Ok(())
}
