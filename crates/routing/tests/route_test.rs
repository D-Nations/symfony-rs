//! RouteTest.php at symfony/routing f3fefc6cad3748d538be970ca81b71f06d7aa8b3.
//! Names and assertions are PHPUnit's; a data provider's cases run in a
//! loop inside one test. `assertSame()` on routes or compiled routes
//! compares values (a Route is not a shared handle here), and "compiles
//! once" is the cached compiled route. The `__toString()` gadget is an
//! object whose `__toString()` records that it ran.
//!
//! Upstream: symfony/routing Tests/RouteTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use common::{TestResult, list, s, strings, values};
use php_value::{
    PhpArray, PhpObject, PhpString, Value,
    object::{Closure, GenericObject},
    serialize::serialize,
};
use symfony_routing::{
    compiled_route::CompiledRoute,
    exception::Error,
    route::{Arguments, Route},
    route_compiler::RouteCompilerInterface,
};

const CUSTOM_ROUTE_COMPILER: &str =
    "Symfony\\Component\\Routing\\Tests\\Fixtures\\CustomRouteCompiler";
const CUSTOM_COMPILED_ROUTE: &str =
    "Symfony\\Component\\Routing\\Tests\\Fixtures\\CustomCompiledRoute";

/// Tests/Fixtures/CustomRouteCompiler.php.
struct CustomRouteCompiler;

impl RouteCompilerInterface for CustomRouteCompiler {
    const CLASS: &'static str = CUSTOM_ROUTE_COMPILER;

    fn compile(_route: &mut Route) -> Result<CompiledRoute, Error> {
        Ok(CompiledRoute::new(
            "",
            "",
            PhpArray::new(),
            PhpArray::new(),
            None,
            PhpArray::new(),
            PhpArray::new(),
            PhpArray::new(),
        )
        .with_class(CUSTOM_COMPILED_ROUTE))
    }
}

#[test]
fn testConstructor() -> TestResult {
    let route = Route::with(
        "/{foo}",
        Arguments {
            defaults: strings(&[("foo", "bar")]),
            requirements: strings(&[("foo", r"\d+")]),
            options: strings(&[("foo", "bar")]),
            host: s("{locale}.example.com"),
            ..Arguments::default()
        },
    )?;
    assert_eq!(
        s("/{foo}"),
        *route.path(),
        "__construct() takes a path as its first argument"
    );
    assert_eq!(
        strings(&[("foo", "bar")]),
        *route.defaults(),
        "__construct() takes defaults as its second argument"
    );
    assert_eq!(
        strings(&[("foo", r"\d+")]),
        *route.requirements(),
        "__construct() takes requirements as its third argument"
    );
    assert_eq!(
        Some(&Value::string("bar")),
        route.option(b"foo"),
        "__construct() takes options as its fourth argument"
    );
    assert_eq!(
        s("{locale}.example.com"),
        *route.host(),
        "__construct() takes a host pattern as its fifth argument"
    );

    let route = Route::with(
        "/",
        Arguments {
            schemes: vec![s("Https")],
            methods: vec![s("POST"), s("put")],
            condition: s("context.getMethod() == \"GET\""),
            ..Arguments::default()
        },
    )?;
    assert_eq!(
        list(&["https"]),
        *route.schemes(),
        "__construct() takes schemes as its sixth argument and lowercases it"
    );
    assert_eq!(
        list(&["POST", "PUT"]),
        *route.methods(),
        "__construct() takes methods as its seventh argument and uppercases it"
    );
    assert_eq!(
        s("context.getMethod() == \"GET\""),
        *route.condition(),
        "__construct() takes a condition as its eight argument"
    );

    let route = Route::with(
        "/",
        Arguments {
            schemes: vec![s("Https")],
            methods: vec![s("Post")],
            ..Arguments::default()
        },
    )?;
    assert_eq!(
        list(&["https"]),
        *route.schemes(),
        "__construct() takes a single scheme as its sixth argument"
    );
    assert_eq!(
        list(&["POST"]),
        *route.methods(),
        "__construct() takes a single method as its seventh argument"
    );
    Ok(())
}

