//! Tests/EnvVarProcessorTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's; a data provider's rows run in a
//! loop. `Container` and a compiled `ContainerBuilder` are a small test
//! container (`Container.php` is ported elsewhere): its parameters are
//! what the tests set. `$_ENV` is a symfony-dotenv `Globals` passed to the
//! processor, the `RewindableGenerator` of loaders a test `EnvVarLoaders`,
//! and the constants and enums a php-value `Registry`.
//!
//! - testRequireFile: n/a, `require:` would run a PHP file (refused).
//! - testGetEnvShuffle: pending, the order for `srand(2)` needs PHP's
//!   Mt19937 `shuffle()`, which php-value does not have.
//!   `shuffle_keeps_the_values` checks that the values are kept.
//!
//! Upstream: symfony/dependency-injection Tests/EnvVarProcessorTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::type_complexity
)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use php_value::constants::Registry;
use php_value::object::{EnumCase, StringableObject};
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::exception::ParameterNotFound;
use symfony_dependency_injection::{
    ContainerInterface, EnvVarLoaderInterface, EnvVarLoaders, EnvVarProcessor,
    EnvVarProcessorInterface, Error, ErrorKind, InvalidBehavior, Result,
};
use symfony_dotenv::Globals;

/// A compiled container's parameters.
#[derive(Default)]
struct TestContainer {
    parameters: Mutex<BTreeMap<String, Value>>,
}

impl TestContainer {
    fn with(parameters: &[(&str, Value)]) -> Arc<Self> {
        let container = Self::default();
        for (name, value) in parameters {
            container.set_parameter(name, value.clone()).unwrap();
        }
        Arc::new(container)
    }
}

impl ContainerInterface for TestContainer {
    fn set(&self, _id: &str, _service: Option<Value>) -> Result<()> {
        Ok(())
    }

    fn get(&self, id: &str, _invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        Err(Error::service_not_found(id, None, Vec::new(), None))
    }

    fn has(&self, _id: &str) -> bool {
        false
    }

    fn initialized(&self, _id: &str) -> bool {
        false
    }

    fn get_parameter(&self, name: &str) -> Result<Value> {
        self.parameters
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| Error::parameter_not_found(ParameterNotFound::new(name)))
    }

    fn has_parameter(&self, name: &str) -> bool {
        self.parameters.lock().unwrap().contains_key(name)
    }

    fn set_parameter(&self, name: &str, value: Value) -> Result<()> {
        self.parameters
            .lock()
            .unwrap()
            .insert(name.to_owned(), value);
        Ok(())
    }
}

fn globals() -> Arc<RwLock<Globals>> {
    Arc::new(RwLock::new(Globals::new()))
}

fn set_env(globals: &Arc<RwLock<Globals>>, name: &str, value: Value) {
    globals.write().unwrap().env.insert(name, value);
}

/// `new EnvVarProcessor(new Container())`, with empty superglobals.
fn processor() -> EnvVarProcessor {
    EnvVarProcessor::new(TestContainer::with(&[]), None).with_globals(globals())
}

fn processor_with(parameters: &[(&str, Value)]) -> EnvVarProcessor {
    EnvVarProcessor::new(TestContainer::with(parameters), None).with_globals(globals())
}

fn s(text: &str) -> Value {
    Value::from(text)
}

fn fail(_: &str) -> Result<Value> {
    panic!("Should not be called");
}

fn noop(_: &str) -> Result<Value> {
    Ok(Value::Null)
}

/// `$this->expectException($kind); $this->expectExceptionMessage($message)`.
fn assert_error(result: Result<Value>, kind: &ErrorKind, message: &str) {
    let error = result.unwrap_err();
    assert!(
        error.is(kind),
        "{} is not a {}",
        error.class(),
        kind.class()
    );
    assert!(
        error.message().contains(message),
        "{:?} does not contain {message:?}",
        error.message()
    );
}

/// `fn ($name) => $value`, asserting the name.
fn returns(expected_name: &'static str, value: Value) -> impl Fn(&str) -> Result<Value> {
    move |name| {
        assert_eq!(expected_name, name);
        Ok(value.clone())
    }
}

