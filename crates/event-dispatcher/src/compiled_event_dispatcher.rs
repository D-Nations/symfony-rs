//! A dispatcher whose listeners are settled before the first request: the
//! listener execution seam of the PLAN (M2-A). It behaves as the pinned
//! EventDispatcher.php given the same `addListener()` calls, and
//! dispatches without a lock, a sort or a copy of the listener list.
//!
//! The design is upstream's own, from a later release than the one pinned
//! (symfony/symfony#66122, `CompiledEventDispatcher`, Symfony 8.2): the
//! listeners are data, sorted once, and anything but dispatching
//! "thaws" them into a plain [`EventDispatcher`] that every later call
//! goes through. Unlike 8.2, nothing is deprecated, and the thaw rebuilds
//! the state the pinned dispatcher would have reached: the same
//! `addListener()` calls in the same order, lazy listeners already called
//! replaced by their object, and the priority buckets of every event
//! dispatched so far `krsort()`ed, as `optimizeListeners()` leaves them.
//! So `getListeners()`, `getListenerPriority()`, the subscriber methods,
//! `removeListener()` and the table `TraceableEventDispatcher` reads give
//! what the pinned dispatcher gives, and a listener added at run time runs
//! from the next dispatch on, as there.
//!
//! [`ListenerPipeline`] is the seam: [`ListenerMap`] runs the sorted
//! table; code generated for a closed set of listeners can run them by
//! direct calls instead, keeping a [`ListenerMap`] for the thaw.
//!
//! The thawed dispatcher shares each stored listener with the compiled
//! table, as PHP's by-reference lists do, so a lazy listener resolved by a
//! dispatch that began before the thaw is resolved in the thawed table too.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

use std::{
    cmp::Reverse,
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use symfony_event_dispatcher_contracts::{
    EventDispatcherInterface as ContractsEventDispatcherInterface, EventObject, Throwable,
};

use crate::{
    event_dispatcher::{EventDispatcher, Slot, resolve},
    interface::{BadMethodCallException, EventDispatcherInterface, EventSubscriber},
    listener::Listener,
};

/// How a compiled dispatcher runs listeners until it thaws.
pub trait ListenerPipeline: Send + Sync {
    /// `hasListeners($eventName)`: whether the event (or, without a name,
    /// any event) has a listener.
    fn has_listeners(&self, event_name: Option<&str>) -> bool;

    /// Calls the listeners of `event_name` in the order the pinned
    /// dispatcher calls them, with `dispatcher` as their third argument,
    /// stopping before the next one once propagation is stopped. A
    /// listener's throwable ends the dispatch.
    fn dispatch(
        &self,
        event_name: &str,
        event: &mut dyn EventObject,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable>;

    /// The plain dispatcher the same `addListener()` calls, followed by the
    /// dispatches made so far, would have built.
    fn thaw(&self) -> EventDispatcher;
}

struct CompiledEvent {
    name: String,
    /// In `addListener()` order, with their priority. A slot is shared with
    /// the thawed dispatcher, so a lazy listener resolved by either is
    /// resolved in both, as by PHP's references.
    registered: Vec<(Slot, i64)>,
    /// In the order they run: by descending priority, then as registered.
    run: Vec<Slot>,
    /// Whether a dispatch has `krsort()`ed the pinned table's buckets.
    dispatched: AtomicBool,
}

/// The listener table, built by the same calls as a plain dispatcher's,
/// before the dispatcher is shared.
#[derive(Default)]
pub struct ListenerMap {
    /// Events in first-added order.
    events: Vec<CompiledEvent>,
    index: HashMap<String, usize>,
}

impl ListenerMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// `addListener($eventName, $listener, $priority)`.
    pub fn add_listener(&mut self, event_name: &str, listener: Listener, priority: i64) {
        let index = if let Some(&index) = self.index.get(event_name) {
            index
        } else {
            self.events.push(CompiledEvent {
                name: event_name.to_owned(),
                registered: Vec::new(),
                run: Vec::new(),
                dispatched: AtomicBool::new(false),
            });
            self.index
                .insert(event_name.to_owned(), self.events.len() - 1);
            self.events.len() - 1
        };
        let event = &mut self.events[index];
        event
            .registered
            .push((Arc::new(Mutex::new(listener)), priority));
        // Stable: equal priorities keep their registration order, as the
        // buckets do.
        let mut run = event.registered.clone();
        run.sort_by_key(|(_, priority)| Reverse(*priority));
        event.run = run.into_iter().map(|(slot, _)| slot).collect();
    }

    /// `addSubscriber($subscriber)`.
    pub fn add_subscriber(&mut self, subscriber: &Arc<dyn EventSubscriber>) {
        for (name, listener, priority) in EventDispatcher::subscriptions(subscriber) {
            self.add_listener(name, listener, priority);
        }
    }

    /// The listeners of `event_name` in the order they run, after noting
    /// the dispatch for the thaw. Empty when the event has none.
    fn begin_dispatch(&self, event_name: &str) -> &[Slot] {
        let Some(event) = self.index.get(event_name).map(|&i| &self.events[i]) else {
            return &[];
        };
        event.dispatched.store(true, Ordering::Relaxed);
        &event.run
    }

    /// Marks `event_name` as dispatched, for a pipeline that calls its
    /// listeners itself; returns whether the event has listeners.
    pub fn note_dispatch(&self, event_name: &str) -> bool {
        !self.begin_dispatch(event_name).is_empty()
    }
}

impl ListenerPipeline for ListenerMap {
    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        match event_name {
            Some(name) => self.index.contains_key(name),
            None => !self.events.is_empty(),
        }
    }

    fn dispatch(
        &self,
        event_name: &str,
        event: &mut dyn EventObject,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        for slot in self.begin_dispatch(event_name) {
            if event.is_propagation_stopped() {
                break;
            }
            resolve(slot).call(event, event_name, dispatcher)?;
        }
        Ok(())
    }

    fn thaw(&self) -> EventDispatcher {
        let dispatcher = EventDispatcher::new();
        for event in &self.events {
            for (slot, priority) in &event.registered {
                dispatcher.add_slot(&event.name, slot.clone(), *priority);
            }
            if event.dispatched.load(Ordering::Relaxed) {
                dispatcher.optimize(&event.name);
            }
        }
        dispatcher
    }
}