#[test]
fn testPath() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    route.set_path("/{bar}")?;
    assert_eq!(s("/{bar}"), *route.path(), "->setPath() sets the path");
    route.set_path("")?;
    assert_eq!(
        s("/"),
        *route.path(),
        "->setPath() adds a / at the beginning of the path if needed"
    );
    route.set_path("bar")?;
    assert_eq!(
        s("/bar"),
        *route.path(),
        "->setPath() adds a / at the beginning of the path if needed"
    );
    let path = route.set_path("")?.path().clone();
    assert_eq!(
        *route.path(),
        path,
        "->setPath() implements a fluent interface"
    );
    route.set_path("//path")?;
    assert_eq!(
        s("/path"),
        *route.path(),
        "->setPath() does not allow two slashes \"//\" at the beginning of the path as it would be confused with a network path when generating the path from the route"
    );
    route.set_path("/path/{!foo}")?;
    assert_eq!(
        s("/path/{!foo}"),
        *route.path(),
        "->setPath() keeps ! to pass important params"
    );
    route.set_path(r"/path/{bar<\w++>}")?;
    assert_eq!(
        s("/path/{bar}"),
        *route.path(),
        "->setPath() removes inline requirements"
    );
    route.set_path("/path/{foo?value}")?;
    assert_eq!(
        s("/path/{foo}"),
        *route.path(),
        "->setPath() removes inline defaults"
    );
    route.set_path(r"/path/{!bar<\d+>?value}")?;
    assert_eq!(
        s("/path/{!bar}"),
        *route.path(),
        "->setPath() removes all inline settings"
    );
    Ok(())
}

#[test]
fn testOptions() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    route.set_options(&strings(&[("foo", "bar")]));
    assert_eq!(
        strings(&[
            (
                "compiler_class",
                "Symfony\\Component\\Routing\\RouteCompiler"
            ),
            ("foo", "bar"),
        ]),
        *route.options(),
        "->setOptions() sets the options"
    );
    let options = route.set_options(&PhpArray::new()).options().clone();
    assert_eq!(
        *route.options(),
        options,
        "->setOptions() implements a fluent interface"
    );

    route.set_options(&strings(&[("foo", "foo")]));
    route.add_options(&strings(&[("bar", "bar")]));
    let options = route.add_options(&PhpArray::new()).options().clone();
    assert_eq!(
        *route.options(),
        options,
        "->addOptions() implements a fluent interface"
    );
    // assertEquals() on arrays ignores key order
    let mut expected = strings(&[
        ("foo", "foo"),
        ("bar", "bar"),
        (
            "compiler_class",
            "Symfony\\Component\\Routing\\RouteCompiler",
        ),
    ]);
    let mut actual = route.options().clone();
    expected.ksort();
    actual.ksort();
    assert_eq!(expected, actual, "->addDefaults() keep previous defaults");
    Ok(())
}

#[test]
fn testOption() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    assert!(
        !route.has_option(b"foo"),
        "->hasOption() return false if option is not set"
    );
    let option = route
        .set_option(b"foo", Value::string("bar"))
        .option(b"foo")
        .cloned();
    assert_eq!(
        route.option(b"foo").cloned(),
        option,
        "->setOption() implements a fluent interface"
    );
    assert_eq!(
        Some(&Value::string("bar")),
        route.option(b"foo"),
        "->setOption() sets the option"
    );
    assert!(
        route.has_option(b"foo"),
        "->hasOption() return true if option is set"
    );
    Ok(())
}

#[test]
fn testDefaults() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    route.set_defaults(&strings(&[("foo", "bar")]))?;
    assert_eq!(
        strings(&[("foo", "bar")]),
        *route.defaults(),
        "->setDefaults() sets the defaults"
    );
    let defaults = route.set_defaults(&PhpArray::new())?.defaults().clone();
    assert_eq!(
        *route.defaults(),
        defaults,
        "->setDefaults() implements a fluent interface"
    );

    route.set_default(b"foo", Value::string("bar"))?;
    assert_eq!(
        Some(&Value::string("bar")),
        route.default_value(b"foo"),
        "->setDefault() sets a default value"
    );

    route.set_default(b"foo2", Value::string("bar2"))?;
    assert_eq!(
        Some(&Value::string("bar2")),
        route.default_value(b"foo2"),
        "->getDefault() return the default value"
    );
    assert_eq!(
        None,
        route.default_value(b"not_defined"),
        "->getDefault() return null if default value is not set"
    );

    let closure = Closure::object(|_| Value::string("Hello"));
    route.set_default(b"_controller", closure.clone())?;
    assert_eq!(
        Some(&closure),
        route.default_value(b"_controller"),
        "->setDefault() sets a default value"
    );

    route.set_defaults(&strings(&[("foo", "foo")]))?;
    route.add_defaults(&strings(&[("bar", "bar")]))?;
    let defaults = route.add_defaults(&PhpArray::new())?.defaults().clone();
    assert_eq!(
        *route.defaults(),
        defaults,
        "->addDefaults() implements a fluent interface"
    );
    assert_eq!(
        strings(&[("foo", "foo"), ("bar", "bar")]),
        *route.defaults(),
        "->addDefaults() keep previous defaults"
    );
    Ok(())
}

