//! Port of Symfony\Component\HttpKernel\Event\KernelEvent.php,
//! RequestEvent.php, ControllerEvent.php, ControllerArgumentsEvent.php,
//! ControllerMetadata.php, ControllerArgumentsMetadata.php, ViewEvent.php,
//! ResponseEvent.php, FinishRequestEvent.php, TerminateEvent.php and
//! ExceptionEvent.php.
//!
//! The events reach listeners through the `EventDispatcher` as
//! `&mut dyn EventObject`; a listener downcasts to the event it expects.
//! The request and response an event carries are object handles (the
//! kernel, the request stack and other listeners see the same objects).
//! `ControllerEvent` and `ControllerArgumentsEvent` are handles too: the
//! metadata objects hold them, and see what later listeners change, as in
//! PHP.
//!
//! PHP's subclassing (`ExceptionEvent extends RequestEvent extends
//! KernelEvent`) is flattened: each event has the methods of its parents.
//! Not ported: `ControllerEvent::evaluate()` and the metadata's
//! `evaluate()` (they need `ExpressionLanguage`), and `ViewEvent`'s
//! deprecated `controllerArgumentsEvent` property. Attributes are matched
//! by exact class name (`instanceof` without the class's parents).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Event/KernelEvent.php Event/RequestEvent.php Event/ControllerEvent.php Event/ControllerArgumentsEvent.php Event/ControllerMetadata.php Event/ControllerArgumentsMetadata.php Event/ViewEvent.php Event/ResponseEvent.php Event/FinishRequestEvent.php Event/TerminateEvent.php Event/ExceptionEvent.php

use std::sync::Arc;

use php_value::{Handle, Key, PhpArray, Value};
use symfony_event_dispatcher::{Event, EventObject};
use symfony_http_foundation::request_stack::RequestHandle;

use crate::{
    controller::{Controller, Parameter, ResponseHandle},
    exception::Throwable,
    http_kernel::{HttpKernelInterface, KernelRef},
};

/// What every kernel event holds (`KernelEvent`).
#[derive(Clone)]
pub struct KernelEvent {
    kernel: KernelRef,
    request: RequestHandle,
    request_type: Option<i64>,
    event: Event,
}

impl KernelEvent {
    pub fn new(kernel: KernelRef, request: RequestHandle, request_type: Option<i64>) -> Self {
        Self {
            kernel,
            request,
            request_type,
            event: Event::new(),
        }
    }

    /// `getKernel()`.
    pub fn get_kernel(&self) -> &KernelRef {
        &self.kernel
    }

    /// `getRequest()`.
    pub fn get_request(&self) -> &RequestHandle {
        &self.request
    }

    /// `getRequestType()`: `None` is the `null` a sub-class may pass.
    pub fn get_request_type(&self) -> Option<i64> {
        self.request_type
    }

    /// `isMainRequest()`.
    pub fn is_main_request(&self) -> bool {
        self.request_type == Some(<dyn HttpKernelInterface>::MAIN_REQUEST)
    }

    /// `isPropagationStopped()`.
    pub fn is_propagation_stopped(&self) -> bool {
        self.event.is_propagation_stopped()
    }

    /// `stopPropagation()`.
    pub fn stop_propagation(&mut self) {
        self.event.stop_propagation();
    }
}

macro_rules! kernel_event {
    ($type:ty, $class:literal, $($base:ident).+) => {
        impl $type {
            /// `$event::class`.
            pub const CLASS: &'static str = concat!("Symfony\\Component\\HttpKernel\\Event\\", $class);

            /// `getKernel()`.
            pub fn get_kernel(&self) -> &KernelRef {
                self.$($base).+.get_kernel()
            }

            /// `getRequest()`.
            pub fn get_request(&self) -> &RequestHandle {
                self.$($base).+.get_request()
            }

            /// `getRequestType()`.
            pub fn get_request_type(&self) -> Option<i64> {
                self.$($base).+.get_request_type()
            }

            /// `isMainRequest()`.
            pub fn is_main_request(&self) -> bool {
                self.$($base).+.is_main_request()
            }

            /// `stopPropagation()`.
            pub fn stop_propagation(&mut self) {
                self.$($base).+.stop_propagation();
            }

            /// `isPropagationStopped()`.
            pub fn is_propagation_stopped(&self) -> bool {
                self.$($base).+.is_propagation_stopped()
            }
        }

        impl EventObject for $type {
            fn class(&self) -> &'static str {
                Self::CLASS
            }

            fn is_propagation_stopped(&self) -> bool {
                self.$($base).+.is_propagation_stopped()
            }
        }
    };
}

