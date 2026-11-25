//! Port of Symfony\Contracts\EventDispatcher\Event.php and
//! EventDispatcherInterface.php, with PSR-14's `StoppableEventInterface`
//! folded into [`EventObject`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher-contracts Event.php EventDispatcherInterface.php

#![forbid(unsafe_code)]

use std::any::Any;

/// What PHP throws out of a listener. `dispatch()` stops at the first one
/// and hands it to the caller unchanged, as an uncaught PHP exception would.
pub type Throwable = Box<dyn std::error::Error + Send + Sync>;

/// A dispatched event: PHP's `object $event`.
///
/// `class()` is the PHP class name (`$event::class`), which `dispatch()`
/// uses as the event name when none is given. Define it once per type.
///
/// `is_propagation_stopped()` is PSR-14's `StoppableEventInterface`. An
/// event that is not stoppable keeps the default `false`, which dispatches
/// exactly as PHP does for an object without that interface.
pub trait EventObject: Any {
    fn class(&self) -> &'static str;

    fn is_propagation_stopped(&self) -> bool {
        false
    }
}

impl dyn EventObject {
    pub fn downcast_ref<T: EventObject>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }

    pub fn downcast_mut<T: EventObject>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}

/// `Symfony\Contracts\EventDispatcher\Event`: the base event with no data.
///
/// PHP events extend this class; a Rust event holds an `Event` field and
/// forwards `is_propagation_stopped()` to it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Event {
    propagation_stopped: bool,
}

impl Event {
    pub const CLASS: &'static str = "Symfony\\Contracts\\EventDispatcher\\Event";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    /// Stops the propagation of the event to further event listeners.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }
}

impl EventObject for Event {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }
}

/// `Symfony\Contracts\EventDispatcher\EventDispatcherInterface`.
///
/// PHP returns the passed event; here the caller keeps ownership and the
/// listeners change it in place. A listener's exception ends the dispatch
/// and is returned.
pub trait EventDispatcherInterface: Send + Sync {
    /// Dispatches an event to all registered listeners. Without a name,
    /// the event's class is used.
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable>;
}

/// A shared dispatcher dispatches as the dispatcher it holds.
impl<T: EventDispatcherInterface + ?Sized> EventDispatcherInterface for std::sync::Arc<T> {
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable> {
        (**self).dispatch(event, event_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_propagation_is_kept() {
        let mut event = Event::new();
        assert!(!event.is_propagation_stopped());
        event.stop_propagation();
        assert!(EventObject::is_propagation_stopped(&event));
    }

    #[test]
    fn downcast_returns_the_concrete_event() {
        let mut event = Event::new();
        let object: &mut dyn EventObject = &mut event;
        assert_eq!(object.class(), "Symfony\\Contracts\\EventDispatcher\\Event");
        assert!(object.downcast_mut::<Event>().is_some());
    }
}
