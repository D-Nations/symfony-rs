//! Port of Symfony\Component\EventDispatcher\Debug\TraceableEventDispatcher.php:
//! collects data about event listeners while delegating to a dispatcher.
//!
//! During a dispatch every listener of the event is swapped in the inner
//! dispatcher for a `WrappedListener` and swapped back afterwards, as in
//! PHP. The dispatcher is created in an `Arc` because the wrappers refer
//! back to it (weakly) and pass it to the listeners they call.
//!
//! - `RequestStack` stands in for HttpFoundation's RequestStack, which is
//!   not ported yet: it only needs the current request's identity (PHP's
//!   `spl_object_id($request)`).
//! - `DispatchHooks` is what a PHP subclass overrides (`beforeDispatch()`,
//!   `afterDispatch()`).
//! - `getNotCalledListeners()` reads the inner `EventDispatcher`'s listener
//!   table by reflection in PHP; here through
//!   `EventDispatcher::get_listeners_with_priority()`.
//! - `__call()` proxying is not ported: Rust callers use the inner
//!   dispatcher directly.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher Debug/TraceableEventDispatcher.php

use std::{
    cmp::Ordering,
    collections::HashMap,
    num::NonZeroU64,
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

use psr_log::{Context, LoggerInterface, context};
use symfony_event_dispatcher_contracts::{
    EventDispatcherInterface as ContractsEventDispatcherInterface, EventObject, Throwable,
};
use symfony_stopwatch::Stopwatch;

use super::wrapped_listener::{ListenerInfo, WrappedListener, as_wrapped, wrapped};
use crate::{
    interface::{BadMethodCallException, EventDispatcherInterface, EventSubscriber},
    listener::Listener,
};

/// A request's identity, as PHP's `spl_object_id()` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub NonZeroU64);

/// `RequestStack::getCurrentRequest()`, reduced to identity.
pub trait RequestStack: Send + Sync {
    fn current_request(&self) -> Option<RequestId>;
}

/// The protected hooks a PHP subclass overrides.
pub trait DispatchHooks: Send + Sync {
    /// Called before dispatching the event. An error skips the dispatch
    /// and `after_dispatch`.
    fn before_dispatch(
        &self,
        _event_name: &str,
        _event: &mut dyn EventObject,
    ) -> Result<(), Throwable> {
        Ok(())
    }

    /// Called after dispatching the event, even when a listener failed;
    /// its own error replaces the listener's, as a PHP `finally` does.
    fn after_dispatch(
        &self,
        _event_name: &str,
        _event: &mut dyn EventObject,
    ) -> Result<(), Throwable> {
        Ok(())
    }
}

struct NoHooks;
impl DispatchHooks for NoHooks {}

/// The optional constructor arguments.
#[derive(Default)]
pub struct Options {
    pub logger: Option<Arc<dyn LoggerInterface>>,
    pub request_stack: Option<Arc<dyn RequestStack>>,
    /// While it returns true, events are dispatched without tracing.
    pub disabled: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    pub hooks: Option<Box<dyn DispatchHooks>>,
}

/// `0` is PHP's "no current request".
type RequestKey = u64;

/// Called listeners per event, in first-called order.
type CalledByEvent = Vec<(String, Vec<Called>)>;
/// Per request, in first-seen order.
type CalledByRequest = Vec<(RequestKey, CalledByEvent)>;
/// Each event's listeners with their priority, when known.
type ListenerTable = Vec<(String, Vec<(Listener, Option<i64>)>)>;

struct Called {
    original: Listener,
    info: ListenerInfo,
    count: usize,
}

#[derive(Default)]
struct State {
    call_stack: Option<Vec<(Arc<WrappedListener>, String, RequestKey)>>,
    wrapped_listeners: HashMap<String, Vec<Arc<WrappedListener>>>,
    orphaned_events: Vec<(RequestKey, Vec<String>)>,
    dispatch_depth: HashMap<String, i64>,
    /// `calledListenerInfos` and `calledOriginalListeners`, kept together.
    called: CalledByRequest,
    current_request_hash: RequestKey,
}

pub struct TraceableEventDispatcher<D> {
    dispatcher: D,
    stopwatch: Arc<Stopwatch>,
    logger: Option<Arc<dyn LoggerInterface>>,
    request_stack: Option<Arc<dyn RequestStack>>,
    disabled: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    hooks: Box<dyn DispatchHooks>,
    this: Weak<Self>,
    state: Mutex<State>,
}

impl<D: EventDispatcherInterface + 'static> TraceableEventDispatcher<D> {
    pub fn new(dispatcher: D, stopwatch: Arc<Stopwatch>) -> Arc<Self> {
        Self::with_options(dispatcher, stopwatch, Options::default())
    }

    pub fn with_options(dispatcher: D, stopwatch: Arc<Stopwatch>, options: Options) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            dispatcher,
            stopwatch,
            logger: options.logger,
            request_stack: options.request_stack,
            disabled: options.disabled,
            hooks: options.hooks.unwrap_or_else(|| Box::new(NoHooks)),
            this: this.clone(),
            state: Mutex::default(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Nothing panics while this lock is held.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn this(&self) -> Weak<dyn EventDispatcherInterface> {
        self.this.clone()
    }

    /// The wrapper standing in for `listener` during a dispatch, if any.
    fn wrapper_of(
        &self,
        event_name: &str,
        listener: &Listener,
        remove: bool,
    ) -> Option<Arc<WrappedListener>> {
        let mut state = self.lock();
        let wrappers = state.wrapped_listeners.get_mut(event_name)?;
        let index = wrappers
            .iter()
            .position(|w| w.get_wrapped_listener().matches(listener))?;
        Some(if remove {
            wrappers.remove(index)
        } else {
            wrappers[index].clone()
        })
    }

    /// The listeners called so far, one entry per call; for one request,
    /// or for all.
    pub fn get_called_listeners(&self, request: Option<RequestId>) -> Vec<ListenerInfo> {
        let hash = request.map(|r| r.0.get());
        let state = self.lock();
        let mut called = Vec::new();
        for (request_hash, events) in &state.called {
            if hash.is_some_and(|h| h != *request_hash) {
                continue;
            }
            for (_, listeners) in events {
                for entry in listeners {
                    called.extend(std::iter::repeat_n(entry.info.clone(), entry.count));
                }
            }
        }
        called
    }

    /// The registered listeners never called, sorted by event name, then
    /// by descending priority (unknown priorities first).
    pub fn get_not_called_listeners(&self, request: Option<RequestId>) -> Vec<ListenerInfo> {
        let all: ListenerTable = match self.dispatcher.as_event_dispatcher() {
            Some(dispatcher) => dispatcher
                .get_listeners_with_priority()
                .into_iter()
                .map(|(name, listeners)| {
                    (
                        name,
                        listeners.into_iter().map(|(l, p)| (l, Some(p))).collect(),
                    )
                })
                .collect(),
            None => self
                .get_all_listeners()
                .into_iter()
                .map(|(name, listeners)| (name, listeners.into_iter().map(|l| (l, None)).collect()))
                .collect(),
        };

        let hash = request.map(|r| r.0.get());
        let called: Vec<Listener> = {
            let state = self.lock();
            state
                .called
                .iter()
                .filter(|(request_hash, _)| hash.is_none_or(|h| h == *request_hash))
                .flat_map(|(_, events)| events.iter())
                .flat_map(|(_, listeners)| listeners.iter().map(|c| c.original.clone()))
                .collect()
        };

        let mut not_called = Vec::new();
        for (event_name, listeners) in all {
            for (listener, priority) in listeners {
                if called.iter().any(|c| c.identical(&listener)) {
                    continue;
                }
                let info = match as_wrapped(&listener) {
                    Some(wrapper) => wrapper.get_info(&event_name),
                    None => WrappedListener::new(
                        listener,
                        None,
                        self.stopwatch.clone(),
                        Some(self.this()),
                        priority,
                    )
                    .get_info(&event_name),
                };
                not_called.push(info);
            }
        }
        not_called.sort_by(sort_not_called_listeners);
        not_called
    }

    /// Events dispatched without listeners; for one request, or for all.
    pub fn get_orphaned_events(&self, request: Option<RequestId>) -> Vec<String> {
        let state = self.lock();
        match request {
            Some(request) => state
                .orphaned_events
                .iter()
                .find(|(hash, _)| *hash == request.0.get())
                .map(|(_, events)| events.clone())
                .unwrap_or_default(),
            None => state
                .orphaned_events
                .iter()
                .flat_map(|(_, events)| events.iter().cloned())
                .collect(),
        }
    }

    pub fn reset(&self) {
        *self.lock() = State::default();
    }

    fn debug(&self, message: &str, context: Context) {
        if let Some(logger) = &self.logger {
            logger.debug(message, context);
        }
    }

    fn pre_process(&self, event_name: &str) {
        let current = {
            let mut state = self.lock();
            *state
                .dispatch_depth
                .entry(event_name.to_owned())
                .or_insert(0) += 1;
            state.current_request_hash
        };

        if !self.dispatcher.has_listeners(Some(event_name)) {
            let mut state = self.lock();
            match state
                .orphaned_events
                .iter_mut()
                .find(|(hash, _)| *hash == current)
            {
                Some((_, events)) => events.push(event_name.to_owned()),
                None => state
                    .orphaned_events
                    .push((current, vec![event_name.to_owned()])),
            }
            return;
        }

        for listener in self.dispatcher.get_listeners(event_name) {
            let priority = self
                .get_listener_priority(event_name, &listener)
                .unwrap_or(0);
            let original = match as_wrapped(&listener) {
                Some(wrapper) => wrapper.get_wrapped_listener().clone(),
                None => listener.clone(),
            };
            let wrapper = Arc::new(WrappedListener::new(
                original,
                None,
                self.stopwatch.clone(),
                Some(self.this()),
                None,
            ));
            self.lock()
                .wrapped_listeners
                .entry(event_name.to_owned())
                .or_default()
                .push(wrapper.clone());
            // The inner dispatcher is this dispatcher's own; these succeed
            // unless it refuses changes, and then nothing is traced.
            let _ = self.dispatcher.remove_listener(event_name, &listener);
            let _ = self
                .dispatcher
                .add_listener(event_name, wrapped(&wrapper), priority);
            let mut state = self.lock();
            let current = state.current_request_hash;
            state.call_stack.get_or_insert_with(Vec::new).push((
                wrapper,
                event_name.to_owned(),
                current,
            ));
        }
    }

    fn post_process(&self, event_name: &str) {
        let depth = {
            let mut state = self.lock();
            if state.call_stack.is_none() {
                return;
            }
            let depth = state.dispatch_depth.get(event_name).copied().unwrap_or(1) - 1;
            state.dispatch_depth.insert(event_name.to_owned(), depth);
            state.wrapped_listeners.remove(event_name);
            depth
        };

        let mut skipped = false;
        for listener in self.dispatcher.get_listeners(event_name) {
            // #12845: a new listener was added during dispatch.
            let Some(wrapper) = as_wrapped(&listener) else {
                continue;
            };
            // Unwrap listener
            let priority = self
                .get_listener_priority(event_name, &listener)
                .unwrap_or(0);
            let _ = self.dispatcher.remove_listener(event_name, &listener);
            let _ = self.dispatcher.add_listener(
                event_name,
                wrapper.get_wrapped_listener().clone(),
                priority,
            );

            let context = || context([("event", event_name), ("listener", wrapper.get_pretty())]);

            if wrapper.was_called() {
                self.debug(
                    "Notified event \"{event}\" to listener \"{listener}\".",
                    context(),
                );
                self.collect_called_listener(&wrapper, event_name);
            }

            if let Some(stack) = &mut self.lock().call_stack {
                stack.retain(|(w, _, _)| !Arc::ptr_eq(w, &wrapper));
            }

            if skipped {
                self.debug(
                    "Listener \"{listener}\" was not called for event \"{event}\".",
                    context(),
                );
            }

            if wrapper.stopped_propagation() {
                self.debug(
                    "Listener \"{listener}\" stopped propagation of the event \"{event}\".",
                    context(),
                );
                skipped = true;
            }
        }

        if depth > 0 {
            return;
        }

        // Clean up stale callStack entries left by nested same-event dispatches
        let stale: Vec<Arc<WrappedListener>> = {
            let mut state = self.lock();
            let Some(stack) = &mut state.call_stack else {
                return;
            };
            let (stale, kept) = std::mem::take(stack)
                .into_iter()
                .partition(|(_, name, _)| name == event_name);
            *stack = kept;
            stale.into_iter().map(|(w, _, _)| w).collect()
        };
        for wrapper in stale {
            if wrapper.was_called() {
                self.collect_called_listener(&wrapper, event_name);
            }
        }
    }

    /// Counts an invocation without storing per-invocation state, so that
    /// the aggregate cannot grow when the same listeners are called
    /// repeatedly.
    fn collect_called_listener(&self, wrapper: &WrappedListener, event_name: &str) {
        let original = wrapper.get_wrapped_listener();
        {
            let mut state = self.lock();
            let current = state.current_request_hash;
            if let Some(entry) = find_called(&mut state.called, current, event_name, original) {
                entry.count += 1;
                return;
            }
        }
        // getInfo() may ask this dispatcher for a priority: no lock held.
        let info = wrapper.get_info(event_name);
        let mut state = self.lock();
        let current = state.current_request_hash;
        if let Some(entry) = find_called(&mut state.called, current, event_name, original) {
            entry.count += 1;
            return;
        }
        let events = match state.called.iter().position(|(hash, _)| *hash == current) {
            Some(i) => &mut state.called[i].1,
            None => {
                state.called.push((current, Vec::new()));
                let last = state.called.len() - 1;
                &mut state.called[last].1
            }
        };
        let listeners = match events.iter().position(|(name, _)| name == event_name) {
            Some(i) => &mut events[i].1,
            None => {
                events.push((event_name.to_owned(), Vec::new()));
                let last = events.len() - 1;
                &mut events[last].1
            }
        };
        listeners.push(Called {
            original: original.clone(),
            info,
            count: 1,
        });
    }

    #[cfg(test)]
    pub(crate) fn state_sizes(&self) -> (usize, Vec<(RequestKey, String, usize)>) {
        let state = self.lock();
        let wrapped = state
            .wrapped_listeners
            .values()
            .map(Vec::len)
            .sum::<usize>()
            + state.wrapped_listeners.len();
        let called = state
            .called
            .iter()
            .flat_map(|(hash, events)| {
                events
                    .iter()
                    .map(move |(name, listeners)| (*hash, name.clone(), listeners.len()))
            })
            .collect();
        (wrapped, called)
    }
}

fn find_called<'a>(
    called: &'a mut [(RequestKey, CalledByEvent)],
    hash: RequestKey,
    event_name: &str,
    original: &Listener,
) -> Option<&'a mut Called> {
    called
        .iter_mut()
        .find(|(h, _)| *h == hash)?
        .1
        .iter_mut()
        .find(|(name, _)| name == event_name)?
        .1
        .iter_mut()
        .find(|c| c.original.identical(original))
}

