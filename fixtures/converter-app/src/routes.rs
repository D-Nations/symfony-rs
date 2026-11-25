//! The routes of oracle/apps/converter-fixture as `debug:router` lists
//! them (`#[Route]` on `GreetController::greet`, with `router.utf8: true`),
//! in the shape symfony2rust is to emit: a routing `RouteCollection`
//! (converter contract 0.3, section 6).
//!
//! Upstream: none

use php_value::{PhpString, Value};
use symfony_routing::{exception::Error, route::Route, route_collection::RouteCollection};

use crate::app::greet_controller;

/// `greet`: `GET /greet/{name}`, `name` matching `[a-z]+`.
pub fn routes() -> Result<RouteCollection, Error> {
    let mut greet = Route::new("/greet/{name}")?;
    greet.set_methods(["GET"]);
    greet.set_default(
        b"_controller",
        Value::String(PhpString::from(format!(
            "{}::greet",
            greet_controller::CLASS
        ))),
    )?;
    greet.set_requirement(b"name", b"[a-z]+")?;
    greet.set_option(b"utf8", Value::Bool(true));
    let mut routes = RouteCollection::new();
    routes.add("greet", greet, 0);
    Ok(routes)
}
