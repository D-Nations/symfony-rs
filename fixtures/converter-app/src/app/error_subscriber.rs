//! `Fixture\EventSubscriber\ErrorSubscriber` (oracle/apps/converter-fixture/src/EventSubscriber/ErrorSubscriber.php).
//!
//! A `kernel.exception` subscriber at priority 0, registered at start-up
//! (`Send + Sync`, converter contract 0.1, section 7). It holds no state.
//!
//! Upstream: none

use axum::http::{HeaderName, StatusCode};
use symfony_http_kernel::{Exception, ExceptionEvent, Listener};

use super::greeting_exception::GreetingException;
use crate::runtime::http_foundation::new_response;

pub struct ErrorSubscriber;

impl<C> Listener<C> for ErrorSubscriber {
    fn exception(&self, event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
        let throwable = &*event.exception;
        let is_greeting = throwable
            .throwable
            .as_ref()
            .is_some_and(|t| t.downcast_ref::<GreetingException>().is_some());
        // match (true) { GreetingException => [422, []], HttpExceptionInterface => [status, headers], default => [null, []] }
        let (status, headers) = if is_greeting {
            (StatusCode::UNPROCESSABLE_ENTITY, Vec::new())
        } else if throwable.is_http() {
            let headers: Vec<(HeaderName, String)> = throwable
                .headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            (throwable.status, headers)
        } else {
            return Ok(());
        };
        let body = format!("error {}: {}\n", status.as_u16(), throwable.detail);
        // ['Content-Type' => 'text/plain'] + $headers: `+` drops a header
        // keyed exactly 'Content-Type'; ResponseHeaderBag would then let a
        // differently cased duplicate replace it. HeaderMap has lost the
        // key's case, so this assumes Symfony's own spelling, 'Content-Type'.
        let mut all: Vec<(&str, &str)> = vec![("Content-Type", "text/plain")];
        for (name, value) in &headers {
            if !name.as_str().eq_ignore_ascii_case("content-type") {
                all.push((name.as_str(), value.as_str()));
            }
        }
        let response = new_response(body.as_bytes(), status, &all)?;
        event.set_response(response);
        Ok(())
    }
}