/// `RequestEvent`: a response set here stops propagation and is sent
/// without calling the controller.
pub struct RequestEvent {
    base: KernelEvent,
    response: Option<ResponseHandle>,
}

impl RequestEvent {
    pub fn new(kernel: KernelRef, request: RequestHandle, request_type: Option<i64>) -> Self {
        Self {
            base: KernelEvent::new(kernel, request, request_type),
            response: None,
        }
    }

    /// `getResponse()`.
    pub fn get_response(&self) -> Option<&ResponseHandle> {
        self.response.as_ref()
    }

    /// `setResponse($response)`: also stops propagation.
    pub fn set_response(&mut self, response: ResponseHandle) {
        self.response = Some(response);
        self.stop_propagation();
    }

    /// `hasResponse()`.
    pub fn has_response(&self) -> bool {
        self.response.is_some()
    }
}

kernel_event!(RequestEvent, "RequestEvent", base);

/// `ControllerEvent`'s state.
struct ControllerState {
    base: KernelEvent,
    controller: Controller,
    /// `isset($this->controller)`: false until the constructor's
    /// `setController()` call.
    initialized: bool,
    reflector: Arc<[Parameter]>,
}

/// `ControllerEvent`: listeners may replace the controller. Clones are the
/// same event object.
#[derive(Clone)]
pub struct ControllerEvent(Handle<ControllerState>);

const CONTROLLER_ATTRIBUTES: &str = "_controller_attributes";

impl ControllerEvent {
    /// `$event::class`.
    pub const CLASS: &'static str = "Symfony\\Component\\HttpKernel\\Event\\ControllerEvent";

    /// `new ControllerEvent($kernel, $controller, $request, $requestType)`.
    pub fn new(
        kernel: KernelRef,
        controller: Controller,
        request: RequestHandle,
        request_type: Option<i64>,
    ) -> Self {
        let event = Self(Handle::new(ControllerState {
            base: KernelEvent::new(kernel, request, request_type),
            controller: controller.clone(),
            initialized: false,
            reflector: Arc::from(Vec::new()),
        }));
        event.set_controller(controller, None);
        event
    }

    /// `$a === $b`.
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    fn base<R>(&self, f: impl FnOnce(&KernelEvent) -> R) -> R {
        self.0.with(|s| f(&s.base))
    }

    /// `getKernel()`.
    pub fn get_kernel(&self) -> KernelRef {
        self.base(|b| b.get_kernel().clone())
    }

    /// `getRequest()`.
    pub fn get_request(&self) -> RequestHandle {
        self.base(|b| b.get_request().clone())
    }

    /// `getRequestType()`.
    pub fn get_request_type(&self) -> Option<i64> {
        self.base(KernelEvent::get_request_type)
    }

    /// `isMainRequest()`.
    pub fn is_main_request(&self) -> bool {
        self.base(KernelEvent::is_main_request)
    }

    /// `stopPropagation()`.
    pub fn stop_propagation(&mut self) {
        self.0.with_mut(|s| s.base.stop_propagation());
    }

    /// `getController()`.
    pub fn get_controller(&self) -> Controller {
        self.0.with(|s| s.controller.clone())
    }

    /// `getControllerReflector()`: the controller's parameters.
    pub fn get_controller_reflector(&self) -> Arc<[Parameter]> {
        self.0.with(|s| s.reflector.clone())
    }

