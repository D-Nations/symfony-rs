//! `Fixture\EventSubscriber\ErrorSubscriber` (oracle/apps/converter-fixture/src/EventSubscriber/ErrorSubscriber.php).
//!
//! A `kernel.exception` subscriber at priority 0, added to the event
//! dispatcher when the kernel is built (`Send + Sync`, converter contract
//! 0.3, section 7). It holds no state.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpString, Value};
use symfony_event_dispatcher::{
    Callable, EventDispatcherInterface, EventObject, EventSubscriber, Subscription, UndefinedMethod,
};
use symfony_http_foundation::{
    header_bag::{HeaderValues, ResponseHeaderBag},
    response::Response,
};
use symfony_http_kernel::{
    event::ExceptionEvent,
    exception::{HttpException, Throwable, message_of},
    http_kernel::{KernelEvents, response_handle},
};

use super::greeting_exception::GreetingException;
use crate::runtime::NotPorted;

pub const CLASS: &str = "Fixture\\EventSubscriber\\ErrorSubscriber";

#[derive(Debug, Default)]
pub struct ErrorSubscriber;

fn throwable(error: impl std::error::Error + Send + Sync + 'static) -> Throwable {
    Arc::new(error)
}

impl ErrorSubscriber {
    /// `onException(ExceptionEvent $event)`.
    pub fn on_exception(&self, event: &mut ExceptionEvent) -> Result<(), Throwable> {
        let error = event.get_throwable().clone();
        // match (true) { GreetingException => [422, []], HttpExceptionInterface => [status, headers], default => [null, []] }
        let (status, headers) = if error.downcast_ref::<GreetingException>().is_some() {
            (422, Vec::new())
        } else if let Some(http) = error.downcast_ref::<HttpException>() {
            (http.get_status_code(), header_values(http)?)
        } else {
            return Ok(());
        };
        let mut body = format!("error {status}: ").into_bytes();
        body.extend_from_slice(message_of(error.as_ref()).as_bytes());
        body.push(b'\n');
        // ['Content-Type' => 'text/plain'] + $headers: `+` keeps the left
        // operand's 'Content-Type' key (exact bytes, as array keys compare).
        let mut all = vec![(b"Content-Type".to_vec(), HeaderValues::from("text/plain"))];
        all.extend(
            headers
                .into_iter()
                .filter(|(name, _)| name.as_slice() != b"Content-Type"),
        );
        let headers = ResponseHeaderBag::new(all).map_err(throwable)?;
        let response =
            Response::new(Some(PhpString::from(body)), status, headers).map_err(throwable)?;
        event.set_response(response_handle(response));
        Ok(())
    }
}

/// `$throwable->getHeaders()` as `new Response()` reads them: a string or
/// scalar value, or a list of them.
fn header_values(http: &HttpException) -> Result<Vec<(Vec<u8>, HeaderValues)>, Throwable> {
    let refuse = || {
        throwable(NotPorted(
            "an exception header that is not a scalar or a list of them",
        ))
    };
    let text = |value: &Value| match value {
        Value::Null => Ok(None),
        Value::Array(_) | Value::Object(_) => Err(refuse()),
        scalar => Ok(scalar.to_php_string()),
    };
    let mut headers = Vec::new();
    for (key, value) in http.get_headers().iter() {
        let name = key.to_value().to_php_string().unwrap_or_default().to_vec();
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

impl Callable for ErrorSubscriber {
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
            "onException" => {
                let Some(event) = event.downcast_mut::<ExceptionEvent>() else {
                    return Err(Box::new(NotPorted(
                        "onException() with another event (a TypeError in PHP)",
                    )));
                };
                // The kernel unboxes the shared throwable (`from_box`).
                self.on_exception(event).map_err(|e| Box::new(e) as _)
            }
            other => Err(UndefinedMethod::error(CLASS, other)),
        }
    }
}

/// `getSubscribedEvents()`: `[KernelEvents::EXCEPTION => 'onException']`.
impl EventSubscriber for ErrorSubscriber {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        vec![(KernelEvents::EXCEPTION, Subscription::Method("onException"))]
    }
}
