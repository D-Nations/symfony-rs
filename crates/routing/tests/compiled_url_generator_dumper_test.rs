//! CompiledUrlGeneratorDumperTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's. Upstream writes `dump()` to a temporary PHP file and
//! `require`s it; here `compiled()` gives the same array (owner, November
//! 24: the dump is data until it becomes a build-time emitter), and
//! `dump()` alone is `compiled()` for its exceptions and deprecations.
//! A context PHP changes after handing it over is changed through
//! `context_mut()`.
//!
//! Upstream: symfony/routing Tests/Generator/Dumper/CompiledUrlGeneratorDumperTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, s, strings};
use php_value::{PhpArray, PhpString};
use symfony_deprecation_contracts::collect;
use symfony_routing::{
    compiled_url_generator::{CompiledRoutes, CompiledUrlGenerator},
    compiled_url_generator_dumper::CompiledUrlGeneratorDumper,
    exception::Error,
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_generator::{ReferenceType, UrlGeneratorInterface},
};

use ReferenceType::{AbsolutePath, AbsoluteUrl};

const ROUTE_NOT_FOUND: &str = "Symfony\\Component\\Routing\\Exception\\RouteNotFoundException";
const CIRCULAR: &str = "Symfony\\Component\\Routing\\Exception\\RouteCircularReferenceException";

/// `require` of the file `dump()` wrote.
fn dumped(route_collection: RouteCollection) -> Result<CompiledRoutes, Error> {
    CompiledUrlGeneratorDumper::new(route_collection).compiled()
}

/// `new RequestContext('/app.php')`.
fn app_context() -> RequestContext {
    let mut context = RequestContext::default();
    context.set_base_url("/app.php");
    context
}

fn generator(
    compiled: CompiledRoutes,
    context: RequestContext,
    default_locale: Option<&str>,
) -> CompiledUrlGenerator {
    CompiledUrlGenerator::new(compiled, context, None, default_locale.map(PhpString::from))
}

fn generate(
    generator: &CompiledUrlGenerator,
    name: &str,
    parameters: PhpArray,
    reference_type: ReferenceType,
) -> Result<String, Error> {
    let url = generator.generate(name.as_bytes(), parameters, reference_type)?;
    Ok(url.to_string_lossy().into_owned())
}

fn path(generator: &CompiledUrlGenerator, name: &str) -> Result<String, Error> {
    generate(generator, name, PhpArray::new(), AbsolutePath)
}

/// `expectException($class)`.
fn throws<T: std::fmt::Debug>(result: Result<T, Error>, class: &str) -> Error {
    match result {
        Ok(value) => panic!("expected {class}, got {value:?}"),
        Err(e) => {
            assert!(e.is_a(class), "expected {class}, got {e}");
            e
        }
    }
}

/// `(new Route($path))->setDefault('_locale', $locale)
/// ->setDefault('_canonical_route', $name)->setRequirement('_locale', $locale)`.
fn localized(path: &str, locale: &str, name: &str) -> Result<Route, Error> {
    let mut route = Route::new(path)?;
    route
        .set_default(b"_locale", locale.into())?
        .set_default(b"_canonical_route", name.into())?
        .set_requirement(b"_locale", locale.as_bytes())?;
    Ok(route)
}

#[test]
fn testDumpWithRoutes() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("Test", Route::new("/testing/{foo}")?, 0);
    route_collection.add("Test2", Route::new("/testing2")?, 0);

    let project_url_generator = generator(dumped(route_collection)?, app_context(), None);

    let foo = || strings(&[("foo", "bar")]);
    let absolute_url_with_parameter = generate(&project_url_generator, "Test", foo(), AbsoluteUrl)?;
    let absolute_url_without_parameter = generate(
        &project_url_generator,
        "Test2",
        PhpArray::new(),
        AbsoluteUrl,
    )?;
    let relative_url_with_parameter =
        generate(&project_url_generator, "Test", foo(), AbsolutePath)?;
    let relative_url_without_parameter = generate(
        &project_url_generator,
        "Test2",
        PhpArray::new(),
        AbsolutePath,
    )?;

    assert_eq!(
        absolute_url_with_parameter,
        "http://localhost/app.php/testing/bar"
    );
    assert_eq!(
        absolute_url_without_parameter,
        "http://localhost/app.php/testing2"
    );
    assert_eq!(relative_url_with_parameter, "/app.php/testing/bar");
    assert_eq!(relative_url_without_parameter, "/app.php/testing2");
    Ok(())
}

