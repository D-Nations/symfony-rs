//! `ArgumentResolver` against PHP 8.4.27 running the pinned
//! symfony/http-kernel (v8.1.11), recorded by
//! `oracle/recorders/http_kernel_argument_resolver.php`
//! (`scripts/record.sh http-kernel-argument-resolver`): 4,000 generated
//! scripts, each a closure with a generated signature (types, nullable and
//! union types, defaults, a variadic, `#[ValueResolver]` attributes),
//! generated request attributes, and the default resolvers or a custom
//! chain, with or without named resolvers.
//!
//! The closure's parameters are declared from what PHP's reflection
//! reported (the recorded metadata); the factory's metadata must equal
//! it, and `getArguments()` must give the same arguments (the request by
//! identity) or the same exception (class and message).
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::sync::Arc;

use base64::Engine;
use php_value::{Handle, Value, vectors};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    argument_resolver::{
        ArgumentResolver, NamedResolvers, Resolvers, ValueResolver, ValueResolverInterface,
    },
    controller::{Controller, ControllerResult, Parameter, as_request},
    controller_metadata::{
        ArgumentMetadata, ArgumentMetadataFactory, ArgumentMetadataFactoryInterface,
    },
    exception::{PhpClass, PhpException, Throwable, class_of, message_of},
    value_resolver::{
        DefaultValueResolver, RequestAttributeValueResolver, RequestValueResolver,
        SessionValueResolver, VariadicValueResolver,
    },
};

const DATA: &str = include_str!("data/argument_resolver.json");

fn b64(bytes: &[u8]) -> Json {
    Json::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// `typed_value()` of oracle/recorders/lib/typed.php, for arguments.
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
        Value::Array(a) => json!([
            "array",
            a.iter()
                .map(|(k, v)| json!([typed(&k.to_value()), typed(v)]))
                .collect::<Vec<_>>()
        ]),
        Value::Object(o) => json!([
            "object",
            o.class(),
            o.properties()
                .iter()
                .map(|(k, v)| json!([typed(&k.to_value()), typed(v)]))
                .collect::<Vec<_>>()
        ]),
    }
}

/// `Yields2`: two values for `$two`.
struct Yields2;

impl ValueResolverInterface for Yields2 {
    fn class(&self) -> &'static str {
        "Yields2"
    }

    fn resolve(
        &self,
        _: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        Ok(if argument.get_name() == "two" {
            vec![Value::string("a"), Value::string("b")]
        } else {
            Vec::new()
        })
    }
}

/// `NearMiss2`: a near miss for `$miss` and `$x`.
struct NearMiss2;

impl ValueResolverInterface for NearMiss2 {
    fn class(&self) -> &'static str {
        "NearMiss2"
    }

    fn resolve(
        &self,
        _: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        if ["miss", "x"].contains(&argument.get_name()) {
            return Err(PhpException::new(
                PhpClass::NearMissValueResolverException,
                format!("Not for ${} either.", argument.get_name()),
            )
            .throwable());
        }
        Ok(Vec::new())
    }
}

/// `NearMiss`: a near miss for `$miss`.
struct NearMiss;

impl ValueResolverInterface for NearMiss {
    fn class(&self) -> &'static str {
        "NearMiss"
    }

    fn resolve(
        &self,
        _: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        if argument.get_name() == "miss" {
            return Err(PhpException::new(
                PhpClass::NearMissValueResolverException,
                format!("No luck for ${}.", argument.get_name()),
            )
            .throwable());
        }
        Ok(Vec::new())
    }
}

fn resolver(name: &str) -> (String, Arc<dyn ValueResolverInterface>) {
    let resolver: Arc<dyn ValueResolverInterface> = match name {
        "RequestAttribute" => Arc::new(RequestAttributeValueResolver),
        "Request" => Arc::new(RequestValueResolver),
        "Session" => Arc::new(SessionValueResolver),
        "Default" => Arc::new(DefaultValueResolver),
        "Variadic" => Arc::new(VariadicValueResolver),
        "Yields2" => Arc::new(Yields2),
        "NearMiss" => Arc::new(NearMiss),
        "NearMiss2" => Arc::new(NearMiss2),
        other => panic!("unknown resolver {other}"),
    };
    (resolver.class().to_owned(), resolver)
}

