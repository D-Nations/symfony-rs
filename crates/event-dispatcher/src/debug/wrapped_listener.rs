//! Port of Symfony\Component\EventDispatcher\Debug\WrappedListener.php.
//!
//! The `stub` of `getInfo()` is VarDumper's `ClassStub` when VarDumper is
//! installed, else the string `pretty()`. VarDumper is not ported, so it is
//! always the string.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher Debug/WrappedListener.php

use std::{
    any::Any,
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

use symfony_event_dispatcher_contracts::{EventObject, Throwable};
use symfony_stopwatch::Stopwatch;

use crate::{
    interface::EventDispatcherInterface,
    listener::{Callable, Closure, Listener, Object, UndefinedMethod},
};

/// `getInfo()`: what the profiler shows for one listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerInfo {
    pub event: String,
    pub priority: Option<i64>,
    pub pretty: String,
    pub stub: String,
}

#[derive(Default)]
struct State {
    called: bool,
    stopped_propagation: bool,
    priority: Option<i64>,
}

/// A listener that records whether it was called, whether it stopped
/// propagation, and how long it ran (a Stopwatch event in the
/// `event_listener` category).
pub struct WrappedListener {
    listener: Listener,
    name: String,
    pretty: String,
    stopwatch: Arc<Stopwatch>,
    dispatcher: Option<Weak<dyn EventDispatcherInterface>>,
    state: Mutex<State>,
}

impl WrappedListener {
    pub const CLASS: &'static str = "Symfony\\Component\\EventDispatcher\\Debug\\WrappedListener";

    /// `name` replaces the described name (the Stopwatch event name);
    /// `dispatcher`, when set, is passed to the listener instead of the
    /// calling one and answers priority lookups.
    pub fn new(
        listener: Listener,
        name: Option<&str>,
        stopwatch: Arc<Stopwatch>,
        dispatcher: Option<Weak<dyn EventDispatcherInterface>>,
        priority: Option<i64>,
    ) -> Self {
        let (described, pretty) = describe(&listener);
        Self {
            listener,
            name: name.map_or(described, str::to_owned),
            pretty,
            stopwatch,
            dispatcher,
            state: Mutex::new(State {
                priority,
                ..State::default()
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Nothing panics while this lock is held.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn get_wrapped_listener(&self) -> &Listener {
        &self.listener
    }

    pub fn was_called(&self) -> bool {
        self.lock().called
    }

    pub fn stopped_propagation(&self) -> bool {
        self.lock().stopped_propagation
    }

    pub fn get_pretty(&self) -> &str {
        &self.pretty
    }

    pub fn get_info(&self, event_name: &str) -> ListenerInfo {
        let known = self.lock().priority;
        let priority = known.or_else(|| {
            let found = self
                .dispatcher
                .as_ref()
                .and_then(Weak::upgrade)
                .and_then(|d| d.get_listener_priority(event_name, &self.listener));
            self.lock().priority = found;
            found
        });
        ListenerInfo {
            event: event_name.to_owned(),
            priority,
            pretty: self.pretty.clone(),
            stub: format!("{}()", self.pretty),
        }
    }

    /// `__invoke()`.
    fn invoke(
        &self,
        event: &mut dyn EventObject,
        event_name: &str,
        caller: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        let own = self.dispatcher.as_ref().and_then(Weak::upgrade);
        let dispatcher: &dyn EventDispatcherInterface = own.as_deref().unwrap_or(caller);

        let known = {
            let mut state = self.lock();
            state.called = true;
            state.priority
        };
        if known.is_none() {
            let priority = dispatcher.get_listener_priority(event_name, &self.listener);
            self.lock().priority = priority;
        }

        let stopwatch_event = self.stopwatch.start(&self.name, Some("event_listener"));
        let result = self.listener.call(event, event_name, dispatcher);
        if stopwatch_event.is_started() {
            // Started, so stop() has a period to close.
            let _ = stopwatch_event.stop();
        }
        result?;

        if event.is_propagation_stopped() {
            self.lock().stopped_propagation = true;
        }
        Ok(())
    }
}

impl Callable for WrappedListener {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        if method.eq_ignore_ascii_case("__invoke") {
            self.invoke(event, event_name, dispatcher)
        } else {
            Err(UndefinedMethod::error(Self::CLASS, method))
        }
    }
}

/// `$listener instanceof WrappedListener`.
pub fn as_wrapped(listener: &Listener) -> Option<Arc<WrappedListener>> {
    match listener {
        Listener::Invokable(object) => {
            let any: Arc<dyn Any + Send + Sync> = object.clone();
            any.downcast::<WrappedListener>().ok()
        }
        _ => None,
    }
}

/// A wrapped listener as the inner dispatcher stores it.
pub fn wrapped(listener: &Arc<WrappedListener>) -> Listener {
    let object: Object = listener.clone();
    Listener::Invokable(object)
}

/// The constructor's naming: `(name, pretty)`.
fn describe(listener: &Listener) -> (String, String) {
    match listener {
        Listener::Method(object, method) => {
            let class = object.class().to_owned();
            (class.clone(), format!("{class}::{method}"))
        }
        Listener::Lazy(factory, method) => {
            // parseListener(): a `#[\Closure(name: ...)]` factory names the
            // class; otherwise the factory is a Closure object.
            let class = factory.name().unwrap_or("Closure").to_owned();
            (class.clone(), format!("{class}::{method}"))
        }
        Listener::Closure(closure) => match closure_method(closure) {
            Some((class, method)) => (class.to_owned(), format!("{class}::{method}")),
            None => ("closure".to_owned(), "closure".to_owned()),
        },
        Listener::Invokable(object) => {
            let class = object.class().to_owned();
            (class.clone(), format!("{class}::__invoke"))
        }
    }
}

/// `ReflectionFunction::getClosureCalledClass()` and the method name of a
/// closure made from a method.
fn closure_method(closure: &Closure) -> Option<(&'static str, &'static str)> {
    closure.bound_method()
}
