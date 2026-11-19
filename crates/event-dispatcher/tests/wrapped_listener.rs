//! Debug/WrappedListenerTest.php at symfony/event-dispatcher
//! 7458da64220376b2e0dc2d8451bf43382c1ad297. Names and assertions are
//! PHPUnit's.
//!
//! testListenerDescription keeps the provider rows this port can express.
//! The four with string callables (`'var_dump'`,
//! `['FooListener', 'listenStatic']`, `['FooListener', 'invalidMethod']`
//! and `\Closure::fromCallable` of the static method) have no Rust form:
//! string callables are not ported (README, "Not ported yet").
//! testStopwatchEventIsStoppedWhenListenerThrows checks a real Stopwatch
//! instead of a mocked StopwatchEvent.
//!
//! Upstream: symfony/event-dispatcher Tests/Debug/WrappedListenerTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use symfony_event_dispatcher::{
    Callable, Closure, Event, EventDispatcher, EventDispatcherInterface, EventObject, Factory,
    Listener, Object, Throwable, UndefinedMethod, debug::WrappedListener,
};
use symfony_stopwatch::Stopwatch;

const FOO_LISTENER: &str = "Symfony\\Component\\EventDispatcher\\Tests\\Debug\\FooListener";

struct FooListener;
impl Callable for FooListener {
    fn class(&self) -> &'static str {
        FOO_LISTENER
    }
    fn call(
        &self,
        method: &str,
        _: &mut dyn EventObject,
        _: &str,
        _: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        match method {
            "listen" | "__invoke" => Ok(()),
            _ => Err(UndefinedMethod::error(self.class(), method)),
        }
    }
}

fn foo() -> Object {
    Arc::new(FooListener)
}

fn provide_listeners_to_describe() -> Vec<(Listener, String)> {
    vec![
        (
            Listener::Invokable(foo()),
            format!("{FOO_LISTENER}::__invoke"),
        ),
        (
            Listener::Method(foo(), "listen"),
            format!("{FOO_LISTENER}::listen"),
        ),
        (Listener::closure(|_, _, _| Ok(())), "closure".to_owned()),
        (
            Closure::from_method(foo(), "listen").into(),
            format!("{FOO_LISTENER}::listen"),
        ),
        // \Closure::fromCallable(static function () {})
        (Listener::closure(|_, _, _| Ok(())), "closure".to_owned()),
        (
            Listener::Lazy(Factory::named(FOO_LISTENER, foo), "listen"),
            format!("{FOO_LISTENER}::listen"),
        ),
    ]
}

#[test]
fn testListenerDescription() {
    for (listener, expected) in provide_listeners_to_describe() {
        let dispatcher: Arc<dyn EventDispatcherInterface> = Arc::new(EventDispatcher::new());
        let wrapped_listener = WrappedListener::new(
            listener,
            None,
            Arc::new(Stopwatch::new(false)),
            Some(Arc::downgrade(&dispatcher)),
            None,
        );

        assert_eq!(expected, wrapped_listener.get_pretty());
    }
}

#[test]
fn testStopwatchEventIsStoppedWhenListenerThrows() {
    let stopwatch = Arc::new(Stopwatch::new(false));
    let dispatcher = EventDispatcher::new();

    let wrapped_listener = WrappedListener::new(
        Listener::closure(|_, _, _| Err("exception".into())),
        None,
        stopwatch.clone(),
        None,
        None,
    );

    let result = wrapped_listener.call("__invoke", &mut Event::new(), "foo", &dispatcher);
    assert!(result.is_err());
    // The listener's Stopwatch event (named "closure") was started and stopped.
    let event = stopwatch.get_event("closure");
    assert!(
        event
            .as_ref()
            .is_ok_and(|e| !e.is_started() && e.get_periods().len() == 1)
    );
}
