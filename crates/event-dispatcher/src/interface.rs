//! Port of Symfony\Component\EventDispatcher\EventDispatcherInterface.php
//! and EventSubscriberInterface.php.
//!
//! PHP's `getListeners(?string $eventName)` returns two shapes; it is split
//! into `get_listeners(name)` and `get_all_listeners()`. Mutators return
//! `Result` because ImmutableEventDispatcher.php throws from them.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher EventDispatcherInterface.php EventSubscriberInterface.php

use std::{fmt, sync::Arc};

use symfony_event_dispatcher_contracts::EventDispatcherInterface as ContractsEventDispatcherInterface;

use crate::{
    event_dispatcher::EventDispatcher,
    listener::{Callable, Listener},
};

/// PHP's `\BadMethodCallException`, thrown by an unmodifiable dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadMethodCallException {
    pub message: &'static str,
}

impl fmt::Display for BadMethodCallException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for BadMethodCallException {}

/// The central point of the event listener system: listeners are
/// registered on the dispatcher and events are dispatched through it.
pub trait EventDispatcherInterface: ContractsEventDispatcherInterface {
    /// Adds an event listener. The higher the priority, the earlier the
    /// listener is triggered (0 by default in PHP).
    fn add_listener(
        &self,
        event_name: &str,
        listener: Listener,
        priority: i64,
    ) -> Result<(), BadMethodCallException>;

    /// Adds a listener for each event the subscriber names.
    fn add_subscriber(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException>;

    fn remove_listener(
        &self,
        event_name: &str,
        listener: &Listener,
    ) -> Result<(), BadMethodCallException>;

    fn remove_subscriber(
        &self,
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException>;

    /// The listeners of one event, by descending priority.
    fn get_listeners(&self, event_name: &str) -> Vec<Listener>;

    /// `getListeners()` without a name: every event that has listeners,
    /// each sorted by descending priority.
    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)>;

    /// The priority of a listener, or `None` if the event or the listener
    /// does not exist.
    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64>;

    /// Whether an event (or, without a name, any event) has listeners.
    fn has_listeners(&self, event_name: Option<&str>) -> bool;

    /// PHP's `$dispatcher instanceof EventDispatcher`, which
    /// `TraceableEventDispatcher` checks before reading the listener table.
    fn as_event_dispatcher(&self) -> Option<&EventDispatcher> {
        None
    }
}

/// A shared dispatcher behaves as the dispatcher it holds.
impl<T: EventDispatcherInterface + ?Sized> EventDispatcherInterface for Arc<T> {
    fn add_listener(
        &self,
        event_name: &str,
        listener: Listener,
        priority: i64,
    ) -> Result<(), BadMethodCallException> {
        (**self).add_listener(event_name, listener, priority)
    }

    fn add_subscriber(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        (**self).add_subscriber(subscriber)
    }

    fn remove_listener(
        &self,
        event_name: &str,
        listener: &Listener,
    ) -> Result<(), BadMethodCallException> {
        (**self).remove_listener(event_name, listener)
    }

    fn remove_subscriber(
        &self,
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        (**self).remove_subscriber(subscriber)
    }

    fn get_listeners(&self, event_name: &str) -> Vec<Listener> {
        (**self).get_listeners(event_name)
    }

    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        (**self).get_all_listeners()
    }

    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64> {
        (**self).get_listener_priority(event_name, listener)
    }

    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        (**self).has_listeners(event_name)
    }

    fn as_event_dispatcher(&self) -> Option<&EventDispatcher> {
        (**self).as_event_dispatcher()
    }
}

/// One value of `getSubscribedEvents()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subscription {
    /// `'methodName'`; priority 0.
    Method(&'static str),
    /// `['methodName', $priority]`; `['methodName']` has priority 0.
    Prioritized(&'static str, i64),
    /// `[['methodName1', $priority], ['methodName2']]`.
    Methods(Vec<(&'static str, i64)>),
}

/// An object that names the events it listens to.
///
/// PHP's `getSubscribedEvents()` is static and is read at compile time; it
/// must not depend on run-time state. Entries keep their order, as a PHP
/// array does.
pub trait EventSubscriber: Callable {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)>;
}
