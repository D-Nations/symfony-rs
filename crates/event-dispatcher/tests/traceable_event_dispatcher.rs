//! Debug/TraceableEventDispatcherTest.php at symfony/event-dispatcher
//! 7458da64220376b2e0dc2d8451bf43382c1ad297. Names and assertions are
//! PHPUnit's. `assertArrayHasKey('stub', ...)` holds by construction (the
//! info is a struct). The four tests that read private properties by
//! reflection are unit tests in `src/debug/traceable_event_dispatcher.rs`.
//!
//! Upstream: symfony/event-dispatcher Tests/Debug/TraceableEventDispatcherTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
};

use psr_log::{ContextValue, LogLevel, LoggerInterface, context};
use symfony_event_dispatcher::{
    Callable, ContractsEventDispatcherInterface, Event, EventDispatcher, EventDispatcherInterface,
    EventObject, EventSubscriber, Listener, Subscription, Throwable, UndefinedMethod,
    debug::{ListenerInfo, Options, TraceableEventDispatcher},
};
use symfony_stopwatch::Stopwatch;

type Traceable = TraceableEventDispatcher<Arc<EventDispatcher>>;
type TestResult = Result<(), Throwable>;

fn new() -> (Arc<EventDispatcher>, Arc<Traceable>) {
    let dispatcher = Arc::new(EventDispatcher::new());
    let tdispatcher =
        TraceableEventDispatcher::new(dispatcher.clone(), Arc::new(Stopwatch::new(false)));
    (dispatcher, tdispatcher)
}

fn noop() -> Listener {
    Listener::closure(|_, _, _| Ok(()))
}

fn stop_propagation() -> Listener {
    Listener::closure(|event, _, _| {
        if let Some(event) = event.downcast_mut::<Event>() {
            event.stop_propagation();
        }
        Ok(())
    })
}

fn counter(count: &Arc<AtomicUsize>) -> Listener {
    let count = count.clone();
    Listener::closure(move |_, _, _| {
        count.fetch_add(1, SeqCst);
        Ok(())
    })
}

/// The info without its stub, as the PHP tests compare it.
fn info(event: &str, pretty: &str, priority: Option<i64>) -> (String, String, Option<i64>) {
    (event.to_owned(), pretty.to_owned(), priority)
}

fn without_stub(listeners: &[ListenerInfo]) -> Vec<(String, String, Option<i64>)> {
    listeners
        .iter()
        .map(|l| {
            assert_ne!(l.stub, "");
            (l.event.clone(), l.pretty.clone(), l.priority)
        })
        .collect()
}

#[test]
fn testAddRemoveListener() -> TestResult {
    let (dispatcher, tdispatcher) = new();

    let listener = noop();
    tdispatcher.add_listener("foo", listener.clone(), 0)?;
    let listeners = dispatcher.get_listeners("foo");
    assert_eq!(1, listeners.len());
    assert_eq!(listener, listeners[0]);

    tdispatcher.remove_listener("foo", &listener)?;
    assert_eq!(0, dispatcher.get_listeners("foo").len());
    Ok(())
}

#[test]
fn testGetListeners() -> TestResult {
    let (dispatcher, tdispatcher) = new();

    tdispatcher.add_listener("foo", noop(), 0)?;
    assert_eq!(
        dispatcher.get_listeners("foo"),
        tdispatcher.get_listeners("foo")
    );
    Ok(())
}

#[test]
fn testHasListeners() -> TestResult {
    let (dispatcher, tdispatcher) = new();

    assert!(!dispatcher.has_listeners(Some("foo")));
    assert!(!tdispatcher.has_listeners(Some("foo")));

    tdispatcher.add_listener("foo", noop(), 0)?;
    assert!(dispatcher.has_listeners(Some("foo")));
    assert!(tdispatcher.has_listeners(Some("foo")));
    Ok(())
}

#[test]
fn testGetListenerPriority() -> TestResult {
    let (dispatcher, tdispatcher) = new();

    tdispatcher.add_listener("foo", noop(), 123)?;

    let listeners = dispatcher.get_listeners("foo");
    assert_eq!(
        Some(123),
        tdispatcher.get_listener_priority("foo", &listeners[0])
    );

    // Verify that priority is preserved when listener is removed and re-added
    // in preProcess() and postProcess().
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    let listeners = dispatcher.get_listeners("foo");
    assert_eq!(
        Some(123),
        tdispatcher.get_listener_priority("foo", &listeners[0])
    );
    Ok(())
}