#[test]
fn testGetEnvString() {
    for (value, processed) in [
        ("hello", "hello"),
        ("true", "true"),
        ("false", "false"),
        ("null", "null"),
        ("1", "1"),
        ("0", "0"),
        ("1.1", "1.1"),
        ("1e1", "1e1"),
    ] {
        let processor = processor_with(&[("env(foo)", s(value))]);
        let result = processor.get_env("string", "foo", &fail).unwrap();
        assert_eq!(s(processed), result);
    }
}

#[test]
fn testGetEnvRealEnv() {
    for (value, processed) in [
        (s("hello"), "hello"),
        (Value::Bool(true), "1"),
        (Value::Bool(false), ""),
        (Value::Int(1), "1"),
        (Value::Int(0), "0"),
        (Value::Float(1.1), "1.1"),
        (Value::Int(10), "10"),
    ] {
        let globals = globals();
        set_env(&globals, "FOO", value);
        let processor = EnvVarProcessor::new(TestContainer::with(&[]), None).with_globals(globals);
        let result = processor.get_env("string", "FOO", &fail).unwrap();
        assert_eq!(s(processed), result);
    }
}

#[test]
fn testGetEnvRealEnvInvalid() {
    let globals = globals();
    set_env(&globals, "FOO", Value::Null);
    let processor = EnvVarProcessor::new(TestContainer::with(&[]), None).with_globals(globals);
    assert_error(
        processor.get_env("string", "FOO", &fail),
        &ErrorKind::EnvNotFound,
        "Environment variable not found: \"FOO\".",
    );
}

#[test]
fn testGetEnvRealEnvNonScalar() {
    let globals = globals();
    set_env(&globals, "FOO", Value::Array(PhpArray::new()));
    let processor = EnvVarProcessor::new(TestContainer::with(&[]), None).with_globals(globals);
    assert_error(
        processor.get_env("string", "FOO", &fail),
        &ErrorKind::Runtime,
        "Non-scalar env var \"FOO\" cannot be cast to \"string\".",
    );
}

fn valid_bools() -> [(&'static str, bool); 8] {
    [
        ("true", true),
        ("false", false),
        ("null", false),
        ("", false),
        ("1", true),
        ("0", false),
        ("1.1", true),
        ("1e1", true),
    ]
}

#[test]
fn testGetEnvBool() {
    for (value, processed) in valid_bools() {
        let result = processor().get_env("bool", "foo", &returns("foo", s(value)));
        assert_eq!(Value::Bool(processed), result.unwrap(), "{value}");
    }
}

/// A loader returning what `vars` holds when it is called
/// (`$GLOBALS['ENV_FOO']`).
struct VarsLoader {
    vars: Arc<Mutex<PhpArray>>,
}

impl EnvVarLoaderInterface for VarsLoader {
    fn load_env_vars(&self) -> Result<PhpArray> {
        Ok(self.vars.lock().unwrap().clone())
    }
}

/// `new RewindableGenerator($generator, $count)`: each iteration calls the
/// generator again.
struct Generator {
    generator: Box<dyn Fn() -> Vec<Result<Arc<dyn EnvVarLoaderInterface>>> + Send + Sync>,
    count: usize,
}

impl EnvVarLoaders for Generator {
    fn loaders(&self) -> Box<dyn Iterator<Item = Result<Arc<dyn EnvVarLoaderInterface>>> + '_> {
        Box::new((self.generator)().into_iter())
    }

    fn count(&self) -> Option<usize> {
        Some(self.count)
    }
}

fn caching_processor() -> (EnvVarProcessor, Arc<Mutex<PhpArray>>) {
    let globals = globals();
    set_env(&globals, "FOO", s(""));
    let vars = Arc::new(Mutex::new(php_array!["FOO" => "value"]));
    let loader_vars = vars.clone();
    let loaders = Generator {
        generator: Box::new(move || {
            vec![Ok(Arc::new(VarsLoader {
                vars: loader_vars.clone(),
            }) as Arc<dyn EnvVarLoaderInterface>)]
        }),
        count: 1,
    };
    let processor = EnvVarProcessor::new(TestContainer::with(&[]), Some(Arc::new(loaders)))
        .with_globals(globals);
    (processor, vars)
}

#[test]
fn testGetEnvCachesEnv() {
    let (processor, vars) = caching_processor();

    let result = processor.get_env("string", "FOO", &noop).unwrap();
    assert_eq!(s("value"), result);

    *vars.lock().unwrap() = php_array!["FOO" => "new value"];

    let result = processor.get_env("string", "FOO", &noop).unwrap();
    assert_eq!(s("value"), result);
}