/// The compiled dispatcher: `pipeline` until something but a dispatch or
/// `hasListeners()` reaches it, then the plain dispatcher it thaws into.
pub struct CompiledEventDispatcher<P: ListenerPipeline = ListenerMap> {
    pipeline: P,
    thawed: OnceLock<EventDispatcher>,
}

impl<P: ListenerPipeline> CompiledEventDispatcher<P> {
    pub fn new(pipeline: P) -> Self {
        Self {
            pipeline,
            thawed: OnceLock::new(),
        }
    }

    /// Whether the listeners have moved to a plain dispatcher.
    pub fn is_thawed(&self) -> bool {
        self.thawed.get().is_some()
    }

    fn thawed(&self) -> &EventDispatcher {
        self.thawed.get_or_init(|| self.pipeline.thaw())
    }
}

impl From<ListenerMap> for CompiledEventDispatcher {
    fn from(map: ListenerMap) -> Self {
        Self::new(map)
    }
}

impl<P: ListenerPipeline> ContractsEventDispatcherInterface for CompiledEventDispatcher<P> {
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable> {
        if let Some(thawed) = self.thawed.get() {
            return thawed.dispatch(event, event_name);
        }
        let class = event.class();
        let name = event_name.unwrap_or(class);
        self.pipeline.dispatch(name, event, self)
    }
}

impl<P: ListenerPipeline> EventDispatcherInterface for CompiledEventDispatcher<P> {
    fn add_listener(
        &self,
        event_name: &str,
        listener: Listener,
        priority: i64,
    ) -> Result<(), BadMethodCallException> {
        self.thawed().add_listener(event_name, listener, priority)
    }

    fn add_subscriber(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        self.thawed().add_subscriber(subscriber)
    }

    fn remove_listener(
        &self,
        event_name: &str,
        listener: &Listener,
    ) -> Result<(), BadMethodCallException> {
        self.thawed().remove_listener(event_name, listener)
    }

    fn remove_subscriber(
        &self,
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        self.thawed().remove_subscriber(subscriber)
    }

    fn get_listeners(&self, event_name: &str) -> Vec<Listener> {
        self.thawed().get_listeners(event_name)
    }

    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        self.thawed().get_all_listeners()
    }

    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64> {
        self.thawed().get_listener_priority(event_name, listener)
    }

    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        match self.thawed.get() {
            Some(thawed) => thawed.has_listeners(event_name),
            None => self.pipeline.has_listeners(event_name),
        }
    }

    /// The pinned `TraceableEventDispatcher` reads the table of an
    /// `EventDispatcher`, which this one stands in for: it thaws.
    fn as_event_dispatcher(&self) -> Option<&EventDispatcher> {
        Some(self.thawed())
    }
}
