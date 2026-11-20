//! The compiled routes of oracle/apps/converter-fixture (`debug:router`), as
//! symfony2rust is to emit them: one typed target per route, matched by
//! `symfony-routing` (converter contract 0.1, section 6).
//!
//! Upstream: none

use symfony_routing::{Definition, Requirement, RouteCollection, Segment, Target};

/// A matched route with its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `greet`: `GET /greet/{name}`, `name` matching `[a-z]+`.
    Greet { name: String },
}

impl Route {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Greet { .. } => "greet",
        }
    }
}

impl Target for Route {
    fn methods(&self) -> Option<&'static [&'static str]> {
        match self {
            Self::Greet { .. } => Some(&["GET"]),
        }
    }
}

/// `[a-z]+`, anchored, as the compiled matcher applies it (the `u` flag
/// from `router.utf8` does not widen `[a-z]`).
fn lowercase_ascii(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_lowercase())
}

pub fn routes() -> RouteCollection<Route> {
    let mut routes = RouteCollection::default();
    routes.add(Definition::new(
        vec![
            Segment::Literal("greet"),
            Segment::Parameter("name", Requirement::Custom(lowercase_ascii)),
        ],
        |parameters| {
            Some(Route::Greet {
                name: parameters.get("name")?.to_owned(),
            })
        },
    ));
    routes
}