#[test]
fn testReset() {
    let (processor, vars) = caching_processor();

    let result = processor.get_env("string", "FOO", &noop).unwrap();
    assert_eq!(s("value"), result);

    *vars.lock().unwrap() = php_array!["FOO" => "new value"];

    processor.reset();

    let result = processor.get_env("string", "FOO", &noop).unwrap();
    assert_eq!(s("new value"), result);
}

#[test]
fn testGetEnvNot() {
    for (value, processed) in valid_bools() {
        let result = processor().get_env("not", "foo", &returns("foo", s(value)));
        assert_eq!(Value::Bool(!processed), result.unwrap(), "{value}");
    }
}

#[test]
fn testGetEnvInt() {
    for (value, processed) in [("1", 1), ("1.1", 1), ("1e1", 10)] {
        let result = processor().get_env("int", "foo", &returns("foo", s(value)));
        assert_eq!(Value::Int(processed), result.unwrap());
    }
}

#[test]
fn testGetEnvIntInvalid() {
    for value in ["foo", "true", "null"] {
        assert_error(
            processor().get_env("int", "foo", &returns("foo", s(value))),
            &ErrorKind::Runtime,
            "Non-numeric env var",
        );
    }
}

#[test]
fn testGetEnvFloat() {
    for (value, processed) in [("1", 1.0), ("1.1", 1.1), ("1e1", 10.0)] {
        let result = processor().get_env("float", "foo", &returns("foo", s(value)));
        assert_eq!(Value::Float(processed), result.unwrap());
    }
}

#[test]
fn testGetEnvFloatInvalid() {
    for value in ["foo", "true", "null"] {
        assert_error(
            processor().get_env("float", "foo", &returns("foo", s(value))),
            &ErrorKind::Runtime,
            "Non-numeric env var",
        );
    }
}

const TEST_CONST: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\EnvVarProcessorTest::TEST_CONST";

/// The constants and enums the test class and its fixtures define.
fn constants() -> Registry {
    let mut registry = Registry::with_core();
    registry.define(TEST_CONST, s("test"));
    registry.define("E_ERROR", Value::Int(1));
    registry.define_enum(
        STRING_BACKED_ENUM,
        vec![EnumCase::new(STRING_BACKED_ENUM, "Bar", Some(s("bar")))],
    );
    registry.define_enum(
        INT_BACKED_ENUM,
        vec![EnumCase::new(INT_BACKED_ENUM, "Nine", Some(Value::Int(9)))],
    );
    registry
}

#[test]
fn testGetEnvConst() {
    for (value, processed) in [(TEST_CONST, s("test")), ("E_ERROR", Value::Int(1))] {
        let processor = processor().with_constants(constants());
        let result = processor.get_env("const", "foo", &returns("foo", s(value)));
        assert_eq!(processed, result.unwrap());
    }
}

#[test]
fn testGetEnvConstInvalid() {
    for value in [
        "Symfony\\Component\\DependencyInjection\\Tests\\EnvVarProcessorTest::UNDEFINED_CONST",
        "UNDEFINED_CONST",
    ] {
        let processor = processor().with_constants(constants());
        assert_error(
            processor.get_env("const", "foo", &returns("foo", s(value))),
            &ErrorKind::Runtime,
            "undefined constant",
        );
    }
}

#[test]
fn testGetEnvBase64() {
    let processor = processor();

    // base64_encode('hello')
    let result = processor.get_env("base64", "foo", &returns("foo", s("aGVsbG8=")));
    assert_eq!(s("hello"), result.unwrap());

    let result = processor.get_env("base64", "foo", &|_| Ok(s("/+0=")));
    assert_eq!(
        Value::String(b"\xFF\xED".as_slice().into()),
        result.unwrap()
    );

    let result = processor.get_env("base64", "foo", &|_| Ok(s("_-0=")));
    assert_eq!(
        Value::String(b"\xFF\xED".as_slice().into()),
        result.unwrap()
    );
}

#[test]
fn testGetEnvTrim() {
    let result = processor().get_env("trim", "foo", &returns("foo", s(" hello\n")));
    assert_eq!(s("hello"), result.unwrap());
}

