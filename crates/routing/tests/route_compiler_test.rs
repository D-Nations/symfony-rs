//! RouteCompilerTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's; a data provider's cases run in a loop inside one test.
//! `new \ReflectionClass(Route::class)->newInstanceArgs($arguments)` is
//! `Route::with()` with the same positional arguments, and the
//! `Utf8RouteCompiler` fixture (a `RouteCompiler` whose SEPARATORS are `/§`)
//! is `RouteCompiler::compile_with_separators()` under its class name.
//!
//! Upstream: symfony/routing Tests/RouteCompilerTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use common::{TestResult, list, s, strings, values};
use php_value::{PhpArray, PhpString, Value};
use symfony_routing::{
    compiled_route::CompiledRoute,
    exception::Error,
    route::{Arguments, Route},
    route_compiler::{RouteCompiler, RouteCompilerInterface},
};

/// The `Utf8RouteCompiler` class at the end of RouteCompilerTest.php.
struct Utf8RouteCompiler;

impl RouteCompilerInterface for Utf8RouteCompiler {
    const CLASS: &'static str = "Symfony\\Component\\Routing\\Tests\\Utf8RouteCompiler";

    fn compile(route: &mut Route) -> Result<CompiledRoute, Error> {
        RouteCompiler::compile_with_separators(route, "/§".as_bytes())
    }
}

fn text(t: &[u8]) -> Value {
    Value::Array(PhpArray::list([
        Value::string("text"),
        Value::String(PhpString::from(t)),
    ]))
}

fn var(separator: &str, regex: &str, name: &str) -> Value {
    Value::Array(PhpArray::list([
        Value::string("variable"),
        Value::string(separator),
        Value::string(regex),
        Value::string(name),
    ]))
}

fn var_utf8(separator: &str, regex: &str, name: &str) -> Value {
    Value::Array(PhpArray::list([
        Value::string("variable"),
        Value::string(separator),
        Value::string(regex),
        Value::string(name),
        Value::Bool(true),
    ]))
}

fn tokens(items: Vec<Value>) -> PhpArray {
    PhpArray::list(items)
}

/// `newInstanceArgs([$path, $defaults, $requirements, $options])`.
fn route(
    path: &[u8],
    defaults: PhpArray,
    requirements: PhpArray,
    options: PhpArray,
) -> Result<Route, Error> {
    Route::with(
        path,
        Arguments {
            defaults,
            requirements,
            options,
            ..Arguments::default()
        },
    )
}

struct Case {
    name: &'static str,
    route: Result<Route, Error>,
    prefix: &'static [u8],
    regex: &'static [u8],
    variables: PhpArray,
    tokens: PhpArray,
}

fn none() -> PhpArray {
    PhpArray::new()
}