    /// `setController($controller, $attributes)`. `attributes` replaces
    /// what `getAttributes()` returns (a list; an array grouped by class
    /// name is deprecated and flattened); `None` with a new controller
    /// forgets the attributes set before.
    pub fn set_controller(&self, controller: Controller, attributes: Option<PhpArray>) {
        let request = self.get_request();
        let has_attributes = attributes.is_some();
        if let Some(attributes) = attributes {
            let flatten = if attributes.is_list() {
                attributes
            } else {
                symfony_deprecation_contracts::trigger_deprecation(
                    b"symfony/http-kernel",
                    b"8.1",
                    b"Passing an array of attributes grouped by class name to \"Symfony\\Component\\HttpKernel\\Event\\ControllerEvent::setController()\" is deprecated. Pass a flat list of attributes instead.",
                );
                let mut flat = Vec::new();
                for (_, group) in attributes.iter() {
                    match group {
                        Value::Array(list) => flat.extend(list.iter().map(|(_, a)| a.clone())),
                        attribute => flat.push(attribute.clone()),
                    }
                }
                PhpArray::list(flat)
            };
            request.with_mut(|r| {
                r.attributes_mut()
                    .set(CONTROLLER_ATTRIBUTES, Value::Array(flatten));
            });
        }

        if self
            .0
            .with(|s| s.initialized && s.controller.same(&controller))
        {
            self.0.with_mut(|s| s.controller = controller);
            return;
        }

        if !has_attributes {
            request.with_mut(|r| r.attributes_mut().remove(CONTROLLER_ATTRIBUTES));
        }

        self.0.with_mut(|s| {
            s.reflector = controller.parameters().clone();
            s.controller = controller;
            s.initialized = true;
        });
    }

    /// `getAttributes($className)`: `Some("*")` is every attribute as a
    /// list, `Some(class)` those of that class, `None` all of them grouped
    /// by class.
    pub fn get_attributes(&self, class_name: Option<&str>) -> PhpArray {
        let request = self.get_request();
        let stored = request.with(|r| r.attributes().get(CONTROLLER_ATTRIBUTES, Value::Null));
        let attributes = match stored {
            Value::Array(list) => list,
            Value::Null => {
                let declared = PhpArray::list(self.get_controller().attributes().iter().cloned());
                request.with_mut(|r| {
                    r.attributes_mut()
                        .set(CONTROLLER_ATTRIBUTES, Value::Array(declared.clone()));
                });
                declared
            }
            other => PhpArray::list([other]),
        };

        match class_name {
            Some("*") => attributes,
            Some(class) => PhpArray::list(
                attributes
                    .iter()
                    .filter(|(_, a)| a.as_object().is_some_and(|o| o.class() == class))
                    .map(|(_, a)| a.clone()),
            ),
            None => {
                let mut grouped = PhpArray::new();
                for (_, attribute) in attributes.iter() {
                    let class = attribute
                        .as_object()
                        .map_or_else(|| attribute.debug_type(), |o| o.class().to_owned());
                    let key = Key::from_bytes(class.as_bytes());
                    let mut group = match grouped.get(&key) {
                        Some(Value::Array(group)) => group.clone(),
                        _ => PhpArray::new(),
                    };
                    // A list built here always has its next index free.
                    let _ = group.push(attribute.clone());
                    grouped.insert(key, Value::Array(group));
                }
                grouped
            }
        }
    }
}

impl EventObject for ControllerEvent {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn is_propagation_stopped(&self) -> bool {
        self.base(KernelEvent::is_propagation_stopped)
    }
}

/// `ControllerArgumentsEvent`'s state.
struct ArgumentsState {
    base: KernelEvent,
    controller_event: ControllerEvent,
    arguments: Vec<Value>,
    named_arguments: Option<PhpArray>,
}

/// `ControllerArgumentsEvent`: listeners may replace the controller and
/// its arguments. Clones are the same event object.
#[derive(Clone)]
pub struct ControllerArgumentsEvent(Handle<ArgumentsState>);

impl ControllerArgumentsEvent {
    /// `$event::class`.
    pub const CLASS: &'static str =
        "Symfony\\Component\\HttpKernel\\Event\\ControllerArgumentsEvent";

