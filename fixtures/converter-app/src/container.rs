//! The compiled container of oracle/apps/converter-fixture, as `debug:container`
//! reports it after the compiler passes, in the shape symfony2rust is to emit
//! (converter contract 0.3, section 4).
//!
//! `Container` is application-scoped: built once, `Send + Sync`, never
//! written after start-up. `RequestScope` holds the shared services a request
//! may write (or that reach one): each request builds its own, lazily, as
//! PHP builds a fresh container per request. [`ServiceContainer`] is the
//! two together as the PSR-11 container the `ContainerControllerResolver`
//! asks for controllers.
//!
//! Upstream: none

use std::sync::{Arc, OnceLock};

use php_value::{PhpArray, PhpObject, PhpString, Value};
use psr_container::{ContainerInterface, Throwable};
use symfony_http_kernel::container_controller_resolver::DependencyInjectionContainer;

use crate::{
    app::{
        error_subscriber::ErrorSubscriber,
        greet_controller::{self, GreetController},
        greeter::Greeter,
        visit_counter::VisitCounter,
    },
    runtime::{
        NotPorted,
        framework_bundle::{REDIRECT_CONTROLLER, RedirectController},
    },
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
    /// `redirect_controller` (`FrameworkBundle`): stateless.
    redirect_controller: Arc<RedirectController>,
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
            redirect_controller: Arc::new(RedirectController::default()),
        }
    }

    /// `Fixture\EventSubscriber\ErrorSubscriber`: stateless, so
    /// application-scoped; added to the dispatcher when the kernel is built.
    pub fn error_subscriber(&self) -> ErrorSubscriber {
        ErrorSubscriber
    }
}

/// The request-scoped shared services: one request uses it, through its
/// [`ServiceContainer`].
#[derive(Debug, Default)]
pub struct RequestScope {
    visit_counter: OnceLock<VisitCounter>,
    greeter: OnceLock<Arc<Greeter>>,
    greet_controller: OnceLock<Arc<GreetController>>,
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

/// The container one request sees: the application's services and that
/// request's scope. Only the public services the resolver asks for are
/// listed (the two controllers).
#[derive(Debug)]
pub struct ServiceContainer {
    container: Arc<Container>,
    scope: RequestScope,
}

impl ServiceContainer {
    pub fn new(container: Arc<Container>) -> Self {
        Self {
            container,
            scope: RequestScope::default(),
        }
    }

    pub fn scope(&self) -> &RequestScope {
        &self.scope
    }
}

impl ContainerInterface for ServiceContainer {
    fn get(&self, id: &[u8]) -> Result<Value, Throwable> {
        let object: Arc<dyn PhpObject> = match id {
            _ if id == greet_controller::CLASS.as_bytes() => {
                self.scope.greet_controller(&self.container)
            }
            _ if id == REDIRECT_CONTROLLER.as_bytes() => self.container.redirect_controller.clone(),
            // ServiceNotFoundException's messages are not ported.
            _ => return Err(Arc::new(NotPorted("get() of a service id not listed"))),
        };
        Ok(Value::Object(object))
    }

    fn has(&self, id: &[u8]) -> bool {
        id == greet_controller::CLASS.as_bytes() || id == REDIRECT_CONTROLLER.as_bytes()
    }
}

/// Symfony's compiled container is a `DependencyInjection\Container`.
impl DependencyInjectionContainer for ServiceContainer {
    /// The compiled container's removed ids are not listed: the resolver
    /// reads them only for a `_controller` that is neither a service nor
    /// a class, and no route names one.
    fn get_removed_ids(&self) -> PhpArray {
        PhpArray::new()
    }
}
