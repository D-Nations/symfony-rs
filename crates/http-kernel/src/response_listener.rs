//! Port of Symfony\Component\HttpKernel\EventListener\ResponseListener.php:
//! fixes the main request's response from the request. The response gets
//! the kernel's charset when it has none, a `Content-Language` from the
//! request's locale when enabled, `Vary: Accept-Language` when the request
//! asks for it (`_vary_by_language`), and then `Response::prepare()`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel EventListener/ResponseListener.php

use std::sync::Arc;

use php_value::{PhpString, Value};
use symfony_event_dispatcher::{
    Callable, EventDispatcherInterface, EventObject, EventSubscriber, Subscription, UndefinedMethod,
};
use symfony_http_foundation::exception::Error as FoundationError;

use crate::{
    event::ResponseEvent,
    exception::{PhpClass, PhpException, Throwable},
    http_kernel::KernelEvents,
};

/// `Symfony\Component\HttpKernel\EventListener\ResponseListener`.
pub const CLASS: &str = "Symfony\\Component\\HttpKernel\\EventListener\\ResponseListener";

/// `ResponseListener`.
#[derive(Debug, Clone)]
pub struct ResponseListener {
    charset: PhpString,
    add_content_language_header: bool,
}

impl ResponseListener {
    /// `new ResponseListener($charset, $addContentLanguageHeader)`.
    pub fn new(charset: impl Into<PhpString>, add_content_language_header: bool) -> Self {
        Self {
            charset: charset.into(),
            add_content_language_header,
        }
    }

    /// `onKernelResponse($event)`.
    pub fn on_kernel_response(&self, event: &mut ResponseEvent) -> Result<(), Throwable> {
        if !event.is_main_request() {
            return Ok(());
        }

        let response = event.get_response().clone();
        let request = event.get_request().clone();

        response.with_mut(|response| {
            if response.get_charset().is_none() {
                response.set_charset(self.charset.clone());
            }

            if self.add_content_language_header
                && !response.is_informational()
                && !response.is_empty()
                && !response.headers().has("Content-Language")
            {
                let locale = request.with(symfony_http_foundation::request::Request::get_locale);
                response
                    .headers_mut()
                    .set(b"Content-Language", locale, true)
                    .map_err(foundation)?;
            }

            let vary = request.with(|r| {
                r.attributes()
                    .get("_vary_by_language", Value::Null)
                    .is_truthy()
            });
            if vary {
                response
                    .set_vary("Accept-Language", false)
                    .map_err(foundation)?;
            }

            request
                .with_mut(|request| response.prepare(request).map(|_| ()))
                .map_err(foundation)
        })
    }

    /// `getSubscribedEvents()`.
    pub fn get_subscribed_events() -> Vec<(&'static str, Subscription)> {
        vec![(
            KernelEvents::RESPONSE,
            Subscription::Method("onKernelResponse"),
        )]
    }
}

fn foundation(error: FoundationError) -> Throwable {
    Arc::new(error)
}

fn boxed(e: Throwable) -> symfony_event_dispatcher::Throwable {
    Box::new(e)
}

impl Callable for ResponseListener {
    fn class(&self) -> &'static str {
        CLASS
    }

    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        _event_name: &str,
        _dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), symfony_event_dispatcher::Throwable> {
        match method {
            "onKernelResponse" => {
                let Some(event) = event.downcast_mut::<ResponseEvent>() else {
                    return Err(boxed(
                        PhpException::new(
                            PhpClass::TypeError,
                            format!(
                                "{CLASS}::onKernelResponse(): Argument #1 ($event) must be of type {}",
                                ResponseEvent::CLASS
                            ),
                        )
                        .throwable(),
                    ));
                };
                self.on_kernel_response(event).map_err(boxed)
            }
            other => Err(UndefinedMethod::error(CLASS, other)),
        }
    }
}

impl EventSubscriber for ResponseListener {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        Self::get_subscribed_events()
    }
}