#[test]
fn testDumpWithUnsupportedCharactersInRouteNames() -> TestResult {
    let route_name = "route'with/special-char";
    let alias_name = "alias'with/special-char";
    let mut route_collection = RouteCollection::new();
    route_collection.add(route_name, Route::new("/route")?, 0);
    route_collection.add_alias(alias_name.as_bytes(), route_name.as_bytes())?;

    let project_url_generator = generator(dumped(route_collection)?, app_context(), None);

    assert_eq!(path(&project_url_generator, route_name)?, "/app.php/route");
    assert_eq!(path(&project_url_generator, alias_name)?, "/app.php/route");
    Ok(())
}

#[test]
fn testDumpWithSimpleLocalizedRoutes() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("test", Route::new("/foo")?, 0);
    route_collection.add("test.en", localized("/testing/is/fun", "en", "test")?, 0);
    route_collection.add("test.nl", localized("/testen/is/leuk", "nl", "test")?, 0);

    let mut project_url_generator = generator(dumped(route_collection)?, app_context(), Some("en"));

    let url_with_default_locale = path(&project_url_generator, "test")?;
    let url_with_specified_locale = generate(
        &project_url_generator,
        "test",
        strings(&[("_locale", "nl")]),
        AbsolutePath,
    )?;
    project_url_generator
        .context_mut()
        .set_parameter(b"_locale", "en".into());
    let url_with_english_context = path(&project_url_generator, "test")?;
    project_url_generator
        .context_mut()
        .set_parameter(b"_locale", "nl".into());
    let url_with_dutch_context = path(&project_url_generator, "test")?;

    assert_eq!(url_with_default_locale, "/app.php/testing/is/fun");
    assert_eq!(url_with_specified_locale, "/app.php/testen/is/leuk");
    assert_eq!(url_with_english_context, "/app.php/testing/is/fun");
    assert_eq!(url_with_dutch_context, "/app.php/testen/is/leuk");

    // test with full route name
    assert_eq!(
        path(&project_url_generator, "test.en")?,
        "/app.php/testing/is/fun"
    );

    project_url_generator
        .context_mut()
        .set_parameter(b"_locale", "de_DE".into());
    // test that it fall backs to another route when there is no matching localized route
    assert_eq!(path(&project_url_generator, "test")?, "/app.php/foo");
    Ok(())
}

#[test]
fn testDumpWithRouteNotFoundLocalizedRoutes() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("test.en", localized("/testing/is/fun", "en", "test")?, 0);

    let project_url_generator = generator(dumped(route_collection)?, app_context(), Some("pl_PL"));

    let e = throws(path(&project_url_generator, "test"), ROUTE_NOT_FOUND);
    assert_eq!(
        e.message(),
        &s("Unable to generate a URL for the named route \"test\" as such route does not exist.")
    );
    Ok(())
}

#[test]
fn testDumpWithFallbackLocaleLocalizedRoutes() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("test.en", localized("/testing/is/fun", "en", "test")?, 0);
    route_collection.add("test.nl", localized("/testen/is/leuk", "nl", "test")?, 0);
    route_collection.add(
        "test.fr",
        localized("/tester/est/amusant", "fr", "test")?,
        0,
    );
    let compiled = dumped(route_collection)?;

    let mut context = app_context();
    context.set_parameter(b"_locale", "en_GB".into());
    let project_url_generator = generator(compiled.clone(), context, None);

    // test with context _locale
    assert_eq!(
        path(&project_url_generator, "test")?,
        "/app.php/testing/is/fun"
    );
    // test with parameters _locale
    assert_eq!(
        generate(
            &project_url_generator,
            "test",
            strings(&[("_locale", "nl_BE")]),
            AbsolutePath
        )?,
        "/app.php/testen/is/leuk"
    );

    let project_url_generator = generator(compiled, app_context(), Some("fr_CA"));
    // test with default locale
    assert_eq!(
        path(&project_url_generator, "test")?,
        "/app.php/tester/est/amusant"
    );
    Ok(())
}

