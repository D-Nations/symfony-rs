//! Port of Symfony\Component\EventDispatcher\ImmutableEventDispatcher.php:
//! a read-only proxy for an event dispatcher.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher ImmutableEventDispatcher.php

use std::sync::Arc;

use symfony_event_dispatcher_contracts::{
    EventDispatcherInterface as ContractsEventDispatcherInterface, EventObject, Throwable,
};

use crate::{
    interface::{BadMethodCallException, EventDispatcherInterface, EventSubscriber},
    listener::Listener,
};

const UNMODIFIABLE: BadMethodCallException = BadMethodCallException {
    message: "Unmodifiable event dispatchers must not be modified.",
};

pub struct ImmutableEventDispatcher<D> {
    dispatcher: D,
}

impl<D: EventDispatcherInterface> ImmutableEventDispatcher<D> {
    pub fn new(dispatcher: D) -> Self {
        Self { dispatcher }
    }
}

impl<D: EventDispatcherInterface> ContractsEventDispatcherInterface
    for ImmutableEventDispatcher<D>
{
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable> {
        self.dispatcher.dispatch(event, event_name)
    }
}

impl<D: EventDispatcherInterface> EventDispatcherInterface for ImmutableEventDispatcher<D> {
    fn add_listener(&self, _: &str, _: Listener, _: i64) -> Result<(), BadMethodCallException> {
        Err(UNMODIFIABLE)
    }

    fn add_subscriber(&self, _: Arc<dyn EventSubscriber>) -> Result<(), BadMethodCallException> {
        Err(UNMODIFIABLE)
    }

    fn remove_listener(&self, _: &str, _: &Listener) -> Result<(), BadMethodCallException> {
        Err(UNMODIFIABLE)
    }

    fn remove_subscriber(
        &self,
        _: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        Err(UNMODIFIABLE)
    }

    fn get_listeners(&self, event_name: &str) -> Vec<Listener> {
        self.dispatcher.get_listeners(event_name)
    }

    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        self.dispatcher.get_all_listeners()
    }

    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64> {
        self.dispatcher.get_listener_priority(event_name, listener)
    }

    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        self.dispatcher.has_listeners(event_name)
    }
}