    /// `new ControllerArgumentsEvent($kernel, $controllerEvent, $arguments,
    /// $request, $requestType)`.
    pub fn new(
        kernel: KernelRef,
        controller_event: ControllerEvent,
        arguments: Vec<Value>,
        request: RequestHandle,
        request_type: Option<i64>,
    ) -> Self {
        Self(Handle::new(ArgumentsState {
            base: KernelEvent::new(kernel, request, request_type),
            controller_event,
            arguments,
            named_arguments: None,
        }))
    }

    /// `new ControllerArgumentsEvent($kernel, $controller, ...)` with a
    /// callable: its own `ControllerEvent` is made.
    pub fn with_controller(
        kernel: KernelRef,
        controller: Controller,
        arguments: Vec<Value>,
        request: RequestHandle,
        request_type: Option<i64>,
    ) -> Self {
        let event = ControllerEvent::new(kernel.clone(), controller, request.clone(), request_type);
        Self::new(kernel, event, arguments, request, request_type)
    }

    /// `$a === $b`.
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    fn controller_event(&self) -> ControllerEvent {
        self.0.with(|s| s.controller_event.clone())
    }

    /// `getKernel()`.
    pub fn get_kernel(&self) -> KernelRef {
        self.0.with(|s| s.base.get_kernel().clone())
    }

    /// `getRequest()`.
    pub fn get_request(&self) -> RequestHandle {
        self.0.with(|s| s.base.get_request().clone())
    }

    /// `getRequestType()`.
    pub fn get_request_type(&self) -> Option<i64> {
        self.0.with(|s| s.base.get_request_type())
    }

    /// `isMainRequest()`.
    pub fn is_main_request(&self) -> bool {
        self.0.with(|s| s.base.is_main_request())
    }

    /// `stopPropagation()`.
    pub fn stop_propagation(&mut self) {
        self.0.with_mut(|s| s.base.stop_propagation());
    }

    /// `getController()`.
    pub fn get_controller(&self) -> Controller {
        self.controller_event().get_controller()
    }

    /// `setController($controller, $attributes)`.
    pub fn set_controller(&self, controller: Controller, attributes: Option<PhpArray>) {
        self.controller_event()
            .set_controller(controller, attributes);
        self.0.with_mut(|s| s.named_arguments = None);
    }

    /// `getArguments()`.
    pub fn get_arguments(&self) -> Vec<Value> {
        self.0.with(|s| s.arguments.clone())
    }

    /// `setArguments($arguments)`.
    pub fn set_arguments(&self, arguments: Vec<Value>) {
        self.0.with_mut(|s| {
            s.arguments = arguments;
            s.named_arguments = None;
        });
    }

    /// `getNamedArguments()`: the arguments by the controller's parameter
    /// names; a variadic parameter takes the rest, a missing argument its
    /// default.
    pub fn get_named_arguments(&self) -> PhpArray {
        if let Some(named) = self.0.with(|s| s.named_arguments.clone()) {
            return named;
        }
        let reflector = self.controller_event().get_controller_reflector();
        let arguments = self.get_arguments();
        let mut named = PhpArray::new();
        for (i, parameter) in reflector.iter().enumerate() {
            let key = Key::from_bytes(parameter.name.as_bytes());
            if parameter.variadic {
                named.insert(
                    key,
                    Value::Array(PhpArray::list(arguments.iter().skip(i).cloned())),
                );
                break;
            }
            if let Some(argument) = arguments.get(i) {
                named.insert(key, argument.clone());
            } else if let Some(default) = &parameter.default {
                named.insert(key, default.clone());
            }
        }
        self.0.with_mut(|s| s.named_arguments = Some(named.clone()));
        named
    }

    /// `getAttributes($className)`.
    pub fn get_attributes(&self, class_name: Option<&str>) -> PhpArray {
        self.controller_event().get_attributes(class_name)
    }
}

impl EventObject for ControllerArgumentsEvent {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn is_propagation_stopped(&self) -> bool {
        self.0.with(|s| s.base.is_propagation_stopped())
    }
}