/// `sortNotCalledListeners()`.
fn sort_not_called_listeners(a: &ListenerInfo, b: &ListenerInfo) -> Ordering {
    a.event
        .as_bytes()
        .cmp(b.event.as_bytes())
        .then_with(|| match (a.priority, b.priority) {
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (a, b) => b.cmp(&a),
        })
}

impl<D: EventDispatcherInterface + 'static> ContractsEventDispatcherInterface
    for TraceableEventDispatcher<D>
{
    fn dispatch(
        &self,
        event: &mut dyn EventObject,
        event_name: Option<&str>,
    ) -> Result<(), Throwable> {
        if self.disabled.as_ref().is_some_and(|disabled| disabled()) {
            return self.dispatcher.dispatch(event, event_name);
        }
        let class = event.class();
        let event_name = event_name.unwrap_or(class);

        let current = self
            .request_stack
            .as_ref()
            .and_then(|stack| stack.current_request())
            .map_or(0, |request| request.0.get());
        {
            let mut state = self.lock();
            state.call_stack.get_or_insert_with(Vec::new);
            state.current_request_hash = current;
        }

        if event.is_propagation_stopped() {
            self.debug(
                &format!(
                    "The \"{event_name}\" event is already stopped. No listeners have been called."
                ),
                Context::new(),
            );
        }

        self.pre_process(event_name);
        let result = (|| {
            self.hooks.before_dispatch(event_name, event)?;
            let stopwatch_event = self.stopwatch.start(event_name, Some("section"));
            let dispatched = self.dispatcher.dispatch(event, Some(event_name));
            if stopwatch_event.is_started() {
                // Started, so stop() has a period to close.
                let _ = stopwatch_event.stop();
            }
            self.hooks.after_dispatch(event_name, event)?;
            dispatched
        })();
        self.lock().current_request_hash = current;
        self.post_process(event_name);
        result
    }
}

