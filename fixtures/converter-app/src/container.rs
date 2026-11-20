//! The compiled container of oracle/apps/converter-fixture, as `debug:container`
//! reports it after the compiler passes, in the shape symfony2rust is to emit
//! (converter contract 0.2, section 4).
//!
//! `Container` is application-scoped: built once, `Send + Sync`, never
//! written after start-up. `RequestScope` holds the shared services a request
//! may write (or that reach one): each request builds its own, lazily, as
//! PHP builds a fresh container per request.
//!
//! Upstream: none

use std::{cell::OnceCell, sync::Arc};

use php_value::{PhpArray, PhpString, Value};

use crate::app::{
    error_subscriber::ErrorSubscriber, greet_controller::GreetController, greeter::Greeter,
    visit_counter::VisitCounter,
};

/// `%app.greeting%`, `%app.max_length%`, `%app.tags%` (config/services.yaml).
#[derive(Debug)]
pub struct Parameters {
    pub app_greeting: PhpString,
    pub app_max_length: i64,
    pub app_tags: PhpArray,
}

#[derive(Debug)]
pub struct Container {
    pub parameters: Parameters,
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Container {
    pub fn new() -> Self {
        Self {
            parameters: Parameters {
                app_greeting: PhpString::from("Hello"),
                app_max_length: 12,
                app_tags: PhpArray::list([Value::string("converter"), Value::string("fixture")]),
            },
        }
    }

    /// `Fixture\EventSubscriber\ErrorSubscriber`: stateless, so
    /// application-scoped; registered with the kernel at start-up.
    pub fn error_subscriber(&self) -> ErrorSubscriber {
        ErrorSubscriber
    }
}

/// The request-scoped shared services. `Send`, not `Sync`: one request uses
/// it, and no borrow of it is held across an `.await`.
#[derive(Default)]
pub struct RequestScope {
    visit_counter: OnceCell<VisitCounter>,
    greeter: OnceCell<Arc<Greeter>>,
    greet_controller: OnceCell<Arc<GreetController>>,
}

impl RequestScope {
    /// `Fixture\Service\VisitCounter`: written after construction.
    pub fn visit_counter(&self) -> VisitCounter {
        self.visit_counter.get_or_init(VisitCounter::new).clone()
    }

    /// `Fixture\Service\Greeter`: reaches `VisitCounter`.
    pub fn greeter(&self, container: &Container) -> Arc<Greeter> {
        self.greeter
            .get_or_init(|| {
                let parameters = &container.parameters;
                Arc::new(Greeter::new(
                    parameters.app_greeting.clone(),
                    parameters.app_max_length,
                    parameters.app_tags.clone(),
                    self.visit_counter(),
                ))
            })
            .clone()
    }

    /// `Fixture\Controller\GreetController`: reaches `Greeter`.
    pub fn greet_controller(&self, container: &Container) -> Arc<GreetController> {
        self.greet_controller
            .get_or_init(|| Arc::new(GreetController::new(self.greeter(container))))
            .clone()
    }
}