#[test]
fn testDumpWithTooManyRoutes() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("Test", Route::new("/testing/{foo}")?, 0);
    for i in 0..32769 {
        route_collection.add(format!("route_{i}"), Route::new(format!("/route_{i}"))?, 0);
    }
    route_collection.add("Test2", Route::new("/testing2")?, 0);

    let project_url_generator = generator(dumped(route_collection)?, app_context(), None);

    let foo = || strings(&[("foo", "bar")]);
    let absolute_url_with_parameter = generate(&project_url_generator, "Test", foo(), AbsoluteUrl)?;
    let absolute_url_without_parameter = generate(
        &project_url_generator,
        "Test2",
        PhpArray::new(),
        AbsoluteUrl,
    )?;
    let relative_url_with_parameter =
        generate(&project_url_generator, "Test", foo(), AbsolutePath)?;
    let relative_url_without_parameter = generate(
        &project_url_generator,
        "Test2",
        PhpArray::new(),
        AbsolutePath,
    )?;

    assert_eq!(
        absolute_url_with_parameter,
        "http://localhost/app.php/testing/bar"
    );
    assert_eq!(
        absolute_url_without_parameter,
        "http://localhost/app.php/testing2"
    );
    assert_eq!(relative_url_with_parameter, "/app.php/testing/bar");
    assert_eq!(relative_url_without_parameter, "/app.php/testing2");
    Ok(())
}

#[test]
fn testDumpWithoutRoutes() -> TestResult {
    let project_url_generator = generator(dumped(RouteCollection::new())?, app_context(), None);
    throws(
        path(&project_url_generator, "Test"),
        "InvalidArgumentException",
    );
    Ok(())
}

#[test]
fn testGenerateNonExistingRoute() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("Test", Route::new("/test")?, 0);

    let project_url_generator =
        generator(dumped(route_collection)?, RequestContext::default(), None);
    throws(path(&project_url_generator, "NonExisting"), ROUTE_NOT_FOUND);
    Ok(())
}

#[test]
fn testDumpForRouteWithDefaults() -> TestResult {
    let mut route_collection = RouteCollection::new();
    let route = Route::with(
        "/testing/{foo}",
        Arguments {
            defaults: strings(&[("foo", "bar")]),
            ..Arguments::default()
        },
    )?;
    route_collection.add("Test", route, 0);

    let project_url_generator =
        generator(dumped(route_collection)?, RequestContext::default(), None);
    assert_eq!(path(&project_url_generator, "Test")?, "/testing");
    Ok(())
}

#[test]
fn testDumpWithSchemeRequirement() -> TestResult {
    let mut route_collection = RouteCollection::new();
    let route = Route::with(
        "/testing",
        Arguments {
            schemes: vec![s("ftp"), s("https")],
            ..Arguments::default()
        },
    )?;
    route_collection.add("Test1", route, 0);
    let compiled = dumped(route_collection)?;

    let project_url_generator = generator(compiled.clone(), app_context(), None);

    let absolute_url = generate(
        &project_url_generator,
        "Test1",
        PhpArray::new(),
        AbsoluteUrl,
    )?;
    let relative_url = generate(
        &project_url_generator,
        "Test1",
        PhpArray::new(),
        AbsolutePath,
    )?;

    assert_eq!(absolute_url, "ftp://localhost/app.php/testing");
    assert_eq!(relative_url, "ftp://localhost/app.php/testing");

    let context = RequestContext::new(
        "/app.php",
        "GET",
        "localhost",
        "https",
        80,
        443,
        "/",
        "",
        None,
    );
    let project_url_generator = generator(compiled, context, None);

    let absolute_url = generate(
        &project_url_generator,
        "Test1",
        PhpArray::new(),
        AbsoluteUrl,
    )?;
    let relative_url = generate(
        &project_url_generator,
        "Test1",
        PhpArray::new(),
        AbsolutePath,
    )?;

    assert_eq!(absolute_url, "https://localhost/app.php/testing");
    assert_eq!(relative_url, "/app.php/testing");
    Ok(())
}