#[test]
fn testRequirements() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    route.set_requirements(&strings(&[("foo", r"\d+")]))?;
    assert_eq!(
        strings(&[("foo", r"\d+")]),
        *route.requirements(),
        "->setRequirements() sets the requirements"
    );
    assert_eq!(
        Some(&s(r"\d+")),
        route.requirement(b"foo"),
        "->getRequirement() returns a requirement"
    );
    assert_eq!(
        None,
        route.requirement(b"bar"),
        "->getRequirement() returns null if a requirement is not defined"
    );
    route.set_requirements(&strings(&[("foo", r"^\d+$")]))?;
    assert_eq!(
        Some(&s(r"\d+")),
        route.requirement(b"foo"),
        "->getRequirement() removes ^ and $ from the path"
    );
    let requirements = route
        .set_requirements(&PhpArray::new())?
        .requirements()
        .clone();
    assert_eq!(
        *route.requirements(),
        requirements,
        "->setRequirements() implements a fluent interface"
    );

    route.set_requirements(&strings(&[("foo", r"\d+")]))?;
    route.add_requirements(&strings(&[("bar", r"\d+")]))?;
    let requirements = route
        .add_requirements(&PhpArray::new())?
        .requirements()
        .clone();
    assert_eq!(
        *route.requirements(),
        requirements,
        "->addRequirements() implements a fluent interface"
    );
    assert_eq!(
        strings(&[("foo", r"\d+"), ("bar", r"\d+")]),
        *route.requirements(),
        "->addRequirement() keep previous requirements"
    );
    Ok(())
}

#[test]
fn testRequirement() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    assert!(
        !route.has_requirement(b"foo"),
        "->hasRequirement() return false if requirement is not set"
    );
    route.set_requirement(b"foo", br"^\d+$")?;
    assert_eq!(
        Some(&s(r"\d+")),
        route.requirement(b"foo"),
        "->setRequirement() removes ^ and $ from the path"
    );
    assert!(
        route.has_requirement(b"foo"),
        "->hasRequirement() return true if requirement is set"
    );
    Ok(())
}

#[test]
fn testRequirementAlternativeStartAndEndRegexSyntax() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    route.set_requirement(b"foo", br"\A\d+\z")?;
    assert_eq!(
        Some(&s(r"\d+")),
        route.requirement(b"foo"),
        "->setRequirement() removes \\A and \\z from the path"
    );
    assert!(route.has_requirement(b"foo"));
    Ok(())
}

/// `getInvalidRequirements()`
const INVALID_REQUIREMENTS: [&str; 7] = ["", "^$", "^", "$", r"\A\z", r"\A", r"\z"];

#[test]
fn testSetInvalidRequirement() -> TestResult {
    for req in INVALID_REQUIREMENTS {
        let mut route = Route::new("/{foo}")?;
        let error = route.set_requirement(b"foo", req.as_bytes()).err();
        assert!(
            error.is_some_and(|e| e.is_a("InvalidArgumentException")),
            "{req:?}"
        );
    }
    Ok(())
}

#[test]
fn testHost() -> TestResult {
    let mut route = Route::new("/")?;
    route.set_host("{locale}.example.net")?;
    assert_eq!(
        s("{locale}.example.net"),
        *route.host(),
        "->setHost() sets the host pattern"
    );
    Ok(())
}

#[test]
fn testScheme() -> TestResult {
    let mut route = Route::new("/")?;
    assert_eq!(
        PhpArray::new(),
        *route.schemes(),
        "schemes is initialized with []"
    );
    assert!(!route.has_scheme(b"http"));
    route.set_schemes(["hTTp"]);
    assert_eq!(
        list(&["http"]),
        *route.schemes(),
        "->setSchemes() accepts a single scheme string and lowercases it"
    );
    assert!(route.has_scheme(b"htTp"));
    assert!(!route.has_scheme(b"httpS"));
    route.set_schemes(["HttpS", "hTTp"]);
    assert_eq!(
        list(&["https", "http"]),
        *route.schemes(),
        "->setSchemes() accepts an array of schemes and lowercases them"
    );
    assert!(route.has_scheme(b"htTp"));
    assert!(route.has_scheme(b"httpS"));
    Ok(())
}

