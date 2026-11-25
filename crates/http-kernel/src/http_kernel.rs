//! Port of Symfony\Component\HttpKernel\HttpKernel.php,
//! HttpKernelInterface.php, TerminableInterface.php and KernelEvents.php:
//! the request lifecycle on `HttpFoundation`'s Request and Response, through
//! the `EventDispatcher`.
//!
//! `handle()` pushes the request on the request stack, dispatches
//! `kernel.request`, resolves the controller (`kernel.controller`) and its
//! arguments (`kernel.controller_arguments`), calls it (`kernel.view` for
//! a result that is not a response), filters the response
//! (`kernel.response`, then `kernel.finish_request`) and pops the request.
//! A throwable goes to `kernel.exception`; `\Error`s pass through unless
//! the kernel handles all throwables, and an `HttpFoundation` request
//! exception becomes a `BadRequestHttpException`. `terminate()` dispatches
//! `kernel.terminate`; `terminateWithException()` sends an error response
//! for a throwable that escaped.
//!
//! The kernel is shared (`Arc`), as the events hand it to listeners.
//! Not ported: the default `ArgumentResolver` (one is passed in), and
//! the `StreamedResponse` callback that re-pushes the request (no
//! `StreamedResponse` yet). `X-Php-Ob-Level` is the number of output buffers
//! in `php::Output`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel HttpKernel.php HttpKernelInterface.php TerminableInterface.php KernelEvents.php

use std::sync::{
    Arc, Mutex, PoisonError, Weak,
    atomic::{AtomicBool, Ordering},
};

use php_value::{Handle, PhpString, Value, numeric::float_to_string};
use symfony_event_dispatcher::{ContractsEventDispatcherInterface, EventObject};
use symfony_http_foundation::{
    exception::{Class, Error as FoundationError},
    header_bag::HeaderValues,
    php::with_output,
    request_stack::{RequestHandle, RequestStack},
};

use crate::{
    controller::{
        ArgumentResolverInterface, ControllerResolverInterface, ControllerResult, ResponseHandle,
    },
    event::{
        ControllerArgumentsEvent, ControllerArgumentsMetadata, ControllerEvent, ControllerMetadata,
        ExceptionEvent, FinishRequestEvent, RequestEvent, ResponseEvent, TerminateEvent, ViewEvent,
    },
    exception::{
        ControllerDoesNotReturnResponseException, HttpArgs, HttpException,
        REQUEST_EXCEPTION_INTERFACE, Throwable, instance_of, message_of,
    },
};

/// `KernelEvents`: the names of the events the kernel dispatches.
pub struct KernelEvents;

impl KernelEvents {
    /// At the very beginning of request dispatching.
    pub const REQUEST: &'static str = "kernel.request";
    /// When an uncaught exception occurs.
    pub const EXCEPTION: &'static str = "kernel.exception";
    /// When a controller has been found for handling a request.
    pub const CONTROLLER: &'static str = "kernel.controller";
    /// When controller arguments have been resolved.
    pub const CONTROLLER_ARGUMENTS: &'static str = "kernel.controller_arguments";
    /// When the return value of a controller is not a Response instance.
    pub const VIEW: &'static str = "kernel.view";
    /// After a response was created by replying to a request.
    pub const RESPONSE: &'static str = "kernel.response";
    /// After a response was generated for a request.
    pub const FINISH_REQUEST: &'static str = "kernel.finish_request";
    /// After a response was sent.
    pub const TERMINATE: &'static str = "kernel.terminate";

    /// `ALIASES`: event class to event name.
    pub const ALIASES: [(&'static str, &'static str); 8] = [
        (ControllerArgumentsEvent::CLASS, Self::CONTROLLER_ARGUMENTS),
        (ControllerEvent::CLASS, Self::CONTROLLER),
        (ResponseEvent::CLASS, Self::RESPONSE),
        (FinishRequestEvent::CLASS, Self::FINISH_REQUEST),
        (RequestEvent::CLASS, Self::REQUEST),
        (ViewEvent::CLASS, Self::VIEW),
        (ExceptionEvent::CLASS, Self::EXCEPTION),
        (TerminateEvent::CLASS, Self::TERMINATE),
    ];
}

