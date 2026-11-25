//! The Debug namespace (feature `debug`): `TraceableEventDispatcher` and
//! `WrappedListener`, which need Stopwatch and a PSR-3 logger.
//!
//! Upstream: none

mod traceable_event_dispatcher;
mod wrapped_listener;

pub use traceable_event_dispatcher::{
    DispatchHooks, Options, RequestId, RequestStack, TraceableEventDispatcher,
};
pub use wrapped_listener::{ListenerInfo, WrappedListener, as_wrapped};