/// `ControllerArgumentsMetadata`: the controller and its arguments, as
/// their events hold them now.
#[derive(Clone)]
pub struct ControllerArgumentsMetadata {
    controller_event: ControllerEvent,
    arguments_event: ControllerArgumentsEvent,
}

impl ControllerArgumentsMetadata {
    pub fn new(
        controller_event: ControllerEvent,
        arguments_event: ControllerArgumentsEvent,
    ) -> Self {
        Self {
            controller_event,
            arguments_event,
        }
    }

    /// `getController()`.
    pub fn get_controller(&self) -> Controller {
        self.controller_event.get_controller()
    }

    /// `getReflector()`.
    pub fn get_reflector(&self) -> Arc<[Parameter]> {
        self.controller_event.get_controller_reflector()
    }

    /// `getAttributes($className)`.
    pub fn get_attributes(&self, class_name: Option<&str>) -> PhpArray {
        self.controller_event.get_attributes(class_name)
    }

    /// `getArguments()`.
    pub fn get_arguments(&self) -> Vec<Value> {
        self.arguments_event.get_arguments()
    }

    /// `getNamedArguments()`.
    pub fn get_named_arguments(&self) -> PhpArray {
        self.arguments_event.get_named_arguments()
    }
}

/// `ControllerMetadata`, or its subclass `ControllerArgumentsMetadata`.
#[derive(Clone)]
pub enum ControllerMetadata {
    /// After the controller was resolved.
    Controller(ControllerEvent),
    /// After its arguments were resolved.
    Arguments(ControllerArgumentsMetadata),
}

impl ControllerMetadata {
    fn controller_event(&self) -> &ControllerEvent {
        match self {
            Self::Controller(event) => event,
            Self::Arguments(metadata) => &metadata.controller_event,
        }
    }

    /// `getController()`.
    pub fn get_controller(&self) -> Controller {
        self.controller_event().get_controller()
    }

    /// `getReflector()`.
    pub fn get_reflector(&self) -> Arc<[Parameter]> {
        self.controller_event().get_controller_reflector()
    }

    /// `getAttributes($className)`.
    pub fn get_attributes(&self, class_name: Option<&str>) -> PhpArray {
        self.controller_event().get_attributes(class_name)
    }

    /// `$metadata instanceof ControllerArgumentsMetadata ? $metadata : null`.
    pub fn as_arguments(&self) -> Option<&ControllerArgumentsMetadata> {
        match self {
            Self::Controller(_) => None,
            Self::Arguments(metadata) => Some(metadata),
        }
    }
}

/// `ViewEvent`: the controller returned something other than a response;
/// a listener sets one.
pub struct ViewEvent {
    request_event: RequestEvent,
    controller_result: Value,
    /// `$event->controllerMetadata`.
    pub controller_metadata: Option<ControllerArgumentsMetadata>,
}

impl ViewEvent {
    pub fn new(
        kernel: KernelRef,
        request: RequestHandle,
        request_type: i64,
        controller_result: Value,
        controller_metadata: Option<ControllerArgumentsMetadata>,
    ) -> Self {
        Self {
            request_event: RequestEvent::new(kernel, request, Some(request_type)),
            controller_result,
            controller_metadata,
        }
    }

    /// `getControllerResult()`.
    pub fn get_controller_result(&self) -> &Value {
        &self.controller_result
    }

    /// `setControllerResult($controllerResult)`.
    pub fn set_controller_result(&mut self, controller_result: Value) {
        self.controller_result = controller_result;
    }

    /// `getResponse()`.
    pub fn get_response(&self) -> Option<&ResponseHandle> {
        self.request_event.get_response()
    }

    /// `setResponse($response)`: also stops propagation.
    pub fn set_response(&mut self, response: ResponseHandle) {
        self.request_event.set_response(response);
    }

    /// `hasResponse()`.
    pub fn has_response(&self) -> bool {
        self.request_event.has_response()
    }
}

kernel_event!(ViewEvent, "ViewEvent", request_event.base);

/// `ResponseEvent`: listeners may change or replace the response.
pub struct ResponseEvent {
    base: KernelEvent,
    response: ResponseHandle,
    /// `$event->controllerMetadata`.
    pub controller_metadata: Option<ControllerArgumentsMetadata>,
}