/// `HttpKernelInterface`: handles a request to convert it to a response.
pub trait HttpKernelInterface: Send + Sync {
    /// `handle($request, $type, $catch)`: with `catch`, a throwable becomes
    /// a response where a listener provides one.
    fn handle(
        &self,
        request: &RequestHandle,
        request_type: i64,
        catch: bool,
    ) -> Result<ResponseHandle, Throwable>;
}

impl dyn HttpKernelInterface {
    pub const MAIN_REQUEST: i64 = 1;
    pub const SUB_REQUEST: i64 = 2;
}

/// A kernel as the events hold it.
pub type KernelRef = Arc<dyn HttpKernelInterface>;

/// `$a === $b` for two kernels.
pub fn same_kernel(a: &KernelRef, b: &KernelRef) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

/// `TerminableInterface`.
pub trait TerminableInterface {
    /// `terminate($request, $response)`: work after the response was sent.
    fn terminate(
        &self,
        request: &RequestHandle,
        response: &ResponseHandle,
    ) -> Result<(), Throwable>;
}

/// The constructor's arguments after the dispatcher and the resolver.
pub struct Options {
    /// `?RequestStack $requestStack` (a new stack by default).
    pub request_stack: Option<RequestStack>,
    /// `ArgumentResolverInterface $argumentResolver`; PHP's default
    /// `ArgumentResolver` is not ported, so one is required.
    pub argument_resolver: Arc<dyn ArgumentResolverInterface>,
    /// `bool $handleAllThrowables`.
    pub handle_all_throwables: bool,
}

/// `Symfony\Component\HttpKernel\HttpKernel`.
pub struct HttpKernel {
    this: Weak<HttpKernel>,
    dispatcher: Arc<dyn ContractsEventDispatcherInterface>,
    resolver: Arc<dyn ControllerResolverInterface>,
    request_stack: RequestStack,
    argument_resolver: Arc<dyn ArgumentResolverInterface>,
    handle_all_throwables: bool,
    terminating: AtomicBool,
    /// `$this->terminateException` (a weak reference).
    terminate_exception: Mutex<Option<Weak<dyn std::error::Error + Send + Sync>>>,
}

/// `$e instanceof \Error`.
fn is_error(e: &Throwable) -> bool {
    instance_of(e.as_ref(), "Error")
}

fn dispatch(
    dispatcher: &dyn ContractsEventDispatcherInterface,
    event: &mut dyn EventObject,
    name: &str,
) -> Result<(), Throwable> {
    dispatcher
        .dispatch(event, Some(name))
        .map_err(crate::exception::from_box)
}

fn foundation(error: FoundationError) -> Throwable {
    Arc::new(error)
}