#[test]
fn testGetEnvJson() {
    for (value, processed) in [
        (s("[1]"), Value::Array(php_array![0 => 1])),
        (
            s("{\"key\": \"value\"}"),
            Value::Array(php_array!["key" => "value"]),
        ),
        (Value::Null, Value::Null),
    ] {
        let result = processor().get_env("json", "foo", &returns("foo", value));
        assert_eq!(processed, result.unwrap());
    }
}

#[test]
fn testGetEnvInvalidJson() {
    assert_error(
        processor().get_env("json", "foo", &returns("foo", s("invalid_json"))),
        &ErrorKind::Runtime,
        "Syntax error",
    );
}

#[test]
fn testGetEnvJsonOther() {
    // json_encode() of 1, 1.1, true, false and 'foo'.
    for value in ["1", "1.1", "true", "false", "\"foo\""] {
        assert_error(
            processor().get_env("json", "foo", &returns("foo", s(value))),
            &ErrorKind::Runtime,
            "Invalid JSON env var",
        );
    }
}

#[test]
fn testGetEnvUnknown() {
    assert_error(
        processor().get_env("unknown", "foo", &returns("foo", s("foo"))),
        &ErrorKind::Runtime,
        "Unsupported env var prefix",
    );
}

#[test]
fn testGetEnvKeyInvalidKey() {
    assert_error(
        processor().get_env("key", "foo", &|_| panic!("Should not get here")),
        &ErrorKind::Runtime,
        "Invalid env \"key:foo\": a key specifier should be provided.",
    );
}

#[test]
fn testGetEnvKeyNoArrayResult() {
    for value in [Value::Null, s("string"), Value::Int(1), Value::Bool(true)] {
        assert_error(
            processor().get_env("key", "index:foo", &returns("foo", value)),
            &ErrorKind::Runtime,
            "Resolved value of \"foo\" did not result in an array value.",
        );
    }
}

#[test]
fn testGetEnvKeyArrayKeyNotFound() {
    for value in [
        PhpArray::new(),
        php_array!["index2" => "value"],
        php_array![0 => "index", 1 => "index2"],
    ] {
        assert_error(
            processor().get_env("key", "index:foo", &returns("foo", Value::Array(value))),
            &ErrorKind::EnvNotFound,
            "Key \"index\" not found in",
        );
    }
}

#[test]
fn testGetEnvKey() {
    for index in [
        s("password"),
        s("true"),
        Value::Bool(false),
        s("1"),
        Value::Int(1),
        s("1.1"),
        Value::Float(1.1),
        Value::Array(PhpArray::new()),
        Value::Array(php_array![0 => "val1", 1 => "val2"]),
    ] {
        let mut value = PhpArray::new();
        value.insert("index", index.clone());
        let result = processor().get_env("key", "index:foo", &returns("foo", Value::Array(value)));
        assert_eq!(index, result.unwrap());
    }
}

#[test]
fn testGetEnvKeyChained() {
    let result = processor().get_env(
        "key",
        "index:file:foo",
        &returns("file:foo", Value::Array(php_array!["index" => "password"])),
    );
    assert_eq!(s("password"), result.unwrap());
}

const STRING_BACKED_ENUM: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\StringBackedEnum";
const INT_BACKED_ENUM: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\IntBackedEnum";

#[test]
fn testGetEnvEnum() {
    for (class, case, value) in [
        (STRING_BACKED_ENUM, "Bar", s("bar")),
        (INT_BACKED_ENUM, "Nine", Value::Int(9)),
    ] {
        let processor = processor().with_constants(constants());
        let name = format!("{class}:foo");
        let result = processor.get_env("enum", &name, &returns("foo", value.clone()));
        let expected = Value::Object(EnumCase::new(class, case, Some(value)).object());
        assert_eq!(expected, result.unwrap());
    }
}

#[test]
fn testGetEnvEnumInvalidEnum() {
    assert_error(
        processor().get_env("enum", "foo", &|_| panic!("Should not get here")),
        &ErrorKind::Runtime,
        "Invalid env \"enum:foo\": a \"BackedEnum\" class-string should be provided.",
    );
}

#[test]
fn testGetEnvEnumInvalidResolvedValue() {
    let processor = processor().with_constants(constants());
    assert_error(
        processor.get_env("enum", &format!("{STRING_BACKED_ENUM}:foo"), &|_| {
            Ok(Value::Null)
        }),
        &ErrorKind::Runtime,
        "Resolved value of \"foo\" did not result in a string or int value.",
    );
}

