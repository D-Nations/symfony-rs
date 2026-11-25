//! `Fixture\Controller\GreetController` (oracle/apps/converter-fixture/src/Controller/GreetController.php).
//!
//! An action is `async fn(&Container, RequestScope, Request, args) ->
//! Result<Response, Exception>` (converter contract 0.1, section 6). The
//! controller is a service like any other, built from the request scope.
//!
//! Upstream: none

use std::sync::Arc;

use axum::{extract::Request, http::StatusCode, response::Response};
use php_value::PhpString;
use symfony_http_kernel::Exception;

use super::greeter::Greeter;
use crate::{
    container::{Container, RequestScope},
    runtime::http_foundation::{InputBag, new_response},
};

pub struct GreetController {
    greeter: Arc<Greeter>,
}

impl GreetController {
    pub fn new(greeter: Arc<Greeter>) -> Self {
        Self { greeter }
    }

    /// `#[Route('/greet/{name}', name: 'greet', requirements: ['name' => '[a-z]+'], methods: ['GET'])]`
    pub fn greet(&self, name: &PhpString, request: &Request) -> Result<Response, Exception> {
        let query = InputBag::query(request);
        let punctuation = query.get_string("punctuation", "!")?;
        let greeting = self.greeter.greet(name, &punctuation)?;
        new_response(
            &greeting.render()?,
            StatusCode::OK,
            &[("Content-Type", "text/plain")],
        )
    }
}

/// The action as the kernel invokes it: `GreetController::greet`, with
/// `$name` from the route and `$request` from the argument resolver.
#[expect(
    clippy::unused_async,
    reason = "converter contract: every action is an async fn"
)]
pub async fn greet(
    container: &Container,
    scope: RequestScope,
    request: Request,
    name: PhpString,
) -> Result<Response, Exception> {
    scope.greet_controller(container).greet(&name, &request)
}
