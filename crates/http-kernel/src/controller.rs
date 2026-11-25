//! Port of Symfony\Component\HttpKernel\Controller\ControllerResolverInterface.php
//! and ArgumentResolverInterface.php, with the controller they deal in.
//!
//! In PHP a controller is any callable, and the kernel learns its
//! parameters and attributes by reflection. Here a [`Controller`] is a
//! closure together with what reflection would report: its parameters
//! (name, variadic, default value) and its attributes (the class's and
//! the method's, as objects). Cloning a controller gives the same
//! callable; [`Controller::same`] is the `==` PHP uses for closures. A
//! controller takes its arguments as PHP values and returns a
//! [`ControllerResult`]: a response, or any other value for the view
//! event.
//!
//! The default `ArgumentResolver` (and its value resolvers) is not
//! ported; a kernel is given an argument resolver.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Controller/ControllerResolverInterface.php Controller/ArgumentResolverInterface.php

use std::{fmt, sync::Arc};

use php_value::{Handle, Value};
use symfony_http_foundation::{request_stack::RequestHandle, response::Response};

use crate::exception::Throwable;

/// A PHP `Response` object handle.
pub type ResponseHandle = Handle<Response>;

/// What a controller returns.
#[derive(Debug, Clone)]
pub enum ControllerResult {
    /// A `Response`.
    Response(ResponseHandle),
    /// Anything else, for `kernel.view` listeners to turn into a response.
    Value(Value),
}

/// One parameter, as `\ReflectionParameter` describes it.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub variadic: bool,
    /// The default value, when one is available.
    pub default: Option<Value>,
}

impl Parameter {
    /// A required parameter.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            variadic: false,
            default: None,
        }
    }

    /// With a default value.
    #[must_use]
    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }

    /// A variadic parameter (`...$name`).
    #[must_use]
    pub fn variadic(mut self) -> Self {
        self.variadic = true;
        self
    }
}

type Callable = dyn Fn(&[Value]) -> Result<ControllerResult, Throwable> + Send + Sync;

/// A PHP controller callable.
#[derive(Clone)]
pub struct Controller {
    function: Arc<Callable>,
    parameters: Arc<[Parameter]>,
    attributes: Arc<[Value]>,
}

impl Controller {
    /// A controller without parameters or attributes.
    pub fn new(
        function: impl Fn(&[Value]) -> Result<ControllerResult, Throwable> + Send + Sync + 'static,
    ) -> Self {
        Self {
            function: Arc::new(function),
            parameters: Arc::from(Vec::new()),
            attributes: Arc::from(Vec::new()),
        }
    }

    /// With its parameters (`\ReflectionFunctionAbstract::getParameters()`).
    #[must_use]
    pub fn with_parameters(mut self, parameters: impl IntoIterator<Item = Parameter>) -> Self {
        self.parameters = parameters.into_iter().collect();
        self
    }

    /// With its attributes (the class's, then the function's), as
    /// `ControllerEvent::getAttributes()` instantiates them.
    #[must_use]
    pub fn with_attributes(mut self, attributes: impl IntoIterator<Item = Value>) -> Self {
        self.attributes = attributes.into_iter().collect();
        self
    }

    /// `$controller(...$arguments)`.
    pub fn call(&self, arguments: &[Value]) -> Result<ControllerResult, Throwable> {
        (self.function)(arguments)
    }

    /// `$a == $b` for closures, `===` for other callables: the same
    /// callable.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.function, &other.function)
    }

    /// The parameters.
    pub fn parameters(&self) -> &Arc<[Parameter]> {
        &self.parameters
    }

    /// The declared attributes.
    pub fn attributes(&self) -> &Arc<[Value]> {
        &self.attributes
    }
}

impl fmt::Debug for Controller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Controller")
            .field("parameters", &self.parameters)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

/// `ControllerResolverInterface`: finds the controller for a request.
pub trait ControllerResolverInterface: Send + Sync {
    /// `getController($request)`: `None` is PHP's `false` (no controller);
    /// the kernel answers it with a `NotFoundHttpException`.
    fn get_controller(&self, request: &RequestHandle) -> Result<Option<Controller>, Throwable>;
}

/// `ArgumentResolverInterface`: the arguments to pass to a controller.
pub trait ArgumentResolverInterface: Send + Sync {
    /// `getArguments($request, $controller, $reflector)`; `reflector` is
    /// the controller's parameters.
    fn get_arguments(
        &self,
        request: &RequestHandle,
        controller: &Controller,
        reflector: &[Parameter],
    ) -> Result<Vec<Value>, Throwable>;
}