#[test]
fn testMethod() -> TestResult {
    let mut route = Route::new("/")?;
    assert_eq!(
        PhpArray::new(),
        *route.methods(),
        "methods is initialized with []"
    );
    route.set_methods(["gEt"]);
    assert_eq!(
        list(&["GET"]),
        *route.methods(),
        "->setMethods() accepts a single method string and uppercases it"
    );
    route.set_methods(["gEt", "PosT"]);
    assert_eq!(
        list(&["GET", "POST"]),
        *route.methods(),
        "->setMethods() accepts an array of methods and uppercases them"
    );
    Ok(())
}

#[test]
fn testCondition() -> TestResult {
    let mut route = Route::new("/")?;
    assert_eq!(s(""), *route.condition());
    route.set_condition("context.getMethod() == \"GET\"");
    assert_eq!(s("context.getMethod() == \"GET\""), *route.condition());
    Ok(())
}

#[test]
fn testCompile() -> TestResult {
    let mut route = Route::new("/{foo}")?;
    let compiled = route.compile()?.clone();
    assert_eq!(
        compiled.class(),
        "Symfony\\Component\\Routing\\CompiledRoute",
        "->compile() returns a compiled route"
    );
    assert_eq!(
        Some(&compiled),
        route.compiled(),
        "->compile() only compiled the route once if unchanged"
    );
    route.set_requirement(b"foo", b".*")?;
    assert_eq!(
        None,
        route.compiled(),
        "->compile() recompiles if the route was modified"
    );
    assert_ne!(
        compiled,
        *route.compile()?,
        "->compile() recompiles if the route was modified"
    );
    Ok(())
}

#[test]
fn testSerialize() -> TestResult {
    let route = Route::with(
        "/prefix/{foo}",
        Arguments {
            defaults: strings(&[("foo", "default")]),
            requirements: strings(&[("foo", r"\d+")]),
            ..Arguments::default()
        },
    )?;

    let serialized = route.serialize()?;
    let unserialized = Route::unserialize(&serialized, &Route::classes(), &[])?;

    assert_eq!(Some(route), unserialized);
    Ok(())
}

/// `RouteTestToStringGadget`: a `\Stringable` whose `__toString()` records
/// that it ran.
#[derive(Debug)]
struct Gadget(Arc<AtomicBool>);

const GADGET: &str = "Symfony\\Component\\Routing\\Tests\\RouteTestToStringGadget";

impl PhpObject for Gadget {
    fn class(&self) -> &str {
        GADGET
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        self.0.store(true, Ordering::SeqCst);
        Some(PhpString::default())
    }

    fn is_stringable(&self) -> bool {
        true
    }
}

#[test]
fn testUnserializeRejectsObjectInTypedScalarProperty() -> TestResult {
    // provideTrampolineProperties()
    for property in ["path", "host", "condition"] {
        let mut data: Vec<(&str, Value)> = vec![
            ("path", Value::string("/")),
            ("host", Value::string("")),
            ("defaults", Value::Array(PhpArray::new())),
            ("requirements", Value::Array(PhpArray::new())),
            ("options", Value::Array(PhpArray::new())),
            ("schemes", Value::Array(PhpArray::new())),
            ("methods", Value::Array(PhpArray::new())),
            ("condition", Value::string("")),
        ];
        let gadget = Value::Object(Arc::new(GenericObject::new(GADGET, PhpArray::new())));
        for (key, value) in &mut data {
            if *key == property {
                *value = gadget.clone();
            }
        }
        let class = "Symfony\\Component\\Routing\\Route";
        let mut payload = format!("O:{}:\"{class}\":{}:{{", class.len(), data.len()).into_bytes();
        for (key, value) in &data {
            payload.extend_from_slice(&serialize(&Value::string(*key)));
            payload.extend_from_slice(&serialize(value));
        }
        payload.push(b'}');
        let fired = Arc::new(AtomicBool::new(false));

        let mut classes = Route::classes();
        let flag = fired.clone();
        classes.allow_with(GADGET, Arc::new(move |_| Arc::new(Gadget(flag.clone()))));
        match Route::unserialize(&payload, &classes, &[]) {
            Err(e) if e.is_a("BadMethodCallException") => {}
            other => panic!("Expected BadMethodCallException, got {other:?} ({property})"),
        }

        assert!(
            !fired.load(Ordering::SeqCst),
            "__toString gadget must not fire during unserialize"
        );
    }
    Ok(())
}

