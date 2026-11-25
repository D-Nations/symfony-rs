//! `ControllerResolver` against PHP 8.4.27 running the pinned
//! symfony/http-kernel (v8.1.11), recorded by
//! `oracle/recorders/http_kernel_controller_resolver.php`
//! (`scripts/record.sh http-kernel-controller-resolver`): 4,000 generated
//! scripts, each a PHP program in its own namespace (classes, abstract
//! classes, interfaces and traits; parents and interfaces; constructors of
//! every visibility, arity and outcome; methods of every visibility,
//! static or abstract, `__invoke`, `__call`, `__callStatic`; attributes;
//! functions; an anonymous class), a generated `_controller`, maybe
//! `_check_controller_is_allowed` and `allowControllers()` calls.
//!
//! The program is declared in a `ClassRegistry` from the recorded
//! declarations. Compared: the closures made of the program's callables
//! (what reflection reports of them, what they return), then the
//! resolver's answer: `false` (and the logger's warning), the callable it
//! returns (given objects and closures by identity), what calling it
//! returns and the name `ArgumentMetadataFactory` gives it, or the
//! exception (class, message, code and previous).
//!
//! `ContainerControllerResolver` against 2,000 more scripts from the same
//! recorder (`CR_MODE=container`, `scripts/record.sh
//! http-kernel-container-controller-resolver`): each also has a container,
//! a Symfony DI `Container` (services set, removed ids) or another PSR-11
//! container, whose services are the script's objects, closures or a
//! `stdClass`, and `_controller` may name one of its ids. Also compared:
//! the container's `has()`, `get()` (PSR-11 only) and `getRemovedIds()`
//! calls, in order.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use base64::Engine;
use php_value::{
    Handle, Key, PhpArray, PhpString, Value, object::StdClass, value::debug_class_name, vectors,
};
use psr_container::{ContainerInterface, Throwable as ContainerThrowable};
use psr_log::{Context, ContextValue, LogLevel, LoggerInterface};
use serde_json::{Value as Json, json};
use symfony_http_foundation::request::Request;
use symfony_http_kernel::{
    container_controller_resolver::{
        ContainerControllerResolver, ControllerContainer, DependencyInjectionContainer,
    },
    controller::{Controller, ControllerResult, Parameter},
    controller_resolver::{
        Class, ClassKind, ClassRegistry, ClosureObject, ControllerResolver, Function, Method,
        Visibility,
    },
    exception::{PhpClass, PhpException, Throwable, class_of, message_of},
};

const DATA: &str = include_str!("data/controller_resolver.json");
const CONTAINER_DATA: &str = include_str!("data/container_controller_resolver.json");

/// The recorder's `RecordingContainer` (a DI `Container`: `has()` and
/// `getRemovedIds()` recorded) or `PsrContainer` (`has()` and `get()`
/// recorded).
struct RecordingContainer {
    di: bool,
    services: HashMap<Vec<u8>, Value>,
    removed: PhpArray,
    calls: Mutex<Vec<Json>>,
}

impl ContainerInterface for RecordingContainer {
    fn get(&self, id: &[u8]) -> Result<Value, ContainerThrowable> {
        if !self.di {
            self.calls.lock().unwrap().push(json!(["get", b64(id)]));
        }
        self.services
            .get(id)
            .cloned()
            .ok_or_else(|| Arc::new(std::io::Error::other("no such service")) as ContainerThrowable)
    }

    fn has(&self, id: &[u8]) -> bool {
        self.calls.lock().unwrap().push(json!(["has", b64(id)]));
        self.services.contains_key(id)
    }
}

impl DependencyInjectionContainer for RecordingContainer {
    fn get_removed_ids(&self) -> PhpArray {
        self.calls.lock().unwrap().push(json!(["getRemovedIds"]));
        self.removed.clone()
    }
}

