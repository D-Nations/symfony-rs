//! ImmutableEventDispatcherTest.php at symfony/event-dispatcher
//! 7458da64220376b2e0dc2d8451bf43382c1ad297. Names and assertions are
//! PHPUnit's; the PHPUnit mock is `Inner`, which records each call.
//!
//! In PHP the inner dispatcher may return another event object; here the
//! caller keeps its event, so testDispatchDelegates checks that the same
//! event and name reach the inner dispatcher exactly once.
//!
//! Upstream: symfony/event-dispatcher Tests/ImmutableEventDispatcherTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use symfony_event_dispatcher::{
    BadMethodCallException, Callable, ContractsEventDispatcherInterface, Event, EventDispatcher,
    EventDispatcherInterface, EventObject, EventSubscriber, ImmutableEventDispatcher, Listener,
    Subscription, Throwable, UndefinedMethod,
};

type Calls = Arc<Mutex<Vec<String>>>;

#[derive(Default)]
struct Inner {
    calls: Calls,
    result: Option<Listener>,
}

fn inner(result: Option<Listener>) -> (Inner, Calls) {
    let calls = Calls::default();
    (
        Inner {
            calls: calls.clone(),
            result,
        },
        calls,
    )
}

fn calls(calls: &Calls) -> Vec<String> {
    calls.lock().unwrap().clone()
}

impl Inner {
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl ContractsEventDispatcherInterface for Inner {
    fn dispatch(&self, event: &mut dyn EventObject, name: Option<&str>) -> Result<(), Throwable> {
        let address = std::ptr::from_mut(event).cast::<()>().addr();
        self.record(format!("dispatch {address:x} {name:?}"));
        Ok(())
    }
}

impl EventDispatcherInterface for Inner {
    fn add_listener(&self, _: &str, _: Listener, _: i64) -> Result<(), BadMethodCallException> {
        unreachable!()
    }
    fn add_subscriber(&self, _: Arc<dyn EventSubscriber>) -> Result<(), BadMethodCallException> {
        unreachable!()
    }
    fn remove_listener(&self, _: &str, _: &Listener) -> Result<(), BadMethodCallException> {
        unreachable!()
    }
    fn remove_subscriber(
        &self,
        _: &Arc<dyn EventSubscriber>,
    ) -> Result<(), BadMethodCallException> {
        unreachable!()
    }
    fn get_listeners(&self, name: &str) -> Vec<Listener> {
        self.record(format!("getListeners {name}"));
        self.result.iter().cloned().collect()
    }
    fn get_all_listeners(&self) -> Vec<(String, Vec<Listener>)> {
        unreachable!()
    }
    fn get_listener_priority(&self, _: &str, _: &Listener) -> Option<i64> {
        unreachable!()
    }
    fn has_listeners(&self, name: Option<&str>) -> bool {
        self.record(format!("hasListeners {name:?}"));
        true
    }
}

struct Stub;
impl Callable for Stub {
    fn class(&self) -> &'static str {
        "Stub"
    }
    fn call(
        &self,
        method: &str,
        _: &mut dyn EventObject,
        _: &str,
        _: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        Err(UndefinedMethod::error(self.class(), method))
    }
}
impl EventSubscriber for Stub {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        Vec::new()
    }
}

fn unmodifiable() -> Result<(), BadMethodCallException> {
    Err(BadMethodCallException {
        message: "Unmodifiable event dispatchers must not be modified.",
    })
}

#[test]
fn testDispatchDelegates() -> Result<(), Throwable> {
    let (inner, log) = inner(None);
    let dispatcher = ImmutableEventDispatcher::new(inner);

    let mut event = Event::new();
    let address = std::ptr::from_mut(&mut event).cast::<()>().addr();

    dispatcher.dispatch(&mut event, Some("event"))?;
    assert_eq!(
        vec![format!("dispatch {address:x} Some(\"event\")")],
        calls(&log)
    );
    Ok(())
}

#[test]
fn testGetListenersDelegates() {
    let result = Listener::closure(|_, _, _| Ok(()));
    let (inner, log) = inner(Some(result.clone()));
    let dispatcher = ImmutableEventDispatcher::new(inner);

    assert_eq!(vec![result], dispatcher.get_listeners("event"));
    assert_eq!(vec!["getListeners event"], calls(&log));
}

#[test]
fn testHasListenersDelegates() {
    let (inner, log) = inner(None);
    let dispatcher = ImmutableEventDispatcher::new(inner);

    assert!(dispatcher.has_listeners(Some("event")));
    assert_eq!(vec!["hasListeners Some(\"event\")"], calls(&log));
}

#[test]
fn testAddListenerDisallowed() {
    let dispatcher = ImmutableEventDispatcher::new(EventDispatcher::new());

    let listener = Listener::closure(|_, _, _| Ok(()));
    assert_eq!(
        unmodifiable(),
        dispatcher.add_listener("event", listener, 0)
    );
}

#[test]
fn testAddSubscriberDisallowed() {
    let dispatcher = ImmutableEventDispatcher::new(EventDispatcher::new());

    assert_eq!(unmodifiable(), dispatcher.add_subscriber(Arc::new(Stub)));
}

#[test]
fn testRemoveListenerDisallowed() {
    let dispatcher = ImmutableEventDispatcher::new(EventDispatcher::new());

    let listener = Listener::closure(|_, _, _| Ok(()));
    assert_eq!(
        unmodifiable(),
        dispatcher.remove_listener("event", &listener)
    );
}

#[test]
fn testRemoveSubscriberDisallowed() {
    let dispatcher = ImmutableEventDispatcher::new(EventDispatcher::new());

    let subscriber: Arc<dyn EventSubscriber> = Arc::new(Stub);
    assert_eq!(unmodifiable(), dispatcher.remove_subscriber(&subscriber));
}
