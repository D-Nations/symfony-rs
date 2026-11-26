//! Port of Symfony\Component\HttpFoundation\EventStreamResponse.php: a
//! `StreamedResponse` for Server-Sent Events, with the event-stream
//! headers, a callback whose events are sent as they come, and the retry
//! a response gives events that have none.
//!
//! A [`Response`] carrying a `Stream` with an [`EventStream`]: the
//! subclass's state (`$retry`, `$lastRetry`), shared with the callback,
//! which PHP gives `$this`. [`Response::stream`]'s `event_stream()` is
//! `instanceof EventStreamResponse`.
//!
//! The callback returns the events lazily (PHP's generator, which may
//! sleep between events), or `None` when it sent them itself with
//! [`EventStream::send_event`]. After each event, `connection_aborted()`
//! (this thread's `php::Output`, which the HTTP adapter sets when the
//! client is gone) ends the loop.
//!
//! Not reproduced: an event callback given to `setCallback()` once a
//! callback is set is called without `$this` and its result ignored, as
//! PHP's parent `setCallback()` does; a PHP callback that declares the
//! response parameter would then throw an `ArgumentCountError` instead.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation EventStreamResponse.php

use std::sync::{Arc, Mutex, PoisonError};

use crate::{
    exception::Result,
    header_bag::{HeaderValues, ResponseHeaderBag},
    php::with_output,
    response::Response,
    server_event::ServerEvent,
    streamed_response::{CallbackOrChunks, Stream, Throwable, echo},
};

/// `EventStreamResponse::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\EventStreamResponse";

/// What an event callback yields: events in order, or the throwable that
/// ends the generator.
pub type Events = Box<dyn Iterator<Item = std::result::Result<ServerEvent, Throwable>>>;

/// The callback of an event stream: it is given the response and returns
/// its events, or `None` when it sent them itself.
pub type EventCallback =
    Arc<dyn Fn(&EventStream) -> std::result::Result<Option<Events>, Throwable> + Send + Sync>;

#[derive(Debug, Default)]
struct Retries {
    /// `$this->retry`.
    retry: Option<i64>,
    /// `$this->lastRetry`.
    last_retry: Option<i64>,
}

/// The `EventStreamResponse` state, and its methods that the callback
/// calls on `$this`.
#[derive(Debug, Clone, Default)]
pub struct EventStream(Arc<Mutex<Retries>>);

impl PartialEq for EventStream {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl EventStream {
    fn retries(&self) -> std::sync::MutexGuard<'_, Retries> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `sendEvent($event)`: the event's parts echoed, each flushed to the
    /// client outside the CLI. An event without a retry gets the
    /// response's when it differs from the last one sent.
    pub fn send_event(&self, event: &mut ServerEvent) -> &Self {
        {
            let mut retries = self.retries();
            if let Some(retry) = event.get_retry().filter(|r| *r > 0) {
                retries.last_retry = Some(retry);
            } else if let Some(retry) = retries.retry.filter(|r| *r > 0)
                && retries.last_retry != Some(retry)
            {
                retries.last_retry = Some(retry);
                event.set_retry(Some(retry));
            }
        }

        let cli = with_output(|o| matches!(o.sapi.as_bytes(), b"cli" | b"phpdbg" | b"embed"));
        for part in event.parts() {
            echo(part);

            if !cli {
                Response::close_output_buffers(0, true);
                crate::php::flush();
            }
        }

        self
    }

    /// `getRetry()`.
    pub fn get_retry(&self) -> Option<i64> {
        self.retries().retry
    }

    /// `setRetry($retry)`.
    pub fn set_retry(&self, retry: i64) {
        self.retries().retry = Some(retry);
    }

    /// `setCallback($callback)` before a callback is set: the events the
    /// callback returns are sent one by one, until the client is gone.
    pub(crate) fn wrap(&self, callback: EventCallback) -> crate::streamed_response::Callback {
        let stream = self.clone();
        Arc::new(move || {
            if let Some(events) = callback(&stream)? {
                for event in events {
                    let mut event = event?;
                    stream.send_event(&mut event);

                    if with_output(|o| o.connection_aborted) {
                        break;
                    }
                }
            }
            Ok(())
        })
    }
}

/// The headers `__construct()` adds with `$headers += [...]`.
const DEFAULT_HEADERS: [(&str, &str); 7] = [
    ("Connection", "keep-alive"),
    ("Content-Type", "text/event-stream"),
    (
        "Cache-Control",
        "private, no-cache, no-store, must-revalidate, max-age=0",
    ),
    ("X-Accel-Buffering", "no"),
    ("Incremental", "?1"),
    ("Pragma", "no-cache"),
    ("Expires", "0"),
];

/// `new EventStreamResponse($callback, $status, $headers, $retry)`.
/// `headers` is PHP's array: the defaults are added under the keys it
/// lacks exactly, so a key in another case is kept and then replaced by
/// the default, as PHP's bag does.
///
/// Fails with `\InvalidArgumentException` when the status code is not
/// valid.
pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
    callback: Option<EventCallback>,
    status: i64,
    headers: impl IntoIterator<Item = (K, V)>,
    retry: Option<i64>,
) -> Result<Response> {
    let mut all: Vec<(Vec<u8>, HeaderValues)> = headers
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_vec(), v.into()))
        .collect();
    for (name, value) in DEFAULT_HEADERS {
        if !all.iter().any(|(k, _)| k == name.as_bytes()) {
            all.push((name.as_bytes().to_vec(), HeaderValues::from(value)));
        }
    }
    let events = EventStream::default();
    events.retries().retry = retry;
    let mut response = crate::streamed_response::new_with_events(
        CallbackOrChunks::None,
        status,
        ResponseHeaderBag::new(all)?,
        events.clone(),
    )?;
    if let (Some(callback), Some(stream)) = (callback, response.stream_mut()) {
        stream.set_event_callback(callback);
    }
    Ok(response)
}

impl Stream {
    /// `setCallback($callback)` with an event callback. Before a callback
    /// is set, an `EventStreamResponse` sends what it returns; otherwise
    /// (and on a plain `StreamedResponse`) it is called and its result
    /// ignored, as the parent's `setCallback()` does.
    pub fn set_event_callback(&mut self, callback: EventCallback) -> &mut Self {
        if let (Some(events), None) = (self.event_stream(), self.get_callback()) {
            let wrapped = events.wrap(callback);
            return self.set_callback(wrapped);
        }
        let events = self.event_stream().cloned().unwrap_or_default();
        self.set_callback(Arc::new(move || callback(&events).map(|_| ())))
    }
}