fn parameters(metadata: &[Json]) -> (String, Vec<Parameter>) {
    let mut name = String::from("{closure}");
    let parameters = metadata
        .iter()
        .map(|m| {
            m[7].as_str().unwrap().clone_into(&mut name);
            let mut p = Parameter::new(m[0].as_str().unwrap());
            if let Some(t) = m[1].as_str() {
                p = p.typed(t);
            }
            if m[2].as_bool().unwrap() {
                p = p.variadic();
            }
            if m[3].as_bool().unwrap() {
                p = p.with_default(vectors::decode(&m[4]).unwrap());
            }
            if m[5].as_bool().unwrap() && m[1].is_string() {
                p = p.nullable();
            }
            let attributes = m[6].as_array().unwrap().iter().map(|a| {
                ValueResolver {
                    class: a[0].as_str().unwrap().to_owned(),
                    resolver: a[1].as_str().unwrap().to_owned(),
                    disabled: a[2].as_bool().unwrap(),
                }
                .object()
            });
            p.with_attributes(attributes)
        })
        .collect();
    (name, parameters)
}

fn metadata_json(m: &ArgumentMetadata) -> Json {
    json!([
        m.get_name(),
        m.get_type(),
        m.is_variadic(),
        m.has_default_value(),
        if m.has_default_value() {
            typed(&m.get_default_value().unwrap())
        } else {
            Json::Null
        },
        m.is_nullable(),
        m.get_attributes(None, 0)
            .iter()
            .map(|a| {
                let Value::Object(o) = a else {
                    panic!("an object")
                };
                let r = o.downcast_ref::<ValueResolver>().unwrap();
                json!([r.class, r.resolver, r.disabled])
            })
            .collect::<Vec<_>>(),
        m.get_controller_name(),
    ])
}

fn run(script: &Json) -> (Json, Json) {
    let (name, parameters) = parameters(script["metadata"].as_array().unwrap());
    let controller = Controller::new(|_| Ok(ControllerResult::Value(Value::Null)))
        .with_name(&name)
        .with_parameters(parameters);

    let metadata: Vec<Json> = ArgumentMetadataFactory
        .create_argument_metadata(&controller, None)
        .iter()
        .map(metadata_json)
        .collect();

    let request = Handle::new(Request::create("/").unwrap());
    for pair in script["attributes"].as_array().unwrap() {
        let key = vectors::decode(&pair[0]).unwrap();
        let value = vectors::decode(&pair[1]).unwrap();
        let key = key.to_php_string().unwrap();
        request.with_mut(|r| {
            r.attributes_mut().set(&key.to_string_lossy(), value);
        });
    }

    let chain: Resolvers = script["chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let (_, r) = resolver(entry[1].as_str().unwrap());
            (entry[0].as_str().map(str::to_owned), r)
        })
        .collect();
    let named = script["named"]
        .as_array()
        .map(|names| NamedResolvers::locator(names.iter().map(|n| resolver(n.as_str().unwrap()))));

    let resolver = ArgumentResolver::new(Some(Arc::new(ArgumentMetadataFactory)), chain, named);
    let outcome = match resolver.get_arguments(&request, &controller, None) {
        Ok(arguments) => json!([
            "returns",
            arguments
                .iter()
                .map(|a| match as_request(a) {
                    Some(r) if r.same(&request) => json!(["request"]),
                    _ => typed(a),
                })
                .collect::<Vec<_>>()
        ]),
        Err(e) => json!([
            "throws",
            class_of(e.as_ref()),
            b64(message_of(e.as_ref()).as_bytes())
        ]),
    };
    (Json::Array(metadata), outcome)
}

#[test]
fn argument_resolver_matches_php() {
    let data: Json = serde_json::from_str(DATA).unwrap();
    let scripts = data["scripts"].as_array().unwrap();
    assert_eq!(4000, scripts.len());
    let mut failures = Vec::new();
    for (n, script) in scripts.iter().enumerate() {
        let (metadata, outcome) = run(script);
        if metadata != script["metadata"] || outcome != script["outcome"] {
            failures.push(n);
            if failures.len() <= 5 {
                let code = vectors::base64(script["code"].as_str().unwrap()).unwrap();
                eprintln!(
                    "script {n}: {}\nattributes {}\nchain {} named {}\nexpected {} {}\nactual   {metadata} {outcome}",
                    String::from_utf8_lossy(&code),
                    script["attributes"],
                    script["chain"],
                    script["named"],
                    script["metadata"],
                    script["outcome"],
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} scripts differ: {:?}",
        failures.len(),
        scripts.len(),
        &failures[..failures.len().min(20)]
    );
}