#[test]
fn testGetListenerPriorityWhileDispatching() -> TestResult {
    let (_, tdispatcher) = new();
    let priority_while_dispatching = Arc::new(Mutex::new(None));

    let this: Arc<OnceLock<Listener>> = Arc::default();
    let listener = {
        let (t, this, seen) = (
            Arc::downgrade(&tdispatcher),
            this.clone(),
            priority_while_dispatching.clone(),
        );
        Listener::closure(move |_, _, _| {
            if let (Some(t), Some(listener)) = (t.upgrade(), this.get()) {
                *seen.lock().unwrap() = t.get_listener_priority("bar", listener);
            }
            Ok(())
        })
    };
    let _ = this.set(listener.clone());

    tdispatcher.add_listener("bar", listener, 5)?;
    tdispatcher.dispatch(&mut Event::new(), Some("bar"))?;
    assert_eq!(Some(5), *priority_while_dispatching.lock().unwrap());
    Ok(())
}

struct TestSubscriber;
impl Callable for TestSubscriber {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\EventDispatcher\\Tests\\Debug\\EventSubscriber"
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
impl EventSubscriber for TestSubscriber {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        vec![("foo", Subscription::Method("call"))]
    }
}

#[test]
fn testAddRemoveSubscriber() -> TestResult {
    let (dispatcher, tdispatcher) = new();

    let subscriber: Arc<dyn EventSubscriber> = Arc::new(TestSubscriber);

    tdispatcher.add_subscriber(subscriber.clone())?;
    let listeners = dispatcher.get_listeners("foo");
    assert_eq!(1, listeners.len());
    let object: symfony_event_dispatcher::Object = subscriber.clone();
    assert_eq!(Listener::Method(object, "call"), listeners[0]);

    tdispatcher.remove_subscriber(&subscriber)?;
    assert_eq!(0, dispatcher.get_listeners("foo").len());
    Ok(())
}

#[test]
fn testGetCalledListeners() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 5)?;

    let listeners = tdispatcher.get_not_called_listeners(None);
    assert_eq!(
        Vec::<ListenerInfo>::new(),
        tdispatcher.get_called_listeners(None)
    );
    assert_eq!(
        vec![info("foo", "closure", Some(5))],
        without_stub(&listeners)
    );

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    let listeners = tdispatcher.get_called_listeners(None);
    assert_eq!(
        vec![info("foo", "closure", Some(5))],
        without_stub(&listeners)
    );
    assert_eq!(
        Vec::<ListenerInfo>::new(),
        tdispatcher.get_not_called_listeners(None)
    );
    Ok(())
}

#[test]
fn testGetNotCalledClosureListeners() -> TestResult {
    let instantiation_count = Arc::new(AtomicUsize::new(0));

    let (_, tdispatcher) = new();
    let count = instantiation_count.clone();
    tdispatcher.add_listener(
        "foo",
        Listener::lazy(
            move || -> symfony_event_dispatcher::Object {
                count.fetch_add(1, SeqCst);
                Arc::new(TestSubscriber)
            },
            "onFoo",
        ),
        0,
    )?;

    tdispatcher.get_not_called_listeners(None);

    assert_eq!(0, instantiation_count.load(SeqCst));
    Ok(())
}

#[test]
fn testClearCalledListeners() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 5)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    tdispatcher.reset();

    let listeners = tdispatcher.get_not_called_listeners(None);
    assert_eq!(
        Vec::<ListenerInfo>::new(),
        tdispatcher.get_called_listeners(None)
    );
    assert_eq!(
        vec![info("foo", "closure", Some(5))],
        without_stub(&listeners)
    );
    Ok(())
}

#[test]
fn testDispatchAfterReset() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 5)?;

    tdispatcher.reset();
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    let listeners = tdispatcher.get_called_listeners(None);
    assert_ne!(listeners[0].stub, "");
    Ok(())
}

#[test]
fn testGetCalledListenersNested() -> TestResult {
    let seen = Arc::new(Mutex::new(None::<usize>));
    let (_, dispatcher) = new();
    let record = seen.clone();
    dispatcher.add_listener(
        "foo",
        Listener::closure(move |_, _, dispatcher| {
            *record.lock().unwrap() = Some(std::ptr::from_ref(dispatcher).cast::<()>().addr());
            dispatcher.dispatch(&mut Event::new(), Some("bar"))
        }),
        0,
    )?;
    dispatcher.add_listener("bar", noop(), 0)?;
    dispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert_eq!(
        Some(Arc::as_ptr(&dispatcher).cast::<()>().addr()),
        *seen.lock().unwrap()
    );
    assert_eq!(2, dispatcher.get_called_listeners(None).len());
    Ok(())
}

#[test]
fn testItReturnsNoOrphanedEventsWhenCreated() {
    let (_, tdispatcher) = new();
    let events = tdispatcher.get_orphaned_events(None);
    assert_eq!(Vec::<String>::new(), events);
}

#[test]
fn testItReturnsOrphanedEventsAfterDispatch() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    let events = tdispatcher.get_orphaned_events(None);
    assert_eq!(1, events.len());
    assert_eq!(vec!["foo"], events);
    Ok(())
}