#[test]
fn testDumpWithLocalizedRoutesPreserveTheGoodLocaleInTheUrl() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("foo.en", localized("/{_locale}/fork", "en", "foo")?, 0);
    route_collection.add(
        "foo.fr",
        localized("/{_locale}/fourchette", "fr", "foo")?,
        0,
    );
    route_collection.add("fun.en", localized("/fun", "en", "fun")?, 0);
    route_collection.add("fun.fr", localized("/amusant", "fr", "fun")?, 0);

    let mut request_context = RequestContext::default();
    request_context.set_parameter(b"_locale", "fr".into());

    let compiled_url_generator = generator(dumped(route_collection)?, request_context, None);
    let en = || strings(&[("_locale", "en")]);
    let g =
        |name: &str, parameters| generate(&compiled_url_generator, name, parameters, AbsolutePath);

    assert_eq!(g("foo", PhpArray::new())?, "/fr/fourchette");
    assert_eq!(g("foo.en", PhpArray::new())?, "/en/fork");
    assert_eq!(g("foo", en())?, "/en/fork");
    assert_eq!(g("foo.fr", en())?, "/fr/fourchette");

    assert_eq!(g("fun", PhpArray::new())?, "/amusant");
    assert_eq!(g("fun.en", PhpArray::new())?, "/fun");
    assert_eq!(g("fun", en())?, "/fun");
    assert_eq!(g("fun.fr", en())?, "/amusant");
    Ok(())
}

#[test]
fn testLocalizedAliasRouteGeneratesCorrectUrlPerLocale() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("foo.en", localized("/en/fork", "en", "foo")?, 0);
    route_collection.add("foo.fr", localized("/fr/fourchette", "fr", "foo")?, 0);
    route_collection.add_alias(b"bar.en", b"foo.en")?;
    route_collection.add_alias(b"bar.fr", b"foo.fr")?;

    let mut request_context = RequestContext::default();
    request_context.set_parameter(b"_locale", "fr".into());

    let compiled_url_generator = generator(dumped(route_collection)?, request_context, None);
    let g = |parameters| generate(&compiled_url_generator, "bar", parameters, AbsolutePath);

    assert_eq!(g(PhpArray::new())?, "/fr/fourchette");
    assert_eq!(g(strings(&[("_locale", "en")]))?, "/en/fork");
    assert_eq!(g(strings(&[("_locale", "fr")]))?, "/fr/fourchette");
    Ok(())
}

#[test]
fn testAliases() -> TestResult {
    let mut sub_collection = RouteCollection::new();
    sub_collection.add("a", Route::new("/sub")?, 0);
    sub_collection.add_alias(b"b", b"a")?;
    sub_collection.add_alias(b"c", b"b")?;
    sub_collection.add_name_prefix(b"sub_")?;

    let mut route_collection = RouteCollection::new();
    route_collection.add("a", Route::new("/foo")?, 0);
    route_collection.add_alias(b"b", b"a")?;
    route_collection.add_alias(b"c", b"b")?;
    route_collection.add_collection(sub_collection);

    let compiled_url_generator =
        generator(dumped(route_collection)?, RequestContext::default(), None);

    assert_eq!(path(&compiled_url_generator, "b")?, "/foo");
    assert_eq!(path(&compiled_url_generator, "c")?, "/foo");
    assert_eq!(path(&compiled_url_generator, "sub_b")?, "/sub");
    assert_eq!(path(&compiled_url_generator, "sub_c")?, "/sub");
    Ok(())
}

#[test]
fn testTargetAliasNotExisting() -> TestResult {
    let mut route_collection = RouteCollection::new();
    route_collection.add("not-existing", Route::new("/not-existing")?, 0);
    route_collection.add_alias(b"alias", b"not-existing")?;

    let mut compiled_routes = dumped(route_collection)?;
    compiled_routes.remove(b"alias");

    let compiled_url_generator = generator(compiled_routes, RequestContext::default(), None);
    throws(path(&compiled_url_generator, "a"), ROUTE_NOT_FOUND);
    Ok(())
}

