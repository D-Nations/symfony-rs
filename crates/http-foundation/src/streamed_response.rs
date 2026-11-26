//! Port of Symfony\Component\HttpFoundation\StreamedResponse.php: a
//! response whose content is written by a callback (`echo`, `flush()`),
//! or by echoing a list of chunks, when it is sent.
//!
//! PHP's subclass overrides methods that `Response`'s own methods call
//! (`prepare()` and `setNotModified()` call `setContent(null)`; `send()`
//! calls `sendHeaders()` and `sendContent()`), and the kernel keeps every
//! response as a `Response`. So a streamed response is a [`Response`]
//! carrying a [`Stream`] (the subclass's state), and those methods check
//! for it, as PHP's late binding does. [`Response::stream`] is
//! `instanceof StreamedResponse`.
//!
//! The callback is a Rust closure that writes through this thread's
//! `php::Output` ([`echo`], [`flush`]) and may throw. A throwable it
//! returns ends `sendContent()` and `send()`, as in PHP.
//!
//! Chunks are a list: PHP also accepts any iterable (a generator is
//! traversed once, when the response is sent). That is not ported: a
//! generator's chunks are written by a callback instead.
//! `is_callable()` on an array of strings (`['Class', 'method']` names a
//! static method) is not reproduced: a list is always chunks.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation StreamedResponse.php

use std::{fmt, sync::Arc};

use php_value::PhpString;

use crate::{
    exception::Result,
    header_bag::ResponseHeaderBag,
    php::{OutputCall, with_output},
    response::Response,
};

/// `StreamedResponse::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\StreamedResponse";

/// A PHP throwable, shared as the kernel shares it.
pub type Throwable = Arc<dyn std::error::Error + Send + Sync + 'static>;

/// The `\Closure` that writes the content.
pub type Callback = Arc<dyn Fn() -> std::result::Result<(), Throwable> + Send + Sync>;

/// What `new StreamedResponse($callbackOrChunks)` is given.
#[derive(Clone)]
pub enum CallbackOrChunks {
    /// `null`: the callback must be set before the content is sent.
    None,
    /// A callable.
    Callback(Callback),
    /// An iterable of strings; an empty one sets no callback, as PHP's
    /// `elseif ($callbackOrChunks)` finds `[]` falsy.
    Chunks(Vec<PhpString>),
}

/// The state `StreamedResponse` adds to `Response`.
#[derive(Clone)]
pub struct Stream {
    /// `$this->callback`.
    callback: Option<Callback>,
    /// `$this->streamed`: the content was sent, or must not be.
    pub(crate) streamed: bool,
    /// `$this->headersSent`.
    pub(crate) headers_sent: bool,
}

impl fmt::Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stream")
            .field("callback", &self.callback.as_ref().map(Arc::as_ptr))
            .field("streamed", &self.streamed)
            .field("headers_sent", &self.headers_sent)
            .finish()
    }
}

/// Two streams are equal when they hold the same closure (`==` on
/// objects compares their properties; closures by identity).
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        let same = match (&self.callback, &other.callback) {
            (None, None) => true,
            (Some(a), Some(b)) => std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b)),
            _ => false,
        };
        same && self.streamed == other.streamed && self.headers_sent == other.headers_sent
    }
}

impl Stream {
    /// `setChunks($chunks)`: a callback that echoes each chunk, then
    /// `@ob_flush()` and `flush()`.
    pub fn set_chunks(&mut self, chunks: Vec<PhpString>) -> &mut Self {
        self.callback = Some(Arc::new(move || {
            for chunk in &chunks {
                echo(chunk.clone());
                with_output(|o| o.calls.push(OutputCall::ObFlush));
                flush();
            }
            Ok(())
        }));
        self
    }

    /// `setCallback($callback)`.
    pub fn set_callback(&mut self, callback: Callback) -> &mut Self {
        self.callback = Some(callback);
        self
    }

    /// `getCallback()`: the callback, or `None` when none is set.
    pub fn get_callback(&self) -> Option<Callback> {
        self.callback.clone()
    }

    /// `sendContent()`'s part: the callback, once.
    pub(crate) fn send_content(&mut self) -> std::result::Result<(), Throwable> {
        if self.streamed {
            return Ok(());
        }

        self.streamed = true;

        let Some(callback) = &self.callback else {
            return Err(Arc::new(crate::exception::Error::new(
                crate::exception::Class::PhpLogic,
                "The Response callback must be set.",
            )));
        };

        callback()
    }
}

/// `new StreamedResponse($callbackOrChunks, $status, $headers)`.
///
/// Fails with `\InvalidArgumentException` when the status code is not
/// valid.
pub fn new(
    callback_or_chunks: CallbackOrChunks,
    status: i64,
    headers: ResponseHeaderBag,
) -> Result<Response> {
    let mut stream = Stream {
        callback: None,
        streamed: false,
        headers_sent: false,
    };
    match callback_or_chunks {
        CallbackOrChunks::Callback(callback) => {
            stream.set_callback(callback);
        }
        CallbackOrChunks::Chunks(chunks) if !chunks.is_empty() => {
            stream.set_chunks(chunks);
        }
        CallbackOrChunks::Chunks(_) | CallbackOrChunks::None => {}
    }
    Response::with_stream(status, headers, stream)
}

/// `echo $text`, for a callback.
pub fn echo(text: impl Into<PhpString>) {
    let text = text.into();
    with_output(|o| o.calls.push(OutputCall::Echo(text)));
}

/// `flush()`, for a callback.
pub fn flush() {
    with_output(|o| o.calls.push(OutputCall::Flush));
}