impl ResponseEvent {
    pub fn new(
        kernel: KernelRef,
        request: RequestHandle,
        request_type: i64,
        response: ResponseHandle,
        controller_metadata: Option<ControllerArgumentsMetadata>,
    ) -> Self {
        Self {
            base: KernelEvent::new(kernel, request, Some(request_type)),
            response,
            controller_metadata,
        }
    }

    /// `getResponse()`.
    pub fn get_response(&self) -> &ResponseHandle {
        &self.response
    }

    /// `setResponse($response)`.
    pub fn set_response(&mut self, response: ResponseHandle) {
        self.response = response;
    }
}

kernel_event!(ResponseEvent, "ResponseEvent", base);

/// `FinishRequestEvent`: the request is done (a response or an exception).
pub struct FinishRequestEvent {
    base: KernelEvent,
    /// `$event->controllerMetadata`.
    pub controller_metadata: Option<ControllerMetadata>,
}

impl FinishRequestEvent {
    pub fn new(
        kernel: KernelRef,
        request: RequestHandle,
        request_type: Option<i64>,
        controller_metadata: Option<ControllerMetadata>,
    ) -> Self {
        Self {
            base: KernelEvent::new(kernel, request, request_type),
            controller_metadata,
        }
    }
}

kernel_event!(FinishRequestEvent, "FinishRequestEvent", base);

/// `TerminateEvent`: after the response was sent.
pub struct TerminateEvent {
    base: KernelEvent,
    response: ResponseHandle,
}

impl TerminateEvent {
    pub fn new(kernel: KernelRef, request: RequestHandle, response: ResponseHandle) -> Self {
        Self {
            base: KernelEvent::new(
                kernel,
                request,
                Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
            ),
            response,
        }
    }

    /// `getResponse()`.
    pub fn get_response(&self) -> &ResponseHandle {
        &self.response
    }
}

kernel_event!(TerminateEvent, "TerminateEvent", base);

/// `ExceptionEvent`: a listener may set a response for the throwable, or
/// replace the throwable.
pub struct ExceptionEvent {
    request_event: RequestEvent,
    throwable: Throwable,
    allow_custom_response_code: bool,
    is_kernel_terminating: bool,
    /// `$event->controllerMetadata`.
    pub controller_metadata: Option<ControllerMetadata>,
}

impl ExceptionEvent {
    pub fn new(
        kernel: KernelRef,
        request: RequestHandle,
        request_type: i64,
        throwable: Throwable,
        is_kernel_terminating: bool,
        controller_metadata: Option<ControllerMetadata>,
    ) -> Self {
        Self {
            request_event: RequestEvent::new(kernel, request, Some(request_type)),
            throwable,
            allow_custom_response_code: false,
            is_kernel_terminating,
            controller_metadata,
        }
    }

    /// `getThrowable()`.
    pub fn get_throwable(&self) -> &Throwable {
        &self.throwable
    }

    /// `setThrowable($exception)`.
    pub fn set_throwable(&mut self, exception: Throwable) {
        self.throwable = exception;
    }

    /// `allowCustomResponseCode()`: the response keeps its own status.
    pub fn allow_custom_response_code(&mut self) {
        self.allow_custom_response_code = true;
    }

    /// `isAllowingCustomResponseCode()`.
    pub fn is_allowing_custom_response_code(&self) -> bool {
        self.allow_custom_response_code
    }

    /// `isKernelTerminating()`.
    pub fn is_kernel_terminating(&self) -> bool {
        self.is_kernel_terminating
    }

    /// `getResponse()`.
    pub fn get_response(&self) -> Option<&ResponseHandle> {
        self.request_event.get_response()
    }

    /// `setResponse($response)`: also stops propagation.
    pub fn set_response(&mut self, response: ResponseHandle) {
        self.request_event.set_response(response);
    }

    /// `hasResponse()`.
    pub fn has_response(&self) -> bool {
        self.request_event.has_response()
    }
}

kernel_event!(ExceptionEvent, "ExceptionEvent", request_event.base);