#[test]
fn testGetEnvEnumInvalidArg() {
    let processor = processor().with_constants(constants());
    assert_error(
        processor.get_env("enum", "bogus:foo", &|_| Ok(s(""))),
        &ErrorKind::Runtime,
        "\"bogus\" is not a \"BackedEnum\".",
    );
}

#[test]
fn testGetEnvEnumInvalidBackedValue() {
    let processor = processor().with_constants(constants());
    assert_error(
        processor.get_env("enum", &format!("{STRING_BACKED_ENUM}:foo"), &|_| {
            Ok(s("bogus"))
        }),
        &ErrorKind::Runtime,
        &format!("Enum value \"bogus\" is not backed by \"{STRING_BACKED_ENUM}\"."),
    );
}

#[test]
fn testGetEnvNullable() {
    for (value, processed) in [
        ("hello", s("hello")),
        ("", Value::Null),
        ("null", s("null")),
        ("Null", s("Null")),
        ("NULL", s("NULL")),
    ] {
        let result = processor().get_env("default", ":foo", &returns("foo", s(value)));
        assert_eq!(processed, result.unwrap());
    }
}

#[test]
fn testRequireMissingFile() {
    assert_error(
        processor().get_env("require", "/missing-file", &|name| Ok(s(name))),
        &ErrorKind::EnvNotFound,
        "missing-file",
    );
}

#[test]
fn testGetEnvResolve() {
    for (value, processed) in [
        (s("string"), "string"),
        (Value::Int(1), "1"),
        (Value::Float(1.1), "1.1"),
        (Value::Bool(true), "1"),
        (Value::Bool(false), ""),
    ] {
        let processor = processor_with(&[("bar", value)]);
        let result = processor.get_env("resolve", "foo", &|_| Ok(s("%bar%")));
        assert_eq!(s(processed), result.unwrap());
    }
}

#[test]
fn testGetEnvResolveNoMatch() {
    let result = processor().get_env("resolve", "foo", &|_| Ok(s("%%")));
    assert_eq!(s("%"), result.unwrap());
}

#[test]
fn testGetEnvResolveNotScalar() {
    for value in [Value::Null, Value::Array(PhpArray::new())] {
        let processor = processor_with(&[("bar", value)]);
        assert_error(
            processor.get_env("resolve", "foo", &|_| Ok(s("%bar%"))),
            &ErrorKind::Runtime,
            "Parameter \"bar\" found when resolving env var \"foo\" must be scalar",
        );
    }
}

#[test]
fn testGetEnvResolveNestedEnv() {
    let processor = processor_with(&[("env(BAR)", s("BAR in container"))]);
    let get_env = |name: &str| {
        if name == "foo" {
            Ok(s("%env(BAR)%"))
        } else {
            processor.get_env("string", name, &noop)
        }
    };
    let result = processor.get_env("resolve", "foo", &get_env);
    assert_eq!(s("BAR in container"), result.unwrap());
}

#[test]
fn testGetEnvResolveNestedRealEnv() {
    let globals = globals();
    set_env(&globals, "BAR", s("BAR in environment"));
    let processor = EnvVarProcessor::new(
        TestContainer::with(&[("env(BAR)", s("BAR in container"))]),
        None,
    )
    .with_globals(globals);
    let get_env = |name: &str| {
        if name == "foo" {
            Ok(s("%env(BAR)%"))
        } else {
            processor.get_env("string", name, &noop)
        }
    };
    let result = processor.get_env("resolve", "foo", &get_env);
    assert_eq!(s("BAR in environment"), result.unwrap());
}

fn list(values: &[&str]) -> Value {
    Value::Array(PhpArray::list(values.iter().map(|v| s(v))))
}

#[test]
fn testGetEnvCsv() {
    let complex = ",\"\"\"\",\"foo\"\"\",\"\\\"\"\",\\,foo\\";
    for (value, processed) in [
        (s(""), Value::Array(PhpArray::new())),
        (s(","), list(&["", ""])),
        (s("1"), list(&["1"])),
        (s("1,2,\" 3 \""), list(&["1", "2", " 3 "])),
        (s("\\,\\\\"), list(&["\\", "\\\\"])),
        (
            s(complex),
            list(&["", "\"", "foo\"", "\\\"", "\\", "foo\\"]),
        ),
        (Value::Null, Value::Null),
    ] {
        let result = processor().get_env("csv", "foo", &returns("foo", value.clone()));
        assert_eq!(processed, result.unwrap(), "{value:?}");
    }
}