fn provide_compile_data() -> Vec<Case> {
    let case = |name, route, prefix, regex, variables, tokens| Case {
        name,
        route,
        prefix,
        regex,
        variables,
        tokens,
    };
    vec![
        case(
            "Static route",
            route(b"/foo", none(), none(), none()),
            b"/foo", b"{^/foo$}sD", none(), tokens(vec![
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with a variable",
            route(b"/foo/{bar}", none(), none(), none()),
            b"/foo", b"{^/foo/(?P<bar>[^/]++)$}sD", list(&["bar"]), tokens(vec![
                var("/", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with a variable that has a default value",
            route(b"/foo/{bar}", strings(&[("bar", "bar")]), none(), none()),
            b"/foo", b"{^/foo(?:/(?P<bar>[^/]++))?$}sD", list(&["bar"]), tokens(vec![
                var("/", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with several variables",
            route(b"/foo/{bar}/{foobar}", none(), none(), none()),
            b"/foo", b"{^/foo/(?P<bar>[^/]++)/(?P<foobar>[^/]++)$}sD", list(&["bar", "foobar"]), tokens(vec![
                var("/", "[^/]++", "foobar"),
                var("/", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with several variables that have default values",
            route(b"/foo/{bar}/{foobar}", strings(&[("bar", "bar"), ("foobar", "")]), none(), none()),
            b"/foo", b"{^/foo(?:/(?P<bar>[^/]++)(?:/(?P<foobar>[^/]++))?)?$}sD", list(&["bar", "foobar"]), tokens(vec![
                var("/", "[^/]++", "foobar"),
                var("/", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with several variables but some of them have no default values",
            route(b"/foo/{bar}/{foobar}", strings(&[("bar", "bar")]), none(), none()),
            b"/foo", b"{^/foo/(?P<bar>[^/]++)/(?P<foobar>[^/]++)$}sD", list(&["bar", "foobar"]), tokens(vec![
                var("/", "[^/]++", "foobar"),
                var("/", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with an optional variable as the first segment",
            route(b"/{bar}", strings(&[("bar", "bar")]), none(), none()),
            b"", b"{^/(?P<bar>[^/]++)?$}sD", list(&["bar"]), tokens(vec![
                var("/", "[^/]++", "bar"),
            ]),
        ),
        case(
            "Route with a requirement of 0",
            route(b"/{bar}", values(vec![("bar", Value::Null)]), strings(&[("bar", "0")]), none()),
            b"", b"{^/(?P<bar>0)?$}sD", list(&["bar"]), tokens(vec![
                var("/", "0", "bar"),
            ]),
        ),
        case(
            "Route with an optional variable as the first segment with requirements",
            route(b"/{bar}", strings(&[("bar", "bar")]), strings(&[("bar", "(foo|bar)")]), none()),
            b"", b"{^/(?P<bar>(?:foo|bar))?$}sD", list(&["bar"]), tokens(vec![
                var("/", "(?:foo|bar)", "bar"),
            ]),
        ),
        case(
            "Route with only optional variables",
            route(b"/{foo}/{bar}", strings(&[("foo", "foo"), ("bar", "bar")]), none(), none()),
            b"", b"{^/(?P<foo>[^/]++)?(?:/(?P<bar>[^/]++))?$}sD", list(&["foo", "bar"]), tokens(vec![
                var("/", "[^/]++", "bar"),
                var("/", "[^/]++", "foo"),
            ]),
        ),
        case(
            "Route with a variable in last position",
            route(b"/foo-{bar}", none(), none(), none()),
            b"/foo-", br"{^/foo\-(?P<bar>[^/]++)$}sD", list(&["bar"]), tokens(vec![
                var("-", "[^/]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Route with nested placeholders",
            route(b"/{static{var}static}", none(), none(), none()),
            b"/{static", br"{^/\{static(?P<var>[^/]+)static\}$}sD", list(&["var"]), tokens(vec![
                text(b"static}"),
                var("", "[^/]+", "var"),
                text(b"/{static"),
            ]),
        ),
        case(
            "Route without separator between variables",
            route(
                b"/{w}{x}{y}{z}.{_format}",
                strings(&[("z", "default-z"), ("_format", "html")]),
                strings(&[("y", "(y|Y)")]),
                none(),
            ),
            b"", br"{^/(?P<w>[^/\.]+)(?P<x>[^/\.]+)(?P<y>(?:y|Y))(?:(?P<z>[^/\.]++)(?:\.(?P<_format>[^/]++))?)?$}sD", list(&["w", "x", "y", "z", "_format"]), tokens(vec![
                var(".", "[^/]++", "_format"),
                var("", r"[^/\.]++", "z"),
                var("", "(?:y|Y)", "y"),
                var("", r"[^/\.]+", "x"),
                var("/", r"[^/\.]+", "w"),
            ]),
        ),
        case(
            "Route with a format",
            route(b"/foo/{bar}.{_format}", none(), none(), none()),
            b"/foo", br"{^/foo/(?P<bar>[^/\.]++)\.(?P<_format>[^/]++)$}sD", list(&["bar", "_format"]), tokens(vec![
                var(".", "[^/]++", "_format"),
                var("/", r"[^/\.]++", "bar"),
                text(b"/foo"),
            ]),
        ),
        case(
            "Static non UTF-8 route",
            route(b"/fo\xE9", none(), none(), none()),
            b"/fo\xE9", b"{^/fo\xE9$}sD", none(), tokens(vec![
                text(b"/fo\xE9"),
            ]),
        ),
        case(
            "Route with an explicit UTF-8 requirement",
            route(b"/{bar}", values(vec![("bar", Value::Null)]), strings(&[("bar", ".")]), values(vec![("utf8", Value::Bool(true))])),
            b"", b"{^/(?P<bar>.)?$}sDu", list(&["bar"]), tokens(vec![
                var_utf8("/", ".", "bar"),
            ]),
        ),
    ]
}

#[test]
fn testCompile() -> TestResult {
    for case in provide_compile_data() {
        let name = case.name;
        let mut route = case.route?;

        let compiled = route.compile()?;
        assert_eq!(
            PhpString::from(case.prefix),
            *compiled.static_prefix(),
            "{name} (static prefix)"
        );
        assert_eq!(
            PhpString::from(case.regex),
            *compiled.regex(),
            "{name} (regex)"
        );
        assert_eq!(case.variables, *compiled.variables(), "{name} (variables)");
        assert_eq!(case.tokens, *compiled.tokens(), "{name} (tokens)");
    }
    Ok(())
}

#[test]
fn testCompileImplicitUtf8Data() -> TestResult {
    // provideCompileImplicitUtf8Data(): every case expects a LogicException
    let cases: Vec<(&str, Result<Route, Error>, bool)> = vec![
        (
            "Static UTF-8 route",
            route("/foé".as_bytes(), none(), none(), none()),
            false,
        ),
        (
            "Route with an implicit UTF-8 requirement",
            route(
                b"/{bar}",
                values(vec![("bar", Value::Null)]),
                strings(&[("bar", "é")]),
                none(),
            ),
            false,
        ),
        (
            "Route with a UTF-8 class requirement",
            route(
                b"/{bar}",
                values(vec![("bar", Value::Null)]),
                strings(&[("bar", r"\pM")]),
                none(),
            ),
            false,
        ),
        (
            "Route with a UTF-8 separator",
            route(
                "/foo/{bar}§{_format}".as_bytes(),
                none(),
                none(),
                strings(&[("compiler_class", Utf8RouteCompiler::CLASS)]),
            ),
            true,
        ),
    ];
    for (name, route, utf8_compiler) in cases {
        let result = route.and_then(|mut route| {
            if utf8_compiler {
                route.compile_with::<Utf8RouteCompiler>().map(|_| ())
            } else {
                route.compile().map(|_| ())
            }
        });
        assert!(
            result.as_ref().is_err_and(|e| e.is_a("LogicException")),
            "{name}: {result:?}"
        );
    }
    Ok(())
}

#[test]
fn testRouteWithSameVariableTwice() -> TestResult {
    let mut route = Route::new("/{name}/{name}")?;

    assert!(route.compile().is_err_and(|e| e.is_a("LogicException")));
    Ok(())
}

#[test]
fn testRouteCharsetMismatch() -> TestResult {
    let mut route = route(
        b"/\xE9/{bar}",
        none(),
        strings(&[("bar", ".")]),
        values(vec![("utf8", Value::Bool(true))]),
    )?;

    assert!(route.compile().is_err_and(|e| e.is_a("LogicException")));
    Ok(())
}

#[test]
fn testRequirementCharsetMismatch() -> TestResult {
    let mut route = route(
        b"/foo/{bar}",
        none(),
        values(vec![("bar", Value::String(PhpString::from(&b"\xE9"[..])))]),
        values(vec![("utf8", Value::Bool(true))]),
    )?;

    assert!(route.compile().is_err_and(|e| e.is_a("LogicException")));
    Ok(())
}

#[test]
fn testRouteWithFragmentAsPathParameter() -> TestResult {
    let mut route = Route::new("/{_fragment}")?;

    assert!(
        route
            .compile()
            .is_err_and(|e| e.is_a("InvalidArgumentException"))
    );
    Ok(())
}

#[test]
fn testRouteWithVariableNameStartingWithADigit() -> TestResult {
    // getVariableNamesStartingWithADigit()
    for name in ["09", "123", "1e2"] {
        let mut route = Route::new(format!("/{{{name}}}"))?;
        assert!(
            route.compile().is_err_and(|e| e.is_a("DomainException")),
            "{name}"
        );
    }
    Ok(())
}

struct HostCase {
    name: &'static str,
    path: &'static str,
    defaults: PhpArray,
    host: &'static str,
    prefix: &'static str,
    regex: &'static str,
    variables: PhpArray,
    path_variables: PhpArray,
    tokens: PhpArray,
    host_regex: &'static str,
    host_variables: PhpArray,
    host_tokens: PhpArray,
}

fn provide_compile_with_host_data() -> Vec<HostCase> {
    vec![
        HostCase {
            name: "Route with host pattern",
            path: "/hello",
            defaults: none(),
            host: "www.example.com",
            prefix: "/hello",
            regex: "{^/hello$}sD",
            variables: none(),
            path_variables: none(),
            tokens: tokens(vec![text(b"/hello")]),
            host_regex: r"{^www\.example\.com$}sDi",
            host_variables: none(),
            host_tokens: tokens(vec![text(b"www.example.com")]),
        },
        HostCase {
            name: "Route with host pattern and some variables",
            path: "/hello/{name}",
            defaults: none(),
            host: "www.example.{tld}",
            prefix: "/hello",
            regex: "{^/hello/(?P<name>[^/]++)$}sD",
            variables: list(&["tld", "name"]),
            path_variables: list(&["name"]),
            tokens: tokens(vec![var("/", "[^/]++", "name"), text(b"/hello")]),
            host_regex: r"{^www\.example\.(?P<tld>[^\.]++)$}sDi",
            host_variables: list(&["tld"]),
            host_tokens: tokens(vec![var(".", r"[^\.]++", "tld"), text(b"www.example")]),
        },
        HostCase {
            name: "Route with variable at beginning of host",
            path: "/hello",
            defaults: none(),
            host: "{locale}.example.{tld}",
            prefix: "/hello",
            regex: "{^/hello$}sD",
            variables: list(&["locale", "tld"]),
            path_variables: none(),
            tokens: tokens(vec![text(b"/hello")]),
            host_regex: r"{^(?P<locale>[^\.]++)\.example\.(?P<tld>[^\.]++)$}sDi",
            host_variables: list(&["locale", "tld"]),
            host_tokens: tokens(vec![
                var(".", r"[^\.]++", "tld"),
                text(b".example"),
                var("", r"[^\.]++", "locale"),
            ]),
        },
        HostCase {
            name: "Route with host variables that has a default value",
            path: "/hello",
            defaults: strings(&[("locale", "a"), ("tld", "b")]),
            host: "{locale}.example.{tld}",
            prefix: "/hello",
            regex: "{^/hello$}sD",
            variables: list(&["locale", "tld"]),
            path_variables: none(),
            tokens: tokens(vec![text(b"/hello")]),
            host_regex: r"{^(?P<locale>[^\.]++)\.example\.(?P<tld>[^\.]++)$}sDi",
            host_variables: list(&["locale", "tld"]),
            host_tokens: tokens(vec![
                var(".", r"[^\.]++", "tld"),
                text(b".example"),
                var("", r"[^\.]++", "locale"),
            ]),
        },
    ]
}

/// `str_replace(["\n", ' '], '', $s)`.
fn squeeze(s: &[u8]) -> PhpString {
    PhpString::from(
        s.iter()
            .copied()
            .filter(|&c| c != b'\n' && c != b' ')
            .collect::<Vec<_>>(),
    )
}

#[test]
fn testCompileWithHost() -> TestResult {
    for case in provide_compile_with_host_data() {
        let name = case.name;
        let mut route = Route::with(
            case.path,
            Arguments {
                defaults: case.defaults,
                host: s(case.host),
                ..Arguments::default()
            },
        )?;

        let compiled = route.compile()?;
        assert_eq!(
            s(case.prefix),
            *compiled.static_prefix(),
            "{name} (static prefix)"
        );
        assert_eq!(s(case.regex), squeeze(compiled.regex()), "{name} (regex)");
        assert_eq!(case.variables, *compiled.variables(), "{name} (variables)");
        assert_eq!(
            case.path_variables,
            *compiled.path_variables(),
            "{name} (path variables)"
        );
        assert_eq!(case.tokens, *compiled.tokens(), "{name} (tokens)");
        assert_eq!(
            s(case.host_regex),
            squeeze(compiled.host_regex().map_or(&b""[..], |r| r.as_bytes())),
            "{name} (host regex)"
        );
        assert_eq!(
            case.host_variables,
            *compiled.host_variables(),
            "{name} (host variables)"
        );
        assert_eq!(
            case.host_tokens,
            *compiled.host_tokens(),
            "{name} (host tokens)"
        );
    }
    Ok(())
}

#[test]
fn testRouteWithTooLongVariableName() -> TestResult {
    let mut route = Route::new(format!(
        "/{{{}}}",
        "a".repeat(RouteCompiler::VARIABLE_MAXIMUM_LENGTH + 1)
    ))?;

    assert!(route.compile().is_err_and(|e| e.is_a("DomainException")));
    Ok(())
}

#[test]
fn testRemoveCapturingGroup() -> TestResult {
    // provideRemoveCapturingGroup()
    let cases = [
        ("{^/(?P<foo>a(?:b|c)(?:d|e)f)$}sD", "a(b|c)(d|e)f"),
        (r"{^/(?P<foo>a\(b\)c)$}sD", r"a\(b\)c"),
        ("{^/(?P<foo>(?:b))$}sD", "(?:b)"),
        ("{^/(?P<foo>(?(b)b))$}sD", "(?(b)b)"),
        ("{^/(?P<foo>(*F))$}sD", "(*F)"),
        ("{^/(?P<foo>(?:(?:foo)))$}sD", "((foo))"),
    ];
    for (regex, requirement) in cases {
        let mut route = route(b"/{foo}", none(), strings(&[("foo", requirement)]), none())?;

        assert_eq!(s(regex), *route.compile()?.regex());
    }
    Ok(())
}
