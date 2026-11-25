//! `Method::with_this`: a declared method whose body reads `$this`. Not an
//! upstream test: PHP methods always see `$this`; this pins which object
//! the port passes for each callable shape, and that what reflection
//! reports is the declared body's.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::sync::Arc;

use php_value::{Handle, PhpArray, PhpObject, Value};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    controller::{Controller, ControllerResult, Parameter},
    controller_resolver::{Class, ClassRegistry, ControllerResolver, Method},
};

const CLASS: &str = "App\\Counter";

/// An object of the declared class with its own state.
#[derive(Debug)]
struct Counter(i64);

impl PhpObject for Counter {
    fn class(&self) -> &str {
        CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// `$this->n`, or `null` without an object of the class.
fn own_number(this: &Value) -> Value {
    match this {
        Value::Object(object) => object
            .downcast_ref::<Counter>()
            .map_or(Value::Null, |c| Value::Int(c.0)),
        _ => Value::Null,
    }
}

fn declared() -> Controller {
    Controller::new(|_| Ok(ControllerResult::Value(Value::string("unbound"))))
        .with_parameters([Parameter::new("step").typed("int")])
}

fn resolver() -> ControllerResolver {
    let mut classes = ClassRegistry::new();
    classes
        .declare_class(
            Class::new(CLASS)
                .with_method(
                    Method::new("show", declared())
                        .with_this(|this, _| Ok(ControllerResult::Value(own_number(this)))),
                )
                .with_method(
                    Method::new("__invoke", declared())
                        .with_this(|this, _| Ok(ControllerResult::Value(own_number(this)))),
                )
                .with_method(
                    Method::new("make", declared())
                        .into_static()
                        .with_this(|_, _| Ok(ControllerResult::Value(Value::string("bound")))),
                ),
        )
        .unwrap();
    ControllerResolver::new(Arc::new(classes), None)
}

fn run(resolver: &ControllerResolver, controller: Value) -> Value {
    let request: RequestHandle = Handle::new(Request::create("/").unwrap());
    request.with_mut(|r| r.attributes_mut().set("_controller", controller));
    let found = resolver.resolve(&request).unwrap().unwrap();
    assert_eq!(1, found.controller.parameters().len());
    match found.controller.call(&[Value::Int(1)]).unwrap() {
        ControllerResult::Value(value) => value,
        ControllerResult::Response(_) => panic!("a response"),
    }
}

fn counter(n: i64) -> Value {
    Value::Object(Arc::new(Counter(n)))
}

#[test]
fn an_array_callable_runs_on_its_object() {
    let resolver = resolver();
    let first = Value::Array(PhpArray::list([counter(7), Value::string("show")]));
    let second = Value::Array(PhpArray::list([counter(9), Value::string("show")]));
    assert_eq!(Value::Int(7), run(&resolver, first));
    assert_eq!(Value::Int(9), run(&resolver, second));
}

#[test]
fn an_invokable_object_runs_on_itself() {
    assert_eq!(Value::Int(3), run(&resolver(), counter(3)));
}

#[test]
fn a_closure_of_a_method_keeps_its_object() {
    let resolver = resolver();
    let callable = Value::Array(PhpArray::list([counter(5), Value::string("show")]));
    let closure = resolver.classes().closure_from_callable(&callable).unwrap();
    assert_eq!(Value::Int(5), run(&resolver, closure));
}

#[test]
fn a_class_string_runs_on_the_new_instance() {
    // `new App\Counter()` is the registry's own object, with no state.
    let value = run(&resolver(), Value::string(format!("{CLASS}::show")));
    assert_eq!(Value::Null, value);
}

#[test]
fn a_static_method_has_no_this() {
    let value = run(&resolver(), Value::string(format!("{CLASS}::make")));
    assert_eq!(Value::string("unbound"), value);
}
