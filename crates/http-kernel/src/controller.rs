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
//! The default `ArgumentResolver` and its value resolvers are in
//! [`crate::argument_resolver`].
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
    /// The declared type as reflection prints it (`string`, `int|string`,
    /// a class name; `?string` is `string` with [`Parameter::nullable`]);
    /// `None` without a type declaration.
    pub type_name: Option<String>,
    /// A nullable type: `?T`, `T|null`, or a `null` default.
    pub nullable: bool,
    /// The parameter's attributes, as objects (`getAttributes()` then
    /// `newInstance()`).
    pub attributes: Vec<Value>,
}

impl Parameter {
    /// A required parameter.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            variadic: false,
            default: None,
            type_name: None,
            nullable: false,
            attributes: Vec::new(),
        }
    }

    /// With a declared type.
    #[must_use]
    pub fn typed(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
    }

    /// With a nullable type (`?T`).
    #[must_use]
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// With attributes.
    #[must_use]
    pub fn with_attributes(mut self, attributes: impl IntoIterator<Item = Value>) -> Self {
        self.attributes = attributes.into_iter().collect();
        self
    }

    /// `allowsNull()`: no type, a nullable type, or a `null` default.
    pub fn allows_null(&self) -> bool {
        self.type_name.is_none() || self.nullable || matches!(self.default, Some(Value::Null))
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

/// The name of a closure that declares none. PHP 8.4 names closures
/// `{closure:file:line}`; a Rust closure has no such name, so a
/// controller is given one with [`Controller::with_name`].
pub const CLOSURE_NAME: &str = "{closure}";

/// `Symfony\Component\HttpFoundation\Request`.
pub const REQUEST_CLASS: &str = "Symfony\\Component\\HttpFoundation\\Request";

/// A `Request` passed as a PHP value (a controller argument): the same
/// object as the handle.
#[derive(Debug)]
pub struct RequestObject(pub RequestHandle);

impl php_value::PhpObject for RequestObject {
    fn class(&self) -> &str {
        REQUEST_CLASS
    }

    fn properties(&self) -> php_value::PhpArray {
        php_value::PhpArray::new()
    }
}

/// The request as a controller argument.
pub fn request_value(request: &RequestHandle) -> Value {
    Value::Object(Arc::new(RequestObject(request.clone())))
}

/// The request a controller argument holds, if it is one.
pub fn as_request(value: &Value) -> Option<RequestHandle> {
    match value {
        Value::Object(o) => o.downcast_ref::<RequestObject>().map(|r| r.0.clone()),
        _ => None,
    }
}

type Callable = dyn Fn(&[Value]) -> Result<ControllerResult, Throwable> + Send + Sync;

/// A PHP controller callable.
#[derive(Clone)]
pub struct Controller {
    function: Arc<Callable>,
    parameters: Arc<[Parameter]>,
    attributes: Arc<[Value]>,
    name: Arc<str>,
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
            name: Arc::from(CLOSURE_NAME),
        }
    }

    /// With the name `ArgumentMetadataFactory` reports in messages:
    /// `Class::method` for a method, the function's name, or a closure's.
    #[must_use]
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.name = Arc::from(name.as_ref());
        self
    }

    /// The controller's name (`getPrettyName()`).
    pub fn name(&self) -> &str {
        &self.name
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
            .field("name", &self.name)
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
