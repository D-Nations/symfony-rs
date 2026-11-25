//! Port of Symfony\Component\EventDispatcher\EventDispatcher.php.
//!
//! The PHP state is kept as PHP holds it, because its order is observable:
//! - `$this->listeners[$eventName][$priority][]`: events in first-added
//!   order; priority buckets in insertion order until a sort `krsort()`s
//!   them in place, which is the order `getListenerPriority()` scans;
//! - `$this->sorted`: per-event results whose key order (first sorted,
//!   kept on re-sorting) is the event order of `getListeners()`;
//! - `$this->optimized`: per-event dispatch lists of the base class.
//!
//! A stored listener is shared with the lists built from it, like PHP's
//! by-reference loops, so resolving a lazy listener anywhere updates it
//! everywhere. No lock is held while a listener or a factory runs: both may
//! call back into the dispatcher, as in PHP. A dispatch calls the listeners
//! of the moment it started (checked in the oracle: a listener added during
//! a dispatch runs from the next one; one removed during it still runs).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher EventDispatcher.php

use std::{
    cmp::Reverse,
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use symfony_event_dispatcher_contracts::{
    EventDispatcherInterface as ContractsEventDispatcherInterface, EventObject, Throwable,
};

use crate::{
    interface::{BadMethodCallException, EventDispatcherInterface, EventSubscriber, Subscription},
    listener::{Listener, Object},
};

type Slot = Arc<Mutex<Listener>>;

struct EventListeners {
    name: String,
    /// Priority buckets in PHP array order.
    buckets: Vec<(i64, Vec<Slot>)>,
    /// Changes whenever PHP would unset `$this->sorted[$eventName]`, which
    /// is how `sortListeners()` notices a factory that added listeners.
    generation: u64,
}

#[derive(Default)]
struct State {
    listeners: Vec<EventListeners>,
    index: HashMap<String, usize>,
    sorted: HashMap<String, (u64, Vec<Listener>)>,
    next_sorted: u64,
    optimized: HashMap<String, Arc<[Slot]>>,
}

impl State {
    fn event(&self, name: &str) -> Option<&EventListeners> {
        self.index.get(name).map(|&i| &self.listeners[i])
    }

    fn event_mut(&mut self, name: &str) -> Option<&mut EventListeners> {
        self.index.get(name).map(|&i| &mut self.listeners[i])
    }

    /// `empty($this->listeners[$eventName])`.
    fn is_empty(&self, name: &str) -> bool {
        self.event(name)
            .is_none_or(|event| event.buckets.is_empty())
    }

    /// `krsort($this->listeners[$eventName])`, stable, in place; then the
    /// listeners in that order.
    fn krsort(&mut self, name: &str) -> Vec<Slot> {
        let Some(event) = self.event_mut(name) else {
            return Vec::new();
        };
        event
            .buckets
            .sort_by_key(|(priority, _)| Reverse(*priority));
        event
            .buckets
            .iter()
            .flat_map(|(_, slots)| slots.iter().cloned())
            .collect()
    }

    /// `unset($this->sorted[$eventName], $this->optimized[$eventName])`.
    fn invalidate(&mut self, name: &str) {
        self.sorted.remove(name);
        self.optimized.remove(name);
        if let Some(event) = self.event_mut(name) {
            event.generation += 1;
        }
    }
}

/// What a PHP subclass changes by overriding `callListeners()`.
///
/// A dispatcher built with one takes the base class's unoptimized path,
/// exactly as any PHP subclass does (`__CLASS__ !== static::class`): lazy
/// listeners are resolved when the listeners are sorted, not when first
/// called.
pub trait CallListeners: Send + Sync {
    fn call_listeners(
        &self,
        listeners: &[Listener],
        event_name: &str,
        event: &mut dyn EventObject,
        dispatcher: &EventDispatcher,
    ) -> Result<(), Throwable> {
        call_listeners(listeners, event_name, event, dispatcher)
    }
}

/// `EventDispatcher::callListeners()`: stops before the next listener once
/// propagation is stopped; a listener's exception ends the dispatch.
pub fn call_listeners(
    listeners: &[Listener],
    event_name: &str,
    event: &mut dyn EventObject,
    dispatcher: &dyn EventDispatcherInterface,
) -> Result<(), Throwable> {
    for listener in listeners {
        if event.is_propagation_stopped() {
            break;
        }
        listener.call(event, event_name, dispatcher)?;
    }
    Ok(())
}

/// The event dispatcher. Safe to share between threads; PHP's semantics
/// hold for each thread's own sequence of calls.
#[derive(Default)]
pub struct EventDispatcher {
    state: Mutex<State>,
    /// `None` is PHP's base class, which dispatches optimized lists.
    subclass: Option<Box<dyn CallListeners>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// The dispatcher as a PHP subclass that overrides `callListeners()`.
    pub fn with_call_listeners(call_listeners: impl CallListeners + 'static) -> Self {
        Self {
            state: Mutex::default(),
            subclass: Some(Box::new(call_listeners)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// The private `$listeners` table, which `TraceableEventDispatcher` reads
    /// by reflection: events in first-added order (events without
    /// listeners left out), priority buckets in their current order, lazy
    /// listeners as stored (unresolved unless something resolved them).
    pub fn get_listeners_with_priority(&self) -> Vec<(String, Vec<(Listener, i64)>)> {
        let slots: Vec<(String, Vec<(Slot, i64)>)> = {
            let state = self.lock();
            state
                .listeners
                .iter()
                .filter(|event| !event.buckets.is_empty())
                .map(|event| {
                    let slots = event
                        .buckets
                        .iter()
                        .flat_map(|(priority, slots)| slots.iter().map(|s| (s.clone(), *priority)))
                        .collect();
                    (event.name.clone(), slots)
                })
                .collect()
        };
        slots
            .into_iter()
            .map(|(name, slots)| {
                let listeners = slots
                    .into_iter()
                    .map(|(slot, priority)| (lock(&slot).clone(), priority))
                    .collect();
                (name, listeners)
            })
            .collect()
    }

    /// `sortListeners()`: resolve lazy listeners in priority order, and
    /// start again if that added or removed listeners of this event.
    fn sort_listeners(&self, name: &str) -> Vec<Listener> {
        loop {
            let (slots, generation) = {
                let mut state = self.lock();
                let slots = state.krsort(name);
                (slots, state.event(name).map(|event| event.generation))
            };
            let sorted: Vec<Listener> = slots.iter().map(resolve).collect();
            let mut state = self.lock();
            if state.event(name).map(|event| event.generation) != generation {
                continue;
            }
            let next = state.next_sorted;
            if let Some(entry) = state.sorted.get_mut(name) {
                // Assigning an existing PHP array key keeps its position.
                entry.1.clone_from(&sorted);
            } else {
                state.next_sorted += 1;
                state.sorted.insert(name.to_owned(), (next, sorted.clone()));
            }
            return sorted;
        }
    }

    /// `optimizeListeners()`: the dispatch list, lazy entries unresolved.
    fn optimized(&self, name: &str) -> Arc<[Slot]> {
        let mut state = self.lock();
        if let Some(slots) = state.optimized.get(name) {
            return slots.clone();
        }
        if state.is_empty(name) {
            return Arc::new([]);
        }
        let slots: Arc<[Slot]> = state.krsort(name).into();
        state.optimized.insert(name.to_owned(), slots.clone());
        slots
    }

    /// The scan shared by `getListenerPriority()` and `removeListener()`:
    /// each stored listener is resolved, unless it is the one looked for,
    /// until it matches. Yields the matching listeners and their priority.
    fn find(&self, name: &str, listener: &Listener, first_only: bool) -> Vec<(i64, Slot)> {
        let buckets: Vec<(i64, Vec<Slot>)> = {
            let state = self.lock();
            match state.event(name) {
                Some(event) if !event.buckets.is_empty() => event.buckets.clone(),
                _ => return Vec::new(),
            }
        };
        let listener = listener.resolve().unwrap_or_else(|| listener.clone());
        let mut found = Vec::new();
        for (priority, slots) in buckets {
            for slot in slots {
                let current = lock(&slot).clone();
                let value = if current.identical(&listener) {
                    current
                } else {
                    resolve(&slot)
                };
                if value.matches(&listener) {
                    found.push((priority, slot));
                    if first_only {
                        return found;
                    }
                }
            }
        }
        found
    }

    fn subscriptions(
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> impl Iterator<Item = (&'static str, Listener, i64)> {
        let object: Object = subscriber.clone();
        subscriber
            .subscribed_events()
            .into_iter()
            .flat_map(move |(name, subscription)| {
                let methods = match subscription {
                    Subscription::Method(method) => vec![(method, 0)],
                    Subscription::Prioritized(method, priority) => vec![(method, priority)],
                    Subscription::Methods(methods) => methods,
                };
                let object = object.clone();
                methods.into_iter().map(move |(method, priority)| {
                    (name, Listener::Method(object.clone(), method), priority)
                })
            })
    }
}

impl ContractsEventDispatcherInterface for EventDispatcher {
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable> {
        let class = event.class();
        let name = event_name.unwrap_or(class);
        if let Some(subclass) = &self.subclass {
            let listeners = self.get_listeners(name);
            if listeners.is_empty() {
                return Ok(());
            }
            return subclass.call_listeners(&listeners, name, event, self);
        }
        for slot in self.optimized(name).iter() {
            if event.is_propagation_stopped() {
                break;
            }
            resolve(slot).call(event, name, self)?;
        }
        Ok(())
    }
}

impl EventDispatcherInterface for EventDispatcher {
    fn add_listener(
        &self,
        event_name: &str,
        listener: Listener,
        priority: i64,
    ) -> Result<(), BadMethodCallException> {
        let mut state = self.lock();
        let index = if let Some(&index) = state.index.get(event_name) {
            index
        } else {
            let index = state.listeners.len();
            state.listeners.push(EventListeners {
                name: event_name.to_owned(),
                buckets: Vec::new(),
                generation: 0,
            });
            state.index.insert(event_name.to_owned(), index);
            index
        };
        let buckets = &mut state.listeners[index].buckets;
        let slot = Arc::new(Mutex::new(listener));
        match buckets.iter_mut().find(|(p, _)| *p == priority) {
            Some((_, slots)) => slots.push(slot),
            None => buckets.push((priority, vec![slot])),
        }
        state.invalidate(event_name);
        Ok(())
    }

    fn add_subscriber(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        for (name, listener, priority) in Self::subscriptions(&subscriber) {
            self.add_listener(name, listener, priority)?;
        }
        Ok(())
    }

    fn remove_listener(
        &self,
        event_name: &str,
        listener: &Listener,
    ) -> Result<(), BadMethodCallException> {
        let found = self.find(event_name, listener, false);
        if found.is_empty() {
            return Ok(());
        }
        let mut state = self.lock();
        if let Some(event) = state.event_mut(event_name) {
            for (_, slots) in &mut event.buckets {
                slots.retain(|slot| !found.iter().any(|(_, f)| Arc::ptr_eq(f, slot)));
            }
            event.buckets.retain(|(_, slots)| !slots.is_empty());
        }
        state.invalidate(event_name);
        Ok(())
    }

    fn remove_subscriber(
        &self,
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        for (name, listener, _) in Self::subscriptions(subscriber) {
            self.remove_listener(name, &listener)?;
        }
        Ok(())
    }

    fn get_listeners(&self, event_name: &str) -> Vec<Listener> {
        {
            let state = self.lock();
            if state.is_empty(event_name) {
                return Vec::new();
            }
            if let Some((_, sorted)) = state.sorted.get(event_name) {
                return sorted.clone();
            }
        }
        self.sort_listeners(event_name)
    }

    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        let unsorted: Vec<String> = {
            let state = self.lock();
            state
                .listeners
                .iter()
                .filter(|event| !state.sorted.contains_key(&event.name))
                .map(|event| event.name.clone())
                .collect()
        };
        for name in unsorted {
            if !self.lock().sorted.contains_key(&name) {
                self.sort_listeners(&name);
            }
        }
        let state = self.lock();
        let mut all: Vec<(u64, String, Vec<Listener>)> = state
            .sorted
            .iter()
            .filter(|(_, (_, listeners))| !listeners.is_empty())
            .map(|(name, (order, listeners))| (*order, name.clone(), listeners.clone()))
            .collect();
        all.sort_by_key(|(order, _, _)| *order);
        all.into_iter()
            .map(|(_, name, listeners)| (name, listeners))
            .collect()
    }

    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64> {
        self.find(event_name, listener, true)
            .first()
            .map(|(priority, _)| *priority)
    }

    fn as_event_dispatcher(&self) -> Option<&EventDispatcher> {
        Some(self)
    }

    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        let state = self.lock();
        match event_name {
            Some(name) => !state.is_empty(name),
            None => state
                .listeners
                .iter()
                .any(|event| !event.buckets.is_empty()),
        }
    }
}

/// `$listener[0] = $listener[0](); $listener[1] ??= '__invoke';` through
/// the reference PHP holds. The factory runs with no lock held.
fn resolve(slot: &Slot) -> Listener {
    let current = lock(slot).clone();
    let Some(resolved) = current.resolve() else {
        return current;
    };
    let mut stored = lock(slot);
    if stored.identical(&current) {
        *stored = resolved.clone();
        resolved
    } else {
        // Another thread resolved it first; keep one object, as PHP does.
        stored.clone()
    }
}

/// No code panics while holding these locks, so poisoning carries no
/// broken invariant; recover the guard rather than fail every later call.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