#[test]
fn testTargetAliasWithNamePrefixNotExisting() -> TestResult {
    let mut sub_collection = RouteCollection::new();
    sub_collection.add("not-existing", Route::new("/not-existing")?, 0);
    sub_collection.add_alias(b"alias", b"not-existing")?;
    sub_collection.add_name_prefix(b"sub_")?;

    let mut route_collection = RouteCollection::new();
    route_collection.add_collection(sub_collection);

    let mut compiled_routes = dumped(route_collection)?;
    compiled_routes.remove(b"sub_alias");

    let compiled_url_generator = generator(compiled_routes, RequestContext::default(), None);
    throws(path(&compiled_url_generator, "sub_alias"), ROUTE_NOT_FOUND);
    Ok(())
}

fn circular(aliases: &[(&str, &str)], message: &str) -> TestResult {
    let mut route_collection = RouteCollection::new();
    for (alias, target) in aliases {
        route_collection.add_alias(alias.as_bytes(), target.as_bytes())?;
    }
    let e = throws(dumped(route_collection), CIRCULAR);
    assert_eq!(e.message(), &s(message));
    Ok(())
}

#[test]
fn testCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "a")],
        "Circular reference detected for route \"b\", path: \"b -> a -> b\".",
    )
}

#[test]
fn testDeepCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "c"), ("c", "b")],
        "Circular reference detected for route \"b\", path: \"b -> c -> b\".",
    )
}

#[test]
fn testIndirectCircularReferenceShouldThrowAnException() -> TestResult {
    circular(
        &[("a", "b"), ("b", "c"), ("c", "a")],
        "Circular reference detected for route \"b\", path: \"b -> c -> a -> b\".",
    )
}

/// `expectUserDeprecationMessage($message)`: `f` triggers it.
fn expect_deprecation(message: &str, f: impl FnOnce() -> TestResult) -> TestResult {
    let (result, deprecations) = collect(f);
    result?;
    let messages: Vec<String> = deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect();
    assert!(
        messages.iter().any(|m| m == message),
        "{message:?} not in {messages:?}"
    );
    Ok(())
}

#[test]
fn testDeprecatedAlias() -> TestResult {
    expect_deprecation(
        "Since foo/bar 1.0.0: The \"b\" route alias is deprecated. You should stop using it, as it will be removed in the future.",
        || {
            let mut route_collection = RouteCollection::new();
            route_collection.add("a", Route::new("/foo")?, 0);
            route_collection
                .add_alias(b"b", b"a")?
                .set_deprecated("foo/bar", "1.0.0", "")?;

            let compiled_url_generator =
                generator(dumped(route_collection)?, RequestContext::default(), None);
            path(&compiled_url_generator, "b")?;
            Ok(())
        },
    )
}

#[test]
fn testDeprecatedAliasWithCustomMessage() -> TestResult {
    expect_deprecation("Since foo/bar 1.0.0: foo b.", || {
        let mut route_collection = RouteCollection::new();
        route_collection.add("a", Route::new("/foo")?, 0);
        route_collection.add_alias(b"b", b"a")?.set_deprecated(
            "foo/bar",
            "1.0.0",
            "foo %alias_id%.",
        )?;

        let compiled_url_generator =
            generator(dumped(route_collection)?, RequestContext::default(), None);
        path(&compiled_url_generator, "b")?;
        Ok(())
    })
}

#[test]
fn testTargettingADeprecatedAliasShouldTriggerDeprecation() -> TestResult {
    expect_deprecation("Since foo/bar 1.0.0: foo b.", || {
        let mut route_collection = RouteCollection::new();
        route_collection.add("a", Route::new("/foo")?, 0);
        route_collection.add_alias(b"b", b"a")?.set_deprecated(
            "foo/bar",
            "1.0.0",
            "foo %alias_id%.",
        )?;
        route_collection.add_alias(b"c", b"b")?;

        dumped(route_collection)?;
        Ok(())
    })
}