/// The script's container, from its recorded spec.
fn container(spec: &Json, objects: &[Value], closures: &[Value]) -> Arc<RecordingContainer> {
    let services = spec["services"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let service = &pair[1];
            let value = match service[0].as_str().unwrap() {
                "object" => objects[index(&service[1])].clone(),
                "closure" => closures[index(&service[1])].clone(),
                _ => StdClass::object(PhpArray::new()),
            };
            (unb64(&pair[0]), value)
        })
        .collect();
    let mut removed = PhpArray::new();
    for pair in spec["removed"].as_array().unwrap() {
        removed.insert(
            Key::from_bytes(&unb64(&pair[0])),
            vectors::decode(&pair[1]).unwrap(),
        );
    }
    Arc::new(RecordingContainer {
        di: spec["kind"] == "di",
        services,
        removed,
        calls: Mutex::default(),
    })
}

fn b64(bytes: &[u8]) -> Json {
    Json::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn unb64(json: &Json) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(json.as_str().unwrap())
        .unwrap()
}

fn unb64_str(json: &Json) -> String {
    String::from_utf8(unb64(json)).unwrap()
}

/// `typed_value()` of oracle/recorders/lib/typed.php.
fn typed(value: &Value) -> Json {
    match value {
        Value::Null => json!(["null"]),
        Value::Bool(b) => json!(["bool", b]),
        Value::Int(i) => json!(["int", i]),
        Value::Float(f) => {
            let hex: String = f
                .to_be_bytes()
                .iter()
                .fold(String::new(), |hex, b| hex + &format!("{b:02x}"));
            json!(["float", hex])
        }
        Value::String(s) => json!(["string", b64(s.as_bytes())]),
        Value::Array(a) => json!(["array", typed_pairs(a)]),
        Value::Object(o) => json!(["object", o.class(), typed_pairs(&o.properties())]),
    }
}

fn typed_pairs(a: &PhpArray) -> Vec<Json> {
    a.iter()
        .map(|(k, v)| json!([typed(&k.to_value()), typed(v)]))
        .collect()
}

#[derive(Default)]
struct Logger(Mutex<Vec<Json>>);

impl LoggerInterface for Logger {
    fn log(&self, level: LogLevel, message: &str, context: Context) {
        let mut array = PhpArray::new();
        for (key, value) in context {
            let value = match value {
                ContextValue::Value(v) => v,
                ContextValue::String(s) => Value::string(s),
                ContextValue::Exception(e) => Value::string(e.to_string()),
            };
            array.insert(Key::from(key), value);
        }
        self.0.lock().unwrap().push(json!([
            level.as_str(),
            b64(message.as_bytes()),
            typed_pairs(&array)
        ]));
    }
}

fn php_class(name: &str) -> PhpClass {
    match name {
        "LogicException" => PhpClass::LogicException,
        "InvalidArgumentException" => PhpClass::InvalidArgumentException,
        "RuntimeException" => PhpClass::RuntimeException,
        "Error" => PhpClass::Error,
        "BadMethodCallException" => PhpClass::BadMethodCallException,
        "TypeError" => PhpClass::TypeError,
        other => panic!("unknown class {other}"),
    }
}

fn returns(value: Value) -> Controller {
    Controller::new(move |_| Ok(ControllerResult::Value(value.clone())))
}

fn visibility(json: &Json) -> Visibility {
    match json.as_str().unwrap() {
        "public" => Visibility::Public,
        "protected" => Visibility::Protected,
        _ => Visibility::Private,
    }
}

fn strings(json: &Json) -> Vec<String> {
    json.as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect()
}