impl HttpKernel {
    /// `new HttpKernel($dispatcher, $resolver, $requestStack,
    /// $argumentResolver, $handleAllThrowables)`.
    pub fn new(
        dispatcher: Arc<dyn ContractsEventDispatcherInterface>,
        resolver: Arc<dyn ControllerResolverInterface>,
        options: Options,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            dispatcher,
            resolver,
            request_stack: options.request_stack.unwrap_or_default(),
            argument_resolver: options.argument_resolver,
            handle_all_throwables: options.handle_all_throwables,
            terminating: AtomicBool::new(false),
            terminate_exception: Mutex::new(None),
        })
    }

    /// The kernel as the events hold it.
    fn kernel(&self) -> KernelRef {
        match self.this.upgrade() {
            Some(kernel) => kernel,
            // Only reachable while the last Arc is being dropped, when no
            // request can be handled.
            None => Arc::new(Unavailable),
        }
    }

    /// The request stack.
    pub fn request_stack(&self) -> &RequestStack {
        &self.request_stack
    }

    /// `handle($request, $type, $catch)`.
    pub fn handle_request(
        &self,
        request: &RequestHandle,
        request_type: i64,
        catch: bool,
    ) -> Result<ResponseHandle, Throwable> {
        let level = with_output(|o| o.buffers.len());
        request.with_mut(|r| {
            // A plain value never fails.
            let _ = r
                .headers_mut()
                .set(b"X-Php-Ob-Level", level.to_string().as_str(), true);
        });

        self.request_stack.push(request.clone());
        let mut metadata = None;
        let result = match self.handle_raw(request, request_type, &mut metadata) {
            Ok(response) => Ok(response),
            Err(e) if is_error(&e) && !self.handle_all_throwables => Err(e),
            Err(e) => {
                let e = if instance_of(e.as_ref(), REQUEST_EXCEPTION_INTERFACE) {
                    HttpException::bad_request(HttpArgs {
                        message: message_of(e.as_ref()),
                        previous: Some(e),
                        ..HttpArgs::default()
                    })
                    .throwable()
                } else {
                    e
                };
                if catch {
                    self.handle_throwable(e, request, request_type, metadata)
                } else {
                    self.finish_request(request, request_type, metadata)
                        .and(Err(e))
                }
            }
        };
        self.request_stack.pop();
        result
    }

    /// `terminateWithException($exception, $request)`: an error response
    /// for a throwable that escaped, sent, then `terminate()`.
    pub fn terminate_with_exception(
        &self,
        exception: Throwable,
        request: Option<RequestHandle>,
    ) -> Result<(), Throwable> {
        let Some(request) = request.or_else(|| self.request_stack.get_main_request()) else {
            return Err(exception);
        };

        let pop = !self
            .request_stack
            .get_main_request()
            .is_some_and(|main| main.same(&request));
        if pop {
            self.request_stack.push(request.clone());
        }

        let terminating = self.terminating.load(Ordering::SeqCst);
        let was_terminate_exception = self
            .terminate_exception
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|stored| Arc::ptr_eq(&stored, &exception));
        self.terminating
            .store(terminating || was_terminate_exception, Ordering::SeqCst);

        let result = self.handle_throwable(
            exception,
            &request,
            <dyn HttpKernelInterface>::MAIN_REQUEST,
            None,
        );
        self.terminating.store(terminating, Ordering::SeqCst);
        if pop {
            self.request_stack.pop();
        }
        let response = result?;

        if !with_output(|o| o.headers_sent) {
            response
                .with_mut(|r| r.send_headers(None).map(|_| ()))
                .map_err(foundation)?;
        }
        response.with_mut(|r| {
            r.send_content();
        });

        self.terminate(&request, &response)
    }

    /// `handleRaw()`: exceptions are not caught.
    fn handle_raw(
        &self,
        request: &RequestHandle,
        request_type: i64,
        metadata: &mut Option<ControllerMetadata>,
    ) -> Result<ResponseHandle, Throwable> {
        let kernel = self.kernel();

        // request
        let mut event = RequestEvent::new(kernel.clone(), request.clone(), Some(request_type));
        dispatch(self.dispatcher.as_ref(), &mut event, KernelEvents::REQUEST)?;

        if let Some(response) = event.get_response() {
            return self.filter_response(response.clone(), request, request_type, None);
        }

        // load controller
        let Some(controller) = self.resolver.get_controller(request)? else {
            let path = request
                .with_mut(symfony_http_foundation::request::Request::get_path_info)
                .map_err(foundation)?;
            let mut message = b"Unable to find the controller for path \"".to_vec();
            message.extend_from_slice(path.as_bytes());
            message.extend_from_slice(b"\". The route is wrongly configured.");
            return Err(HttpException::not_found(HttpArgs::message(message)).throwable());
        };

        let mut controller_event = ControllerEvent::new(
            kernel.clone(),
            controller,
            request.clone(),
            Some(request_type),
        );
        *metadata = Some(ControllerMetadata::Controller(controller_event.clone()));
        dispatch(
            self.dispatcher.as_ref(),
            &mut controller_event,
            KernelEvents::CONTROLLER,
        )?;
        let controller = controller_event.get_controller();

        // controller arguments
        let arguments = self.argument_resolver.get_arguments(
            request,
            &controller,
            &controller_event.get_controller_reflector(),
        )?;

        let mut arguments_event = ControllerArgumentsEvent::new(
            kernel.clone(),
            controller_event.clone(),
            arguments,
            request.clone(),
            Some(request_type),
        );
        let arguments_metadata =
            ControllerArgumentsMetadata::new(controller_event, arguments_event.clone());
        *metadata = Some(ControllerMetadata::Arguments(arguments_metadata.clone()));
        dispatch(
            self.dispatcher.as_ref(),
            &mut arguments_event,
            KernelEvents::CONTROLLER_ARGUMENTS,
        )?;
        let controller = arguments_event.get_controller();
        let arguments = arguments_event.get_arguments();

        // call controller
        let response = match controller.call(&arguments)? {
            ControllerResult::Response(response) => response,
            // view
            ControllerResult::Value(result) => {
                let mut event = ViewEvent::new(
                    kernel,
                    request.clone(),
                    request_type,
                    result.clone(),
                    Some(arguments_metadata),
                );
                dispatch(self.dispatcher.as_ref(), &mut event, KernelEvents::VIEW)?;

                if let Some(response) = event.get_response() {
                    response.clone()
                } else {
                    let mut message = b"The controller must return a \"Symfony\\Component\\HttpFoundation\\Response\" object but it returned ".to_vec();
                    message.extend_from_slice(var_to_string(&result).as_bytes());
                    message.push(b'.');

                    // the user may have forgotten to return something
                    if result.is_null() {
                        message.extend_from_slice(
                            b" Did you forget to add a return statement somewhere in your controller?",
                        );
                    }

                    return Err(Arc::new(ControllerDoesNotReturnResponseException::new(
                        message,
                    )));
                }
            }
        };

        self.filter_response(response, request, request_type, metadata.clone())
    }

    /// `filterResponse()`: `kernel.response`, then `kernel.finish_request`.
    fn filter_response(
        &self,
        response: ResponseHandle,
        request: &RequestHandle,
        request_type: i64,
        metadata: Option<ControllerMetadata>,
    ) -> Result<ResponseHandle, Throwable> {
        let mut event = ResponseEvent::new(
            self.kernel(),
            request.clone(),
            request_type,
            response,
            metadata
                .as_ref()
                .and_then(ControllerMetadata::as_arguments)
                .cloned(),
        );

        dispatch(self.dispatcher.as_ref(), &mut event, KernelEvents::RESPONSE)?;

        self.finish_request(request, request_type, metadata)?;

        Ok(event.get_response().clone())
    }

    /// `finishRequest()`: `kernel.finish_request`.
    fn finish_request(
        &self,
        request: &RequestHandle,
        request_type: i64,
        metadata: Option<ControllerMetadata>,
    ) -> Result<(), Throwable> {
        let mut event =
            FinishRequestEvent::new(self.kernel(), request.clone(), Some(request_type), metadata);
        dispatch(
            self.dispatcher.as_ref(),
            &mut event,
            KernelEvents::FINISH_REQUEST,
        )
    }

    /// `handleThrowable()`: a response from a `kernel.exception` listener,
    /// with the throwable's status unless the listener allowed its own.
    fn handle_throwable(
        &self,
        e: Throwable,
        request: &RequestHandle,
        request_type: i64,
        metadata: Option<ControllerMetadata>,
    ) -> Result<ResponseHandle, Throwable> {
        let mut event = ExceptionEvent::new(
            self.kernel(),
            request.clone(),
            request_type,
            e,
            self.terminating.load(Ordering::SeqCst),
            metadata.clone(),
        );
        dispatch(
            self.dispatcher.as_ref(),
            &mut event,
            KernelEvents::EXCEPTION,
        )?;

        // a listener might have replaced the exception
        let e = event.get_throwable().clone();

        let Some(response) = event.get_response().cloned() else {
            self.finish_request(request, request_type, metadata)?;

            return Err(e);
        };

        // the developer asked for a specific status code
        let keep = event.is_allowing_custom_response_code()
            || response.with(|r| r.is_client_error() || r.is_server_error() || r.is_redirect(None));
        if !keep {
            // ensure that we actually have an error response
            if let Some(http) = e.downcast_ref::<HttpException>() {
                // keep the HTTP status code and headers
                let headers = header_values(http)?;
                response
                    .with_mut(|r| {
                        r.set_status_code(http.get_status_code(), None)?;
                        r.headers_mut().add(headers)
                    })
                    .map_err(foundation)?;
            } else {
                response
                    .with_mut(|r| r.set_status_code(500, None).map(|_| ()))
                    .map_err(foundation)?;
            }
        }

        match self.filter_response(response.clone(), request, request_type, metadata) {
            Ok(filtered) => Ok(filtered),
            Err(e) if is_error(&e) && !self.handle_all_throwables => Err(e),
            Err(_) => Ok(response),
        }
    }
}