impl<D: EventDispatcherInterface + 'static> EventDispatcherInterface
    for TraceableEventDispatcher<D>
{
    fn add_listener(
        &self,
        event_name: &str,
        listener: Listener,
        priority: i64,
    ) -> Result<(), BadMethodCallException> {
        self.dispatcher.add_listener(event_name, listener, priority)
    }

    fn add_subscriber(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        self.dispatcher.add_subscriber(subscriber)
    }

    fn remove_listener(
        &self,
        event_name: &str,
        listener: &Listener,
    ) -> Result<(), BadMethodCallException> {
        match self.wrapper_of(event_name, listener, true) {
            Some(wrapper) => self
                .dispatcher
                .remove_listener(event_name, &wrapped(&wrapper)),
            None => self.dispatcher.remove_listener(event_name, listener),
        }
    }

    fn remove_subscriber(
        &self,
        subscriber: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        self.dispatcher.remove_subscriber(subscriber)
    }

    fn get_listeners(&self, event_name: &str) -> Vec<Listener> {
        self.dispatcher.get_listeners(event_name)
    }

    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        self.dispatcher.get_all_listeners()
    }

    /// While dispatching, a listener's priority is its wrapper's.
    fn get_listener_priority(&self, event_name: &str, listener: &Listener) -> Option<i64> {
        match self.wrapper_of(event_name, listener, false) {
            Some(wrapper) => self
                .dispatcher
                .get_listener_priority(event_name, &wrapped(&wrapper)),
            None => self.dispatcher.get_listener_priority(event_name, listener),
        }
    }

    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        self.dispatcher.has_listeners(event_name)
    }
}

