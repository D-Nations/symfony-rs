//! Port of the symfony/event-dispatcher component: EventDispatcher,
//! GenericEvent, ImmutableEventDispatcher, the two interfaces and, with
//! the `debug` feature, the Debug namespace. See README.md for the
//! contract and what is not ported yet.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

#[cfg(feature = "debug")]
pub mod debug;
mod event_dispatcher;
mod generic_event;
mod immutable_event_dispatcher;
mod interface;
mod listener;

pub use event_dispatcher::{CallListeners, EventDispatcher, call_listeners};
pub use generic_event::{GenericEvent, InvalidArgumentException};
pub use immutable_event_dispatcher::ImmutableEventDispatcher;
pub use interface::{
    BadMethodCallException, EventDispatcherInterface, EventSubscriber, Subscription,
};
pub use listener::{Callable, Closure, Factory, Listener, Object, UndefinedMethod};
pub use symfony_event_dispatcher_contracts::{
    Event, EventDispatcherInterface as ContractsEventDispatcherInterface, EventObject, Throwable,
};