impl HttpKernelInterface for HttpKernel {
    fn handle(
        &self,
        request: &RequestHandle,
        request_type: i64,
        catch: bool,
    ) -> Result<ResponseHandle, Throwable> {
        self.handle_request(request, request_type, catch)
    }
}

impl TerminableInterface for HttpKernel {
    fn terminate(
        &self,
        request: &RequestHandle,
        response: &ResponseHandle,
    ) -> Result<(), Throwable> {
        self.terminating.store(true, Ordering::SeqCst);
        let mut event = TerminateEvent::new(self.kernel(), request.clone(), response.clone());
        let result = dispatch(
            self.dispatcher.as_ref(),
            &mut event,
            KernelEvents::TERMINATE,
        );
        if let Err(e) = &result {
            *self
                .terminate_exception
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(Arc::downgrade(e));
        }
        self.terminating.store(false, Ordering::SeqCst);
        result
    }
}

/// `$response->headers->add($e->getHeaders())`'s values: strings, scalars
/// as PHP's string conversion gives them, lists of those.
fn header_values(http: &HttpException) -> Result<Vec<(Vec<u8>, HeaderValues)>, Throwable> {
    let refuse = || {
        foundation(FoundationError::new(
            Class::NotPorted,
            "an HttpException header value that is not a string, a scalar or a list of them",
        ))
    };
    let text = |value: &Value| match value {
        Value::Null => Ok(None),
        Value::Array(_) | Value::Object(_) => Err(refuse()),
        scalar => Ok(scalar.to_php_string()),
    };
    let mut headers = Vec::new();
    for (key, value) in http.get_headers().iter() {
        let name = key
            .to_value()
            .to_php_string()
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        let values = match value {
            Value::Array(list) => HeaderValues::Many(
                list.iter()
                    .map(|(_, v)| text(v))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            other => HeaderValues::One(text(other)?),
        };
        headers.push((name, values));
    }
    Ok(headers)
}

/// `varToString($var)`: a human-readable form of what a controller
/// returned.
fn var_to_string(var: &Value) -> PhpString {
    match var {
        Value::Object(object) => PhpString::from(format!("an object of type {}", object.class())),
        Value::Array(array) => {
            let keys: Vec<String> = array
                .keys()
                .map(|k| {
                    format!(
                        "{} => ...",
                        k.to_value()
                            .to_php_string()
                            .unwrap_or_default()
                            .to_string_lossy()
                    )
                })
                .collect();
            let joined = keys.join(", ");
            PhpString::from(format!("an array ([{}])", mb_substr(&joined, 255)))
        }
        Value::Null => PhpString::from("null"),
        Value::Bool(false) => PhpString::from("a boolean value (false)"),
        Value::Bool(true) => PhpString::from("a boolean value (true)"),
        Value::String(text) => {
            let text = text.to_string_lossy();
            let more = if text.chars().count() > 255 {
                "..."
            } else {
                ""
            };
            PhpString::from(format!("a string (\"{}{more}\")", mb_substr(&text, 255)))
        }
        Value::Int(i) => PhpString::from(format!("a number ({i})")),
        Value::Float(f) => PhpString::from(format!("a number ({})", float_to_string(*f))),
    }
}

/// `mb_substr($text, 0, $length)`.
fn mb_substr(text: &str, length: usize) -> String {
    text.chars().take(length).collect()
}

/// The kernel after its last reference was dropped.
struct Unavailable;

impl HttpKernelInterface for Unavailable {
    fn handle(
        &self,
        _request: &RequestHandle,
        _request_type: i64,
        _catch: bool,
    ) -> Result<ResponseHandle, Throwable> {
        Err(crate::exception::PhpException::new(
            crate::exception::PhpClass::Error,
            "the kernel has been dropped",
        )
        .throwable())
    }
}

/// `new Response(...)` as a handle, for listeners and controllers.
pub fn response_handle(response: symfony_http_foundation::response::Response) -> ResponseHandle {
    Handle::new(response)
}