/// The TraceableEventDispatcherTest.php methods that read private
/// properties by reflection (`wrappedListeners`, `calledListenerInfos`).
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use symfony_event_dispatcher_contracts::Event;

    use super::*;
    use crate::EventDispatcher;

    type TestResult = Result<(), Throwable>;

    fn new() -> Arc<TraceableEventDispatcher<EventDispatcher>> {
        TraceableEventDispatcher::new(EventDispatcher::new(), Arc::new(Stopwatch::new(false)))
    }

    fn noop() -> Listener {
        Listener::closure(|_, _, _| Ok(()))
    }

    fn resetting(t: &Arc<TraceableEventDispatcher<EventDispatcher>>) -> Listener {
        let t = Arc::downgrade(t);
        Listener::closure(move |_, _, _| {
            if let Some(t) = t.upgrade() {
                t.reset();
            }
            Ok(())
        })
    }

    #[test]
    fn testResetClearsWrappedListenersOnMidDispatchReset() -> TestResult {
        let tdispatcher = new();
        tdispatcher.add_listener("foo", noop(), 0)?;
        // Second listener triggers a reset mid-dispatch. postProcess() then
        // returns early on null === $callStack and leaves wrappedListeners['foo']
        // uncleared; subsequent dispatches keep appending WrappedListener
        // instances unless reset() also clears wrappedListeners.
        tdispatcher.add_listener("foo", resetting(&tdispatcher), 0)?;

        for _ in 0..5 {
            tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
        }

        assert_eq!(0, tdispatcher.state_sizes().0);
        Ok(())
    }

    #[test]
    fn testCallStackIsNotLeakingOnRepeatedDispatch() -> TestResult {
        let tdispatcher = new();
        tdispatcher.add_listener("foo", noop(), 5)?;

        for _ in 0..5 {
            tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
        }

        assert_eq!(5, tdispatcher.get_called_listeners(None).len());

        // repeated invocations are aggregated as a counter, the stored state must not grow
        assert_eq!(vec![(0, "foo".to_owned(), 1)], tdispatcher.state_sizes().1);
        Ok(())
    }

    #[test]
    fn testCallStackIsNotLeakingWithNestedSameEventDispatch() -> TestResult {
        let tdispatcher = new();
        let iteration = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (t, i) = (Arc::downgrade(&tdispatcher), iteration.clone());
        tdispatcher.add_listener(
            "foo",
            Listener::closure(move |_, _, _| {
                if i.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 1
                    && let Some(t) = t.upgrade()
                {
                    t.dispatch(&mut Event::new(), Some("foo"))?;
                }
                Ok(())
            }),
            0,
        )?;

        tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

        // the listener ran twice: once for the outer dispatch, once for the nested one
        assert_eq!(2, tdispatcher.get_called_listeners(None).len());

        assert_eq!(vec![(0, "foo".to_owned(), 1)], tdispatcher.state_sizes().1);
        Ok(())
    }

    #[test]
    fn testResetDuringDispatchClearsWrappedListeners() -> TestResult {
        let tdispatcher = new();
        tdispatcher.add_listener("foo", noop(), 0)?;
        // postProcess() returns early after a mid-dispatch reset, so reset() itself must clear wrappedListeners
        tdispatcher.add_listener("foo", resetting(&tdispatcher), 0)?;

        for _ in 0..5 {
            tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
        }

        assert_eq!(0, tdispatcher.state_sizes().0);
        Ok(())
    }
}