#[test]
fn testItDoesNotReturnHandledEvents() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 0)?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    let events = tdispatcher.get_orphaned_events(None);
    assert_eq!(Vec::<String>::new(), events);
    Ok(())
}

/// One `[level, message, context]` record.
type Record = (LogLevel, String, Vec<(String, ContextValue)>);

/// `ErrorHandler`'s `BufferingLogger`: keeps the records.
#[derive(Default)]
struct BufferingLogger(Mutex<Vec<Record>>);

impl BufferingLogger {
    fn clean_logs(&self) -> Vec<Record> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

impl LoggerInterface for BufferingLogger {
    fn log(&self, level: LogLevel, message: &str, context: psr_log::Context) {
        self.0
            .lock()
            .unwrap()
            .push((level, message.to_owned(), context));
    }
}

fn with_logger() -> (Arc<BufferingLogger>, Arc<Traceable>) {
    let logger = Arc::new(BufferingLogger::default());
    let options = Options {
        logger: Some(logger.clone()),
        ..Options::default()
    };
    let tdispatcher = TraceableEventDispatcher::with_options(
        Arc::new(EventDispatcher::new()),
        Arc::new(Stopwatch::new(false)),
        options,
    );
    (logger, tdispatcher)
}

#[test]
fn testLogger() -> TestResult {
    let (logger, tdispatcher) = with_logger();
    tdispatcher.add_listener("foo", noop(), 0)?;
    tdispatcher.add_listener("foo", noop(), 0)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    let notified = (
        LogLevel::Debug,
        "Notified event \"{event}\" to listener \"{listener}\".".to_owned(),
        context([("event", "foo"), ("listener", "closure")]),
    );
    assert_eq!(vec![notified.clone(), notified], logger.clean_logs());
    Ok(())
}

#[test]
fn testLoggerWithStoppedEvent() -> TestResult {
    let (logger, tdispatcher) = with_logger();
    tdispatcher.add_listener("foo", stop_propagation(), 0)?;
    tdispatcher.add_listener("foo", noop(), 0)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    let entry = |message: &str| {
        (
            LogLevel::Debug,
            message.to_owned(),
            context([("event", "foo"), ("listener", "closure")]),
        )
    };
    assert_eq!(
        vec![
            entry("Notified event \"{event}\" to listener \"{listener}\"."),
            entry("Listener \"{listener}\" stopped propagation of the event \"{event}\"."),
            entry("Listener \"{listener}\" was not called for event \"{event}\"."),
        ],
        logger.clean_logs()
    );
    Ok(())
}

#[test]
fn testDispatchCallListeners() -> TestResult {
    let called = Arc::new(Mutex::new(Vec::new()));

    let (_, tdispatcher) = new();
    let push = |name: &'static str| {
        let called = called.clone();
        Listener::closure(move |_, _, _| {
            called.lock().unwrap().push(name);
            Ok(())
        })
    };
    tdispatcher.add_listener("foo", push("foo1"), 10)?;
    tdispatcher.add_listener("foo", push("foo2"), 20)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(vec!["foo2", "foo1"], *called.lock().unwrap());
    Ok(())
}

#[test]
fn testDispatchNested() -> TestResult {
    let (_, dispatcher) = new();
    let r#loop = Arc::new(AtomicUsize::new(1));
    let dispatched_events = Arc::new(AtomicUsize::new(0));
    let listener1 = {
        let (d, r#loop) = (Arc::downgrade(&dispatcher), r#loop.clone());
        Listener::closure(move |_, _, _| {
            if r#loop.fetch_add(1, SeqCst) + 1 == 2
                && let Some(d) = d.upgrade()
            {
                d.dispatch(&mut Event::new(), Some("foo"))?;
            }
            Ok(())
        })
    };
    dispatcher.add_listener("foo", listener1, 0)?;
    dispatcher.add_listener("foo", counter(&dispatched_events), 0)?;

    dispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(2, dispatched_events.load(SeqCst));
    Ok(())
}

#[test]
fn testDispatchReusedEventNested() -> TestResult {
    let nested_call = Arc::new(AtomicBool::new(false));
    let (_, dispatcher) = new();
    let d = Arc::downgrade(&dispatcher);
    dispatcher.add_listener(
        "foo",
        Listener::closure(move |_, _, _| match d.upgrade() {
            Some(d) => d.dispatch(&mut Event::new(), Some("bar")),
            None => Ok(()),
        }),
        0,
    )?;
    let flag = nested_call.clone();
    dispatcher.add_listener(
        "bar",
        Listener::closure(move |_, _, _| {
            flag.store(true, SeqCst);
            Ok(())
        }),
        0,
    )?;

    assert!(!nested_call.load(SeqCst));
    dispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert!(nested_call.load(SeqCst));
    Ok(())
}

#[test]
fn testListenerCanRemoveItselfWhenExecuted() -> TestResult {
    let (_, event_dispatcher) = new();
    let this: Arc<OnceLock<Listener>> = Arc::default();
    let listener1 = {
        let this = this.clone();
        Listener::closure(move |_, _, dispatcher| {
            if let Some(listener1) = this.get() {
                dispatcher.remove_listener("foo", listener1)?;
            }
            Ok(())
        })
    };
    let _ = this.set(listener1.clone());
    event_dispatcher.add_listener("foo", listener1, 0)?;
    event_dispatcher.add_listener("foo", noop(), 0)?;
    event_dispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(
        1,
        event_dispatcher.get_listeners("foo").len(),
        "expected listener1 to be removed"
    );
    Ok(())
}

#[test]
fn testClearOrphanedEvents() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    let events = tdispatcher.get_orphaned_events(None);
    assert_eq!(1, events.len());
    tdispatcher.reset();
    let events = tdispatcher.get_orphaned_events(None);
    assert_eq!(0, events.len());
    Ok(())
}