/// A row of `provideInlineDefaultAndRequirementCases()`.
struct Inline {
    route: Route,
    path: &'static str,
    host: &'static str,
    defaults: PhpArray,
    requirements: PhpArray,
}

fn host(host: &str) -> Arguments {
    Arguments {
        host: s(host),
        ..Arguments::default()
    }
}

fn provide_inline_default_and_requirement_cases() -> Result<Vec<Inline>, Error> {
    let null = |name: &str| values(vec![(name, Value::Null)]);
    let none = PhpArray::new;
    let row = |route, path, host, defaults, requirements| Inline {
        route,
        path,
        host,
        defaults,
        requirements,
    };
    Ok(vec![
        row(
            Route::new("/foo/{bar?}")?,
            "/foo/{bar}",
            "",
            null("bar"),
            none(),
        ),
        row(
            Route::new("/foo/{bar?baz}")?,
            "/foo/{bar}",
            "",
            strings(&[("bar", "baz")]),
            none(),
        ),
        row(
            Route::new("/foo/{bar?baz<buz>}")?,
            "/foo/{bar}",
            "",
            strings(&[("bar", "baz<buz>")]),
            none(),
        ),
        row(
            Route::new("/foo/{!bar?baz<buz>}")?,
            "/foo/{!bar}",
            "",
            strings(&[("bar", "baz<buz>")]),
            none(),
        ),
        row(
            Route::with(
                "/foo/{bar?}",
                Arguments {
                    defaults: strings(&[("bar", "baz")]),
                    ..Arguments::default()
                },
            )?,
            "/foo/{bar}",
            "",
            strings(&[("bar", "baz")]),
            none(),
        ),
        row(
            Route::new("/foo/{bar<.*>}")?,
            "/foo/{bar}",
            "",
            none(),
            strings(&[("bar", ".*")]),
        ),
        row(
            Route::new("/foo/{bar<>>}")?,
            "/foo/{bar}",
            "",
            none(),
            strings(&[("bar", ">")]),
        ),
        row(
            Route::with(
                "/foo/{bar<.*>}",
                Arguments {
                    requirements: strings(&[("bar", r"\d+")]),
                    ..Arguments::default()
                },
            )?,
            "/foo/{bar}",
            "",
            none(),
            strings(&[("bar", r"\d+")]),
        ),
        row(
            Route::new("/foo/{bar<[a-z]{2}>}")?,
            "/foo/{bar}",
            "",
            none(),
            strings(&[("bar", "[a-z]{2}")]),
        ),
        row(
            Route::new(r"/foo/{!bar<\d+>}")?,
            "/foo/{!bar}",
            "",
            none(),
            strings(&[("bar", r"\d+")]),
        ),
        row(
            Route::new("/foo/{bar<.*>?}")?,
            "/foo/{bar}",
            "",
            null("bar"),
            strings(&[("bar", ".*")]),
        ),
        row(
            Route::new("/foo/{bar<>>?<>}")?,
            "/foo/{bar}",
            "",
            strings(&[("bar", "<>")]),
            strings(&[("bar", ">")]),
        ),
        row(
            Route::new(r"/{foo<.>?\}/{!bar<\>?<>}")?,
            "/{foo}/{!bar}",
            "",
            strings(&[("foo", r"\"), ("bar", "<>")]),
            strings(&[("foo", "."), ("bar", r"\")]),
        ),
        row(
            Route::with("/", host("{bar?}"))?,
            "/",
            "{bar}",
            null("bar"),
            none(),
        ),
        row(
            Route::with("/", host("{bar?baz}"))?,
            "/",
            "{bar}",
            strings(&[("bar", "baz")]),
            none(),
        ),
        row(
            Route::with("/", host("{bar?baz<buz>}"))?,
            "/",
            "{bar}",
            strings(&[("bar", "baz<buz>")]),
            none(),
        ),
        row(
            Route::with(
                "/",
                Arguments {
                    defaults: strings(&[("bar", "baz")]),
                    ..host("{bar?}")
                },
            )?,
            "/",
            "{bar}",
            null("bar"),
            none(),
        ),
        row(
            Route::with("/", host("{bar<.*>}"))?,
            "/",
            "{bar}",
            none(),
            strings(&[("bar", ".*")]),
        ),
        row(
            Route::with("/", host("{bar<>>}"))?,
            "/",
            "{bar}",
            none(),
            strings(&[("bar", ">")]),
        ),
        row(
            Route::with(
                "/",
                Arguments {
                    requirements: strings(&[("bar", r"\d+")]),
                    ..host("{bar<.*>}")
                },
            )?,
            "/",
            "{bar}",
            none(),
            strings(&[("bar", ".*")]),
        ),
        row(
            Route::with("/", host("{bar<[a-z]{2}>}"))?,
            "/",
            "{bar}",
            none(),
            strings(&[("bar", "[a-z]{2}")]),
        ),
        row(
            Route::with("/", host("{bar<.*>?}"))?,
            "/",
            "{bar}",
            null("bar"),
            strings(&[("bar", ".*")]),
        ),
        row(
            Route::with("/", host("{bar<>>?<>}"))?,
            "/",
            "{bar}",
            strings(&[("bar", "<>")]),
            strings(&[("bar", ">")]),
        ),
    ])
}

#[test]
fn testInlineDefaultAndRequirement() -> TestResult {
    for case in provide_inline_default_and_requirement_cases()? {
        assert_eq!(s(case.path), *case.route.path());
        assert_eq!(s(case.host), *case.route.host());
        assert_eq!(case.defaults, *case.route.defaults(), "{}", case.path);
        assert_eq!(
            case.requirements,
            *case.route.requirements(),
            "{}",
            case.path
        );
    }
    Ok(())
}

/// Tests that the compiled version is also serialized to prevent the overhead
/// of compiling it again after unserialize.
#[test]
fn testSerializeWhenCompiled() -> TestResult {
    let mut route = Route::with(
        "/prefix/{foo}",
        Arguments {
            defaults: strings(&[("foo", "default")]),
            requirements: strings(&[("foo", r"\d+")]),
            ..Arguments::default()
        },
    )?;
    route.set_host("{locale}.example.net")?;
    route.compile()?;

    let serialized = route.serialize()?;
    let unserialized = Route::unserialize(&serialized, &Route::classes(), &[])?;

    assert_eq!(Some(route), unserialized);
    Ok(())
}

/// Tests that unserialization does not fail when the compiled Route is of a
/// class other than `CompiledRoute`, such as a subclass of it.
#[test]
fn testSerializeWhenCompiledWithClass() -> TestResult {
    let mut route = Route::with(
        "/",
        Arguments {
            options: strings(&[("compiler_class", CUSTOM_ROUTE_COMPILER)]),
            ..Arguments::default()
        },
    )?;
    assert_eq!(
        CUSTOM_COMPILED_ROUTE,
        route.compile_with::<CustomRouteCompiler>()?.class(),
        "->compile() returned a proper route"
    );

    let serialized = route.serialize()?;
    let mut classes = Route::classes();
    classes.allow_with_defaults(CUSTOM_COMPILED_ROUTE, PhpArray::new());
    match Route::unserialize(&serialized, &classes, &[CUSTOM_COMPILED_ROUTE]) {
        Ok(Some(mut unserialized)) => assert_eq!(
            CUSTOM_COMPILED_ROUTE,
            unserialized.compile_with::<CustomRouteCompiler>()?.class(),
            "the unserialized route compiled successfully"
        ),
        _ => panic!("unserializing a route which uses a custom compiled route class"),
    }
    Ok(())
}

/// Tests that the serialized representation of a route in one symfony version
/// also works in later symfony versions, i.e. the unserialized route is in the
/// same state as another, semantically equivalent, route.
#[test]
fn testSerializedRepresentationKeepsWorking() -> TestResult {
    let serialized = br#"O:31:"Symfony\Component\Routing\Route":9:{s:4:"path";s:13:"/prefix/{foo}";s:4:"host";s:20:"{locale}.example.net";s:8:"defaults";a:1:{s:3:"foo";s:7:"default";}s:12:"requirements";a:1:{s:3:"foo";s:3:"\d+";}s:7:"options";a:1:{s:14:"compiler_class";s:39:"Symfony\Component\Routing\RouteCompiler";}s:7:"schemes";a:0:{}s:7:"methods";a:0:{}s:9:"condition";s:0:"";s:8:"compiled";O:39:"Symfony\Component\Routing\CompiledRoute":8:{s:4:"vars";a:2:{i:0;s:6:"locale";i:1;s:3:"foo";}s:11:"path_prefix";s:7:"/prefix";s:10:"path_regex";s:31:"{^/prefix(?:/(?P<foo>\d+))?$}sD";s:11:"path_tokens";a:2:{i:0;a:4:{i:0;s:8:"variable";i:1;s:1:"/";i:2;s:3:"\d+";i:3;s:3:"foo";}i:1;a:2:{i:0;s:4:"text";i:1;s:7:"/prefix";}}s:9:"path_vars";a:1:{i:0;s:3:"foo";}s:10:"host_regex";s:40:"{^(?P<locale>[^\.]++)\.example\.net$}sDi";s:11:"host_tokens";a:2:{i:0;a:2:{i:0;s:4:"text";i:1;s:12:".example.net";}i:1;a:4:{i:0;s:8:"variable";i:1;s:0:"";i:2;s:7:"[^\.]++";i:3;s:6:"locale";}}s:9:"host_vars";a:1:{i:0;s:6:"locale";}}}"#;
    let unserialized = Route::unserialize(serialized, &Route::classes(), &[])?;

    let mut route = Route::with(
        "/prefix/{foo}",
        Arguments {
            defaults: strings(&[("foo", "default")]),
            requirements: strings(&[("foo", r"\d+")]),
            ..Arguments::default()
        },
    )?;
    route.set_host("{locale}.example.net")?;
    route.compile()?;

    assert_eq!(Some(route), unserialized);
    Ok(())
}

fn provide_non_localized_routes() -> Result<Vec<Route>, Error> {
    let mut routes = vec![Route::new("/foo")?];
    let mut r = Route::new("/foo")?;
    r.set_default(b"_locale", Value::string("en"))?;
    routes.push(r);
    let mut r = Route::new("/foo")?;
    r.set_default(b"_locale", Value::string("en"))?
        .set_default(b"_canonical_route", Value::string("foo"))?;
    routes.push(r);
    let mut r = Route::new("/foo")?;
    r.set_default(b"_locale", Value::string("en"))?
        .set_default(b"_canonical_route", Value::string("foo"))?
        .set_requirement(b"_locale", b"foobar")?;
    routes.push(r);
    Ok(routes)
}

fn provide_localized_routes() -> Result<Vec<Route>, Error> {
    let mut r = Route::new("/foo")?;
    r.set_default(b"_locale", Value::string("en"))?
        .set_default(b"_canonical_route", Value::string("foo"))?
        .set_requirement(b"_locale", b"en")?;
    Ok(vec![r])
}

#[test]
fn testLocaleDefaultWithNonLocalizedRoutes() -> TestResult {
    for mut route in provide_non_localized_routes()? {
        assert_ne!(Some(&Value::string("fr")), route.default_value(b"_locale"));
        route.set_default(b"_locale", Value::string("fr"))?;
        assert_eq!(Some(&Value::string("fr")), route.default_value(b"_locale"));
    }
    Ok(())
}

#[test]
fn testLocaleDefaultWithLocalizedRoutes() -> TestResult {
    for mut route in provide_localized_routes()? {
        let expected = route.default_value(b"_locale").cloned();
        assert!(matches!(expected, Some(Value::String(_))));
        assert_ne!(Some(Value::string("fr")), expected);
        route.set_default(b"_locale", Value::string("fr"))?;
        assert_eq!(expected.as_ref(), route.default_value(b"_locale"));
    }
    Ok(())
}

#[test]
fn testLocaleRequirementWithNonLocalizedRoutes() -> TestResult {
    for mut route in provide_non_localized_routes()? {
        assert_ne!(Some(&s("fr")), route.requirement(b"_locale"));
        route.set_requirement(b"_locale", b"fr")?;
        assert_eq!(Some(&s("fr")), route.requirement(b"_locale"));
    }
    Ok(())
}

#[test]
fn testLocaleRequirementWithLocalizedRoutes() -> TestResult {
    for mut route in provide_localized_routes()? {
        let expected = route.requirement(b"_locale").cloned();
        assert!(expected.is_some());
        assert_ne!(Some(s("fr")), expected);
        route.set_requirement(b"_locale", b"fr")?;
        assert_eq!(expected.as_ref(), route.requirement(b"_locale"));
    }
    Ok(())
}