/// The recorded program, declared.
fn declare(script: &Json) -> Result<ClassRegistry, String> {
    let mut registry = ClassRegistry::new();
    let specs = script["classes"].as_array().unwrap();
    let names: Vec<String> = specs.iter().map(|c| unb64_str(&c["name"])).collect();
    for (spec, name) in specs.iter().zip(&names) {
        let kind = match spec["kind"].as_str().unwrap() {
            "class" => ClassKind::Class,
            "abstract" => ClassKind::AbstractClass,
            "interface" => ClassKind::Interface,
            _ => ClassKind::Trait,
        };
        let mut class = Class::of_kind(kind, name.as_str())
            .with_attributes(strings(&spec["attributes"]))
            .implements(
                spec["interfaces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| names[index(i)].clone()),
            );
        if spec["parent"].is_u64() {
            class = class.extends(names[index(&spec["parent"])].as_str());
        }
        if let Some(constructor) = spec.get("constructor") {
            let parameters = constructor["params"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(n, p)| match p.as_str().unwrap() {
                    "required" => Parameter::new(format!("r{n}")),
                    "optional" => Parameter::new(format!("o{n}")).with_default(Value::Int(1)),
                    _ => Parameter::new("v").variadic(),
                });
            let throws = constructor["throws"].as_str().map(php_class);
            let message = format!("Constructor of {name} failed.");
            let body = Controller::new(move |_| match throws {
                Some(class) => Err(PhpException::new(class, message.as_str())
                    .with_code(7)
                    .throwable()),
                None => Ok(ControllerResult::Value(Value::Null)),
            })
            .with_parameters(parameters);
            class = class.with_method(
                Method::new("__construct", body)
                    .with_visibility(visibility(&constructor["visibility"])),
            );
        }
        for m in spec["methods"].as_array().unwrap() {
            let method_name = m["name"].as_str().unwrap();
            let mut method = if m["abstract"].as_bool().unwrap() {
                Method::abstract_method(method_name)
            } else {
                let full = format!("{name}::{method_name}");
                let (class, method) = (name.clone(), method_name.to_owned());
                let body = if ["__call", "__callStatic"].contains(&method_name) {
                    Controller::new(move |arguments| {
                        let Some(called) = arguments.first() else {
                            return Err(PhpException::new(
                                PhpClass::ArgumentCountError,
                                format!(
                                    "Too few arguments to function {}::{method}(), 0 passed and exactly 2 expected",
                                    debug_class_name(&class)
                                ),
                            )
                            .throwable());
                        };
                        let called = called.to_php_string().unwrap();
                        let mut out = format!("{full}|").into_bytes();
                        out.extend_from_slice(called.as_bytes());
                        Ok(ControllerResult::Value(Value::String(PhpString::from(out))))
                    })
                } else {
                    returns(Value::string(full))
                };
                Method::new(method_name, body)
            };
            method = method
                .with_visibility(visibility(&m["visibility"]))
                .with_attributes(strings(&m["attributes"]));
            if m["static"].as_bool().unwrap() {
                method = method.into_static();
            }
            class = class.with_method(method);
        }
        registry.declare_class(class).map_err(|e| e.to_string())?;
    }
    for f in script["functions"].as_array().unwrap() {
        let name = f["name"].as_str().unwrap();
        registry
            .declare_function(
                Function::new(name, returns(Value::string(name)))
                    .with_attributes(strings(&f["attributes"])),
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(registry)
}

fn index(json: &Json) -> usize {
    usize::try_from(json.as_u64().unwrap()).unwrap()
}

fn class_name(script: &Json, class: &Json) -> String {
    unb64_str(&script["classes"][index(class)]["name"])
}

/// A recorded call: ["returns", typed] or ["throws", throwable].
fn call_json(result: Result<ControllerResult, Throwable>) -> Json {
    match result {
        Ok(ControllerResult::Value(v)) => json!(["returns", typed(&v)]),
        Ok(ControllerResult::Response(_)) => json!(["returns", "response"]),
        Err(e) => json!(["throws", throwable(&e)]),
    }
}

fn code_of(e: &(dyn std::error::Error + 'static)) -> i64 {
    if let Some(e) = e.downcast_ref::<PhpException>() {
        return e.get_code();
    }
    if let Some(e) = e.downcast_ref::<symfony_http_foundation::exception::Error>() {
        return e.code();
    }
    0
}

fn throwable(e: &Throwable) -> Json {
    let e: &(dyn std::error::Error + 'static) = e.as_ref();
    let previous = e.source().map_or(Json::Null, |p| {
        json!([class_of(p), b64(message_of(p).as_bytes())])
    });
    json!([
        class_of(e),
        b64(message_of(e).as_bytes()),
        code_of(e),
        previous
    ])
}

fn closure_json(closure: &Value) -> Json {
    let Value::Object(o) = closure else {
        panic!("a closure")
    };
    let c = o.downcast_ref::<ClosureObject>().unwrap();
    json!({
        "name": b64(c.name().as_bytes()),
        "class": c.called_class().map_or(Json::Null, |c| b64(c.as_bytes())),
        "anonymous": c.is_anonymous(),
        "attributes": c.attributes(),
        "pretty": b64(c.controller().name().as_bytes()),
        "call": call_json(c.controller().call(&[])),
    })
}

/// `encode()` of the recorder: given objects and closures by index.
fn encode(value: &Value, objects: &[Value], closures: &[Value]) -> Json {
    match value {
        Value::Array(a) => json!([
            "array",
            a.iter()
                .map(|(k, v)| json!([typed(&k.to_value()), encode(v, objects, closures)]))
                .collect::<Vec<_>>()
        ]),
        Value::Object(o) => {
            if let Some(i) = closures.iter().position(|c| c.identical(value)) {
                return json!(["closure", i]);
            }
            if let Some(i) = objects.iter().position(|c| c.identical(value)) {
                return json!(["object", i]);
            }
            json!(["new", b64(o.class().as_bytes())])
        }
        other => typed(other),
    }
}

fn decode_controller(spec: &Json, objects: &[Value], closures: &[Value]) -> Value {
    match spec[0].as_str().unwrap() {
        "value" => vectors::decode(&spec[1]).unwrap(),
        "object" => objects[index(&spec[1])].clone(),
        "closure" => closures[index(&spec[1])].clone(),
        "array" => {
            let mut array = PhpArray::new();
            for pair in spec[1].as_array().unwrap() {
                let key = Key::from_value(&vectors::decode(&pair[0]).unwrap()).unwrap();
                array.insert(key, decode_controller(&pair[1], objects, closures));
            }
            Value::Array(array)
        }
        "service" => Value::String(PhpString::from(unb64(&spec[1]))),
        "service_array" => Value::Array(PhpArray::list([
            Value::String(PhpString::from(unb64(&spec[1]))),
            Value::String(PhpString::from(unb64(&spec[2]))),
        ])),
        other => panic!("unknown controller {other}"),
    }
}

/// The closure PHP made of a recorded callable (`Closure::fromCallable()`),
/// or an anonymous one as recorded; `None` if the registry finds the
/// callable not callable.
fn make_closure(
    script: &Json,
    registry: &ClassRegistry,
    objects: &[Value],
    c: &Json,
) -> Option<Value> {
    let spec = &c["spec"];
    Some(match spec[0].as_str().unwrap() {
        "anonymous" => {
            let call = &c["call"];
            assert_eq!(call[0], "returns");
            let value = vectors::decode(&call[1]).unwrap();
            ClosureObject::new(
                unb64_str(&c["name"]),
                None,
                true,
                strings(&c["attributes"]),
                returns(value),
            )
            .object()
        }
        "function" => {
            let name = &script["functions"][index(&spec[1])]["name"];
            registry.closure_from_callable(&Value::string(name.as_str().unwrap()))?
        }
        "method" => {
            let object = objects[index(&spec[1])].clone();
            let callable = PhpArray::list([object, Value::string(spec[2].as_str().unwrap())]);
            registry.closure_from_callable(&Value::Array(callable))?
        }
        _ => {
            let class = class_name(script, &spec[1]);
            let callable = PhpArray::list([
                Value::string(class),
                Value::string(spec[2].as_str().unwrap()),
            ]);
            registry.closure_from_callable(&Value::Array(callable))?
        }
    })
}

/// `new ControllerResolver($logger)`, or `new
/// ContainerControllerResolver($container, $logger)` for a script with a
/// container.
fn resolver(
    file: &str,
    registry: &Arc<ClassRegistry>,
    logger: &Arc<Logger>,
    container: Option<&Arc<RecordingContainer>>,
) -> ControllerResolver {
    let logger: Arc<dyn LoggerInterface> = logger.clone();
    match container {
        Some(container) => {
            let shared = if container.di {
                ControllerContainer::DependencyInjection(container.clone())
            } else {
                ControllerContainer::Psr(container.clone())
            };
            ContainerControllerResolver::new(shared, registry.clone(), Some(logger))
                .with_source_file(file)
                .into_inner()
        }
        None => ControllerResolver::new(registry.clone(), Some(logger)).with_source_file(file),
    }
}

fn run(file: &str, script: &Json) -> Vec<(String, Json, Json)> {
    let mut differences = Vec::new();
    let mut differ = |what: &str, expected: &Json, actual: Json| {
        if *expected != actual {
            differences.push((what.to_owned(), expected.clone(), actual));
        }
    };

    let registry = match declare(script) {
        Ok(registry) => Arc::new(registry),
        Err(e) => {
            differences.push(("declarations".to_owned(), json!("declared"), json!(e)));
            return differences;
        }
    };
    let objects: Vec<Value> = script["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| match o[0].as_str().unwrap() {
            "class" => registry
                .new_instance(class_name(script, &o[1]).as_bytes())
                .unwrap(),
            _ => StdClass::object(PhpArray::new()),
        })
        .collect();
    let closures: Option<Vec<Value>> = script["closures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let closure = make_closure(script, &registry, &objects, c)?;
            let mut expected = c.clone();
            expected.as_object_mut().unwrap().remove("spec");
            differ("closure", &expected, closure_json(&closure));
            Some(closure)
        })
        .collect();
    // PHP made a closure of each recorded callable.
    let Some(closures) = closures else {
        differences.push((
            "closure".to_owned(),
            json!("a closure"),
            json!("not callable"),
        ));
        return differences;
    };

    let request = Handle::new(Request::create(script["path"].as_str().unwrap()).unwrap());
    if script["controller"][0] != "unset" {
        let controller = decode_controller(&script["controller"], &objects, &closures);
        request.with_mut(|r| r.attributes_mut().set("_controller", controller));
    }
    if !script["check"].is_null() {
        let check = vectors::decode(&script["check"]).unwrap();
        request.with_mut(|r| {
            r.attributes_mut()
                .set("_check_controller_is_allowed", check);
        });
    }

    let logger = Arc::new(Logger::default());
    let container = script
        .get("container")
        .map(|spec| container(spec, &objects, &closures));
    let mut resolver = resolver(file, &registry, &logger, container.as_ref());
    for call in script["allow"].as_array().unwrap() {
        let types: Vec<PhpString> = call[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| PhpString::from(unb64(t)))
            .collect();
        resolver.allow_controllers(types, strings(&call[1]));
    }

    let outcome = match resolver.resolve(&request) {
        Ok(None) => json!(["false"]),
        Ok(Some(found)) => json!([
            "returns",
            encode(&found.callable, &objects, &closures),
            call_json(found.controller.call(&[])),
            b64(found.controller.name().as_bytes()),
        ]),
        Err(e) => json!(["throws", throwable(&e)]),
    };
    differ("outcome", &script["outcome"], outcome);
    differ(
        "logs",
        &script["logs"],
        Json::Array(logger.0.lock().unwrap().clone()),
    );
    if let Some(container) = &container {
        differ(
            "container calls",
            &script["container_calls"],
            Json::Array(container.calls.lock().unwrap().clone()),
        );
    }
    differences
}

#[test]
fn recorded_scripts() {
    run_all(DATA, 4000);
}

#[test]
fn recorded_container_scripts() {
    run_all(CONTAINER_DATA, 2000);
}

fn run_all(data: &str, count: usize) {
    let data: Json = serde_json::from_str(data).unwrap();
    let file = data["file"].as_str().unwrap();
    let scripts = data["scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), count);
    let mut failures = 0;
    for (n, script) in scripts.iter().enumerate() {
        let differences = run(file, script);
        if !differences.is_empty() {
            failures += 1;
            if failures <= 5 {
                for (what, expected, actual) in differences {
                    eprintln!("script {n} {what}:\n  php:  {expected}\n  rust: {actual}");
                }
            }
        }
    }
    assert_eq!(failures, 0, "{failures} scripts differ");
}