#[test]
fn testCallStackIsNotLeakingWithMultipleListeners() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 10)?;
    tdispatcher.add_listener("foo", noop(), 5)?;

    for _ in 0..5 {
        tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    }

    assert_eq!(10, tdispatcher.get_called_listeners(None).len());
    Ok(())
}

#[test]
fn testCallStackCleanupDoesNotAffectOtherEvents() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 0)?;
    tdispatcher.add_listener("bar", noop(), 0)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    tdispatcher.dispatch(&mut Event::new(), Some("bar"))?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(3, tdispatcher.get_called_listeners(None).len());
    Ok(())
}

#[test]
fn testCallStackIsNotLeakingWithStoppedPropagation() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", stop_propagation(), 10)?;
    tdispatcher.add_listener("foo", noop(), 5)?;

    for _ in 0..5 {
        tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    }

    assert_eq!(5, tdispatcher.get_called_listeners(None).len());
    assert_eq!(1, tdispatcher.get_not_called_listeners(None).len());
    Ok(())
}

#[test]
fn testPreviouslyCalledListenerStaysCalledAfterLaterStopPropagation() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 5)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert_eq!(1, tdispatcher.get_called_listeners(None).len());

    tdispatcher.add_listener("foo", stop_propagation(), 10)?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(2, tdispatcher.get_called_listeners(None).len());
    assert_eq!(0, tdispatcher.get_not_called_listeners(None).len());
    Ok(())
}

#[test]
fn testCallStackCleanupWhenEventBecomesOrphaned() -> TestResult {
    let (_, tdispatcher) = new();
    let listener = noop();
    tdispatcher.add_listener("foo", listener.clone(), 0)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert_eq!(1, tdispatcher.get_called_listeners(None).len());

    tdispatcher.remove_listener("foo", &listener)?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(1, tdispatcher.get_called_listeners(None).len());
    assert_eq!(1, tdispatcher.get_orphaned_events(None).len());
    Ok(())
}

#[test]
fn testCallStackIsNotLeakingWhenListenerIsAddedBetweenDispatches() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 10)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert_eq!(1, tdispatcher.get_called_listeners(None).len());

    tdispatcher.add_listener("foo", noop(), 5)?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(3, tdispatcher.get_called_listeners(None).len());
    Ok(())
}

#[test]
fn testResetDuringDispatch() -> TestResult {
    let (_, tdispatcher) = new();
    let t = Arc::downgrade(&tdispatcher);
    tdispatcher.add_listener(
        "foo",
        Listener::closure(move |_, _, _| {
            if let Some(t) = t.upgrade() {
                t.reset();
            }
            Ok(())
        }),
        0,
    )?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(
        Vec::<ListenerInfo>::new(),
        tdispatcher.get_called_listeners(None)
    );
    Ok(())
}

#[test]
fn testCallStackIsNotLeakingWhenListenerIsRemovedBetweenDispatches() -> TestResult {
    let (_, tdispatcher) = new();
    let listener_a = noop();
    let listener_b = noop();
    tdispatcher.add_listener("foo", listener_a, 10)?;
    tdispatcher.add_listener("foo", listener_b.clone(), 5)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    assert_eq!(2, tdispatcher.get_called_listeners(None).len());

    tdispatcher.remove_listener("foo", &listener_b)?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(3, tdispatcher.get_called_listeners(None).len());
    Ok(())
}

#[test]
fn testCountsMultipleInvocationsOfSameListener() -> TestResult {
    let (_, tdispatcher) = new();
    tdispatcher.add_listener("foo", noop(), 0)?;

    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;
    tdispatcher.dispatch(&mut Event::new(), Some("foo"))?;

    assert_eq!(2, tdispatcher.get_called_listeners(None).len());
    Ok(())
}
