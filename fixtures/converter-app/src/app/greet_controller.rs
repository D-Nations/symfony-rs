//! `Fixture\Controller\GreetController` (oracle/apps/converter-fixture/src/Controller/GreetController.php).
//!
//! A controller is a service like any other, built from the request scope,
//! and a PHP object: the `ContainerControllerResolver` takes it from the
//! container for `Fixture\Controller\GreetController::greet`. Its class is
//! declared for the resolver (what reflection would report of `greet()`),
//! and the declared method runs the Rust method on `$this` (converter
//! contract 0.3, section 6).
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpArray, PhpObject, PhpString, Value};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag, request::Request, request_stack::RequestHandle,
    response::Response,
};
use symfony_http_kernel::{
    controller::{Controller, ControllerResult, Parameter, REQUEST_CLASS, as_request},
    controller_resolver::{Class, Method},
    exception::Throwable,
    http_kernel::response_handle,
};

use super::greeter::Greeter;
use crate::runtime::NotPorted;

pub const CLASS: &str = "Fixture\\Controller\\GreetController";

#[derive(Debug)]
pub struct GreetController {
    greeter: Arc<Greeter>,
}

impl PhpObject for GreetController {
    fn class(&self) -> &str {
        CLASS
    }

    /// Its only property is private.
    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

fn throwable(error: impl std::error::Error + Send + Sync + 'static) -> Throwable {
    Arc::new(error)
}

impl GreetController {
    pub fn new(greeter: Arc<Greeter>) -> Self {
        Self { greeter }
    }

    /// `#[Route('/greet/{name}', name: 'greet', requirements: ['name' => '[a-z]+'], methods: ['GET'])]`
    pub fn greet(&self, name: &PhpString, request: &mut Request) -> Result<Response, Throwable> {
        let punctuation = request
            .query()
            .get_string("punctuation", "!")
            .map_err(throwable)?;
        let greeting = self.greeter.greet(name, &punctuation).map_err(throwable)?;
        let headers =
            ResponseHeaderBag::new([("Content-Type", "text/plain")]).map_err(throwable)?;
        let content = greeting.render().map_err(throwable)?;
        Response::new(Some(content), 200, headers).map_err(throwable)
    }
}

/// The declared class: `greet(string $name, Request $request): Response`.
pub fn class() -> Class {
    let declared = Controller::new(|_| Err(throwable(NotPorted("greet() without its object"))))
        .with_parameters([
            Parameter::new("name").typed("string"),
            Parameter::new("request").typed(REQUEST_CLASS),
        ]);
    Class::new(CLASS).with_method(Method::new("greet", declared).with_this(|this, arguments| {
        let controller = match this {
            Value::Object(object) => object.downcast_ref::<GreetController>(),
            _ => None,
        }
        .ok_or_else(|| throwable(NotPorted("greet() on another object")))?;
        // `string $name`: the route attribute is always a string.
        let Some(Value::String(name)) = arguments.first() else {
            return Err(throwable(NotPorted(
                "greet() with a name that is not a string",
            )));
        };
        let request: RequestHandle = arguments
            .get(1)
            .and_then(as_request)
            .ok_or_else(|| throwable(NotPorted("greet() without its request")))?;
        let response = request.with_mut(|r| controller.greet(name, r))?;
        Ok(ControllerResult::Response(response_handle(response)))
    }))
}