/// Not upstream: what testGetEnvShuffle checks apart from the seeded
/// order (pending).
#[test]
fn shuffle_keeps_the_values() {
    let result = processor()
        .get_env("shuffle", "", &|_| Ok(list(&["foo", "bar"])))
        .unwrap();
    let Value::Array(result) = result else {
        panic!("not an array");
    };
    assert!(result.is_list());
    let mut values: Vec<String> = result
        .values()
        .map(|v| String::from_utf8_lossy(v.as_string().unwrap().as_bytes()).into_owned())
        .collect();
    values.sort();
    assert_eq!(vec!["bar", "foo"], values);
}

#[test]
fn testGetEnvShuffleInvalid() {
    assert_error(
        processor().get_env("shuffle", "foo", &|_| Ok(s("bar"))),
        &ErrorKind::Runtime,
        "Env var \"foo\" cannot be shuffled, expected array, got \"string\".",
    );
}

struct ArrayLoader(PhpArray);

impl EnvVarLoaderInterface for ArrayLoader {
    fn load_env_vars(&self) -> Result<PhpArray> {
        Ok(self.0.clone())
    }
}

#[test]
fn testEnvLoader() {
    let globals = globals();
    set_env(&globals, "BAZ_ENV_LOADER", s(""));
    set_env(&globals, "BUZ_ENV_LOADER", s(""));

    let loaders = Generator {
        generator: Box::new(|| {
            let mut first = php_array!["FOO_ENV_LOADER" => "123", "BAZ_ENV_LOADER" => ""];
            first.insert(
                "LAZY_ENV_LOADER",
                StringableObject::object("class@anonymous", ""),
            );
            let mut second = php_array![
                "FOO_ENV_LOADER" => "234",
                "BAR_ENV_LOADER" => "456",
                "BAZ_ENV_LOADER" => "567",
            ];
            second.insert(
                "LAZY_ENV_LOADER",
                StringableObject::object("class@anonymous", "678"),
            );
            vec![
                Ok(Arc::new(ArrayLoader(first)) as Arc<dyn EnvVarLoaderInterface>),
                Ok(Arc::new(ArrayLoader(second)) as Arc<dyn EnvVarLoaderInterface>),
            ]
        }),
        count: 2,
    };
    let processor = EnvVarProcessor::new(TestContainer::with(&[]), Some(Arc::new(loaders)))
        .with_globals(globals);

    let result = processor
        .get_env("string", "FOO_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("123"), result);

    let result = processor
        .get_env("string", "BAR_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("456"), result);

    let result = processor
        .get_env("string", "BAZ_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("567"), result);

    let result = processor
        .get_env("string", "BUZ_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s(""), result);

    let result = processor
        .get_env("string", "FOO_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("123"), result); // check twice

    let result = processor
        .get_env("string", "LAZY_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("678"), result);
}

#[test]
fn testCircularEnvLoader() {
    let index = Arc::new(AtomicUsize::new(0));
    let counter = index.clone();
    let loaders = Generator {
        generator: Box::new(move || {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                return vec![Err(Error::parameter_circular_reference(vec![
                    "FOO_CONTAINER".to_owned(),
                ]))];
            }
            vec![Ok(
                Arc::new(ArrayLoader(php_array!["FOO_ENV_LOADER" => "123"]))
                    as Arc<dyn EnvVarLoaderInterface>,
            )]
        }),
        count: 1,
    };
    let processor = EnvVarProcessor::new(
        TestContainer::with(&[("env(FOO_CONTAINER)", s("foo"))]),
        Some(Arc::new(loaders)),
    )
    .with_globals(globals());

    let result = processor.get_env("string", "FOO_CONTAINER", &noop).unwrap();
    assert_eq!(s("foo"), result);

    let result = processor
        .get_env("string", "FOO_ENV_LOADER", &noop)
        .unwrap();
    assert_eq!(s("123"), result);

    let result = processor
        .get_env("default", ":BAR_CONTAINER", &|name| {
            assert_eq!("BAR_CONTAINER", name);
            processor.get_env("string", name, &noop)
        })
        .unwrap();
    assert_eq!(Value::Null, result);

    assert_eq!(2, index.load(Ordering::SeqCst));
}

#[test]
fn testGetEnvInvalidPrefixWithDefault() {
    assert_error(
        processor().get_env(
            "unknown",
            "default::FAKE",
            &returns("default::FAKE", Value::Null),
        ),
        &ErrorKind::Runtime,
        "Unsupported env var prefix",
    );
}

#[test]
fn testGetEnvUrlPath() {
    for (expected, url) in [
        ("", "https://symfony.com"),
        ("", "https://symfony.com/"),
        ("/", "https://symfony.com//"),
        ("blog", "https://symfony.com/blog"),
        ("blog/", "https://symfony.com/blog/"),
        ("blog//", "https://symfony.com/blog//"),
    ] {
        let result = processor().get_env("url", "foo", &|_| Ok(s(url))).unwrap();
        let path = result.as_array().unwrap().get(&"path".into()).cloned();
        assert_eq!(Some(s(expected)), path, "{url}");
    }
}

#[test]
fn testGetEnvBadUrl() {
    for url in [
        "http://foo.com\\bar",
        "\\\\foo.com/bar",
        "a\rb",
        "a\nb",
        "a\tb",
        "\u{0}foo",
        "foo\u{0}",
        " foo",
        "foo ",
        ":",
    ] {
        let result = processor().get_env("url", "foo", &|_| Ok(s(url)));
        assert!(result.unwrap_err().is(&ErrorKind::Runtime), "{url:?}");
    }
}

#[test]
fn testGetEnvCastsNullBehavior() {
    for (expected, prefix) in [
        (s(""), "string"),
        (Value::Null, ""),
        (Value::Bool(false), "bool"),
        (Value::Bool(true), "not"),
        (Value::Int(0), "int"),
        (Value::Float(0.0), "float"),
    ] {
        let processor = processor();
        let result = processor.get_env(prefix, "default::FOO", &|_| {
            processor.get_env("default", ":FOO", &|_| Ok(Value::Null))
        });
        assert_eq!(expected, result.unwrap(), "{prefix}");
    }
}

#[test]
fn testGetEnvWithEmptyStringPrefixCastsToString() {
    let globals = globals();
    set_env(&globals, "FOO", Value::Int(4));
    let processor = EnvVarProcessor::new(TestContainer::with(&[]), None).with_globals(globals);
    assert_eq!(s("4"), processor.get_env("", "FOO", &fail).unwrap());
}

#[test]
fn testGetEnvDefined() {
    let not_found = |_: &str| Err(Error::env_not_found(""));
    let cases: [(&str, bool, &dyn Fn(&str) -> Result<Value>); 5] = [
        ("Defined", true, &|_| Ok(s("foo"))),
        ("Falsy but defined", true, &|_| Ok(s("0"))),
        ("Empty string", false, &|_| Ok(s(""))),
        ("Null", false, &|_| Ok(Value::Null)),
        ("Env var not defined", false, &not_found),
    ];
    for (label, expected, callback) in cases {
        let result = processor().get_env("defined", "NO_SOMETHING", callback);
        assert_eq!(Value::Bool(expected), result.unwrap(), "{label}");
    }
}

#[test]
fn testGetEnvUrlencode() {
    let result = processor().get_env("urlencode", "URLENCODETEST", &|_| {
        Ok(s(
            "foo: Data123!@-_ + bar: Not the same content as Data123!@-_ +",
        ))
    });
    assert_eq!(
        s(
            "foo%3A%20Data123%21%40-_%20%2B%20bar%3A%20Not%20the%20same%20content%20as%20Data123%21%40-_%20%2B"
        ),
        result.unwrap()
    );
}

#[test]
fn testQueryStringEnvVarProcessor() {
    for (label, env_value, expected) in [
        ("url_without_query", "https://example.com", PhpArray::new()),
        (
            "url_with_empty_query",
            "https://example.com?",
            PhpArray::new(),
        ),
        (
            "url_with_query",
            "https://example.com?foo=bar&baz=123",
            php_array!["foo" => "bar", "baz" => "123"],
        ),
        (
            "raw_query_string",
            "foo=bar&test=1",
            php_array!["foo" => "bar", "test" => "1"],
        ),
    ] {
        let result = processor().get_env("query_string", "MY_VAR", &|_| Ok(s(env_value)));
        assert_eq!(Value::Array(expected), result.unwrap(), "{label}");
    }
}
