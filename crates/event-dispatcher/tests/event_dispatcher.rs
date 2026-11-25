//! EventDispatcherTest.php and ChildEventDispatcherTest.php at
//! symfony/event-dispatcher 7458da64220376b2e0dc2d8451bf43382c1ad297.
//! Names and assertions are PHPUnit's. `ChildEventDispatcherTest` runs the
//! same tests on a subclass, here `EventDispatcher::with_call_listeners`.
//!
//! The tests below the upstream ones pin behaviour checked against PHP
//! 8.4.27 by `oracle/recorders/event_dispatcher_edges.php`, whose output is
//! tests/data/edges.txt.
//!
//! Upstream: symfony/event-dispatcher Tests/EventDispatcherTest.php Tests/ChildEventDispatcherTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
};

use symfony_event_dispatcher::{
    CallListeners, Callable, Closure, ContractsEventDispatcherInterface, Event, EventDispatcher,
    EventDispatcherInterface, EventObject, EventSubscriber, Factory, Listener, Object,
    Subscription, Throwable, UndefinedMethod,
};

const PRE_FOO: &str = "pre.foo";
const POST_FOO: &str = "post.foo";
const PRE_BAR: &str = "pre.bar";

struct ChildEventDispatcher;
impl CallListeners for ChildEventDispatcher {}

fn stop(event: &mut dyn EventObject) -> Result<(), Throwable> {
    match event.downcast_mut::<Event>() {
        Some(event) => {
            event.stop_propagation();
            Ok(())
        }
        None => Err(UndefinedMethod::error(event.class(), "stopPropagation")),
    }
}

#[derive(Default)]
struct CallableClass;
impl Callable for CallableClass {
    fn class(&self) -> &'static str {
        "CallableClass"
    }
    fn call(
        &self,
        method: &str,
        _: &mut dyn EventObject,
        _: &str,
        _: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        match method {
            "__invoke" => Ok(()),
            _ => Err(UndefinedMethod::error(self.class(), method)),
        }
    }
}

#[derive(Default)]
struct TestEventListener {
    name: Mutex<String>,
    pre_foo_invoked: AtomicBool,
    post_foo_invoked: AtomicBool,
}
impl TestEventListener {
    fn named(name: &str) -> Arc<Self> {
        let listener = Arc::new(Self::default());
        name.clone_into(&mut listener.name.lock().unwrap());
        listener
    }
    fn pre_foo(&self) {
        self.pre_foo_invoked.store(true, SeqCst);
    }
}
impl Callable for TestEventListener {
    fn class(&self) -> &'static str {
        "TestEventListener"
    }
    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        _: &str,
        _: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        match method {
            "preFoo" => {
                self.pre_foo();
                Ok(())
            }
            "postFoo" => {
                self.post_foo_invoked.store(true, SeqCst);
                if !self.pre_foo_invoked.load(SeqCst) {
                    stop(event)?;
                }
                Ok(())
            }
            "__invoke" => Ok(()),
            _ => Err(UndefinedMethod::error(self.class(), method)),
        }
    }
}

#[derive(Default)]
struct TestWithDispatcher {
    name: Mutex<Option<String>>,
    dispatcher: Mutex<Option<usize>>,
    invoked: AtomicBool,
}
impl Callable for TestWithDispatcher {
    fn class(&self) -> &'static str {
        "TestWithDispatcher"
    }
    fn call(
        &self,
        method: &str,
        _: &mut dyn EventObject,
        name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        if method != "foo" && method != "__invoke" {
            return Err(UndefinedMethod::error(self.class(), method));
        }
        *self.name.lock().unwrap() = Some(name.to_owned());
        *self.dispatcher.lock().unwrap() = Some(address(dispatcher));
        if method == "__invoke" {
            self.invoked.store(true, SeqCst);
        }
        Ok(())
    }
}

fn address(dispatcher: &dyn EventDispatcherInterface) -> usize {
    std::ptr::from_ref(dispatcher).cast::<()>().addr()
}

macro_rules! subscriber {
    ($name:ident, $events:expr) => {
        struct $name;
        impl Callable for $name {
            fn class(&self) -> &'static str {
                stringify!($name)
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
        impl EventSubscriber for $name {
            fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
                $events
            }
        }
    };
}

subscriber!(
    TestEventSubscriber,
    vec![
        ("pre.foo", Subscription::Method("preFoo")),
        ("post.foo", Subscription::Method("postFoo")),
    ]
);
subscriber!(
    TestEventSubscriberWithPriorities,
    vec![
        ("pre.foo", Subscription::Prioritized("preFoo", 10)),
        ("post.foo", Subscription::Prioritized("postFoo", 0)),
    ]
);
subscriber!(
    TestEventSubscriberWithMultipleListeners,
    vec![(
        "pre.foo",
        Subscription::Methods(vec![("preFoo1", 0), ("preFoo2", 10)])
    )]
);

fn object<T: Callable>(value: &Arc<T>) -> Object {
    value.clone()
}

fn invokable<T: Callable>(value: &Arc<T>) -> Listener {
    Listener::Invokable(object(value))
}

fn noop() -> Listener {
    Listener::closure(|_, _, _| Ok(()))
}

macro_rules! event_dispatcher_tests {
    ($create:expr) => {
        use super::*;

        fn create_event_dispatcher() -> EventDispatcher {
            $create
        }

        fn set_up() -> (EventDispatcher, Arc<TestEventListener>) {
            (
                create_event_dispatcher(),
                Arc::new(TestEventListener::default()),
            )
        }

        #[test]
        fn testInitialState() {
            let (dispatcher, _) = set_up();
            assert_eq!(dispatcher.get_all_listeners(), vec![]);
            assert!(!dispatcher.has_listeners(Some(PRE_FOO)));
            assert!(!dispatcher.has_listeners(Some(POST_FOO)));
        }

        #[test]
        fn testAddListener() -> Result<(), Throwable> {
            let (dispatcher, listener) = set_up();
            dispatcher.add_listener(
                "pre.foo",
                Listener::method(&object(&listener), "preFoo"),
                0,
            )?;
            dispatcher.add_listener(
                "post.foo",
                Closure::from_method(object(&listener), "postFoo").into(),
                0,
            )?;
            assert!(dispatcher.has_listeners(None));
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert!(dispatcher.has_listeners(Some(POST_FOO)));
            assert_eq!(dispatcher.get_listeners(PRE_FOO).len(), 1);
            assert_eq!(dispatcher.get_listeners(POST_FOO).len(), 1);
            assert_eq!(dispatcher.get_all_listeners().len(), 2);
            Ok(())
        }

        #[test]
        fn testGetListenersSortsByPriority() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener1 = object(&TestEventListener::named("1"));
            let listener2 = object(&TestEventListener::named("2"));
            let listener3 = object(&TestEventListener::named("3"));
            let listener4 = object(&TestEventListener::named("4"));

            dispatcher.add_listener("pre.foo", Listener::method(&listener1, "preFoo"), -10)?;
            dispatcher.add_listener("pre.foo", Listener::method(&listener2, "preFoo"), 10)?;
            dispatcher.add_listener("pre.foo", Listener::method(&listener3, "preFoo"), 0)?;
            let listener4_listener: Listener = Closure::from_method(listener4, "preFoo").into();
            dispatcher.add_listener("pre.foo", listener4_listener.clone(), 20)?;

            let expected = vec![
                listener4_listener,
                Listener::method(&listener2, "preFoo"),
                Listener::method(&listener3, "preFoo"),
                Listener::method(&listener1, "preFoo"),
            ];

            assert_eq!(expected, dispatcher.get_listeners("pre.foo"));
            Ok(())
        }

        #[test]
        fn testGetAllListenersSortsByPriority() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listeners: Vec<Listener> = (0..6)
                .map(|_| invokable(&Arc::new(TestEventListener::default())))
                .collect();

            dispatcher.add_listener("pre.foo", listeners[0].clone(), -10)?;
            dispatcher.add_listener("pre.foo", listeners[1].clone(), 0)?;
            dispatcher.add_listener("pre.foo", listeners[2].clone(), 10)?;
            dispatcher.add_listener("post.foo", listeners[3].clone(), -10)?;
            dispatcher.add_listener("post.foo", listeners[4].clone(), 0)?;
            dispatcher.add_listener("post.foo", listeners[5].clone(), 10)?;

            let expected = vec![
                (
                    "pre.foo".to_owned(),
                    vec![
                        listeners[2].clone(),
                        listeners[1].clone(),
                        listeners[0].clone(),
                    ],
                ),
                (
                    "post.foo".to_owned(),
                    vec![
                        listeners[5].clone(),
                        listeners[4].clone(),
                        listeners[3].clone(),
                    ],
                ),
            ];

            assert_eq!(expected, dispatcher.get_all_listeners());
            Ok(())
        }

        #[test]
        fn testGetListenerPriority() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener1 = invokable(&Arc::new(TestEventListener::default()));
            let listener2 = invokable(&Arc::new(TestEventListener::default()));

            dispatcher.add_listener("pre.foo", listener1.clone(), -10)?;
            dispatcher.add_listener("pre.foo", listener2.clone(), 0)?;

            assert_eq!(
                Some(-10),
                dispatcher.get_listener_priority("pre.foo", &listener1)
            );
            assert_eq!(
                Some(0),
                dispatcher.get_listener_priority("pre.foo", &listener2)
            );
            assert_eq!(
                None,
                dispatcher.get_listener_priority("pre.bar", &listener2)
            );
            assert_eq!(None, dispatcher.get_listener_priority("pre.foo", &noop()));
            Ok(())
        }

        #[test]
        fn testDispatch() -> Result<(), Throwable> {
            let (dispatcher, listener) = set_up();
            dispatcher.add_listener(
                "pre.foo",
                Listener::method(&object(&listener), "preFoo"),
                0,
            )?;
            dispatcher.add_listener(
                "post.foo",
                Closure::from_method(object(&listener), "postFoo").into(),
                0,
            )?;
            dispatcher.dispatch(&mut Event::new(), Some(PRE_FOO))?;
            assert!(listener.pre_foo_invoked.load(SeqCst));
            assert!(!listener.post_foo_invoked.load(SeqCst));
            // PHP returns the event; here the caller keeps it.
            let mut event = Event::new();
            dispatcher.dispatch(&mut event, Some("noevent"))?;
            dispatcher.dispatch(&mut event, Some(PRE_FOO))?;
            assert_eq!(event, Event::new());
            Ok(())
        }

        #[test]
        fn testDispatchForClosure() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let invoked = Arc::new(AtomicUsize::new(0));
            let counter = invoked.clone();
            let listener = Listener::closure(move |_, _, _| {
                counter.fetch_add(1, SeqCst);
                Ok(())
            });
            dispatcher.add_listener("pre.foo", listener.clone(), 0)?;
            dispatcher.add_listener("post.foo", listener, 0)?;
            dispatcher.dispatch(&mut Event::new(), Some(PRE_FOO))?;
            assert_eq!(1, invoked.load(SeqCst));
            Ok(())
        }

        #[test]
        fn testStopEventPropagation() -> Result<(), Throwable> {
            let (dispatcher, listener) = set_up();
            let other_listener = Arc::new(TestEventListener::default());

            // postFoo() stops the propagation, so only one listener should
            // be executed
            // Manually set priority to enforce $this->listener to be called first
            dispatcher.add_listener(
                "post.foo",
                Listener::method(&object(&listener), "postFoo"),
                10,
            )?;
            dispatcher.add_listener(
                "post.foo",
                Closure::from_method(object(&other_listener), "postFoo").into(),
                0,
            )?;
            dispatcher.dispatch(&mut Event::new(), Some(POST_FOO))?;
            assert!(listener.post_foo_invoked.load(SeqCst));
            assert!(!other_listener.post_foo_invoked.load(SeqCst));
            Ok(())
        }

        #[test]
        fn testDispatchByPriority() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let invoked = Arc::new(Mutex::new(Vec::new()));
            let push = |name: &'static str| {
                let invoked = invoked.clone();
                Listener::closure(move |_, _, _| {
                    invoked.lock().unwrap().push(name);
                    Ok(())
                })
            };
            dispatcher.add_listener("pre.foo", push("1"), -10)?;
            dispatcher.add_listener("pre.foo", push("2"), 0)?;
            dispatcher.add_listener("pre.foo", push("3"), 10)?;
            dispatcher.dispatch(&mut Event::new(), Some(PRE_FOO))?;
            assert_eq!(vec!["3", "2", "1"], *invoked.lock().unwrap());
            Ok(())
        }

        #[test]
        fn testRemoveListener() -> Result<(), Throwable> {
            let (dispatcher, listener) = set_up();
            let listener = invokable(&listener);
            dispatcher.add_listener("pre.bar", listener.clone(), 0)?;
            assert!(dispatcher.has_listeners(Some(PRE_BAR)));
            dispatcher.remove_listener("pre.bar", &listener)?;
            assert!(!dispatcher.has_listeners(Some(PRE_BAR)));
            dispatcher.remove_listener("notExists", &listener)?;
            Ok(())
        }

        #[test]
        fn testAddSubscriber() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            dispatcher.add_subscriber(Arc::new(TestEventSubscriber))?;
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert!(dispatcher.has_listeners(Some(POST_FOO)));
            Ok(())
        }

        #[test]
        fn testAddSubscriberWithPriorities() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            dispatcher.add_subscriber(Arc::new(TestEventSubscriber))?;
            dispatcher.add_subscriber(Arc::new(TestEventSubscriberWithPriorities))?;

            let listeners = dispatcher.get_listeners("pre.foo");
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert_eq!(listeners.len(), 2);
            let first = listeners[0]
                .object()
                .and_then(|o| o.downcast_ref::<TestEventSubscriberWithPriorities>());
            assert!(first.is_some());
            Ok(())
        }

        #[test]
        fn testAddSubscriberWithMultipleListeners() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            dispatcher.add_subscriber(Arc::new(TestEventSubscriberWithMultipleListeners))?;

            let listeners = dispatcher.get_listeners("pre.foo");
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert_eq!(listeners.len(), 2);
            assert_eq!(Some("preFoo2"), listeners[0].method_name());
            Ok(())
        }

        #[test]
        fn testRemoveSubscriber() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let event_subscriber: Arc<dyn EventSubscriber> = Arc::new(TestEventSubscriber);
            dispatcher.add_subscriber(event_subscriber.clone())?;
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert!(dispatcher.has_listeners(Some(POST_FOO)));
            dispatcher.remove_subscriber(&event_subscriber)?;
            assert!(!dispatcher.has_listeners(Some(PRE_FOO)));
            assert!(!dispatcher.has_listeners(Some(POST_FOO)));
            Ok(())
        }

        #[test]
        fn testRemoveSubscriberWithPriorities() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let event_subscriber: Arc<dyn EventSubscriber> =
                Arc::new(TestEventSubscriberWithPriorities);
            dispatcher.add_subscriber(event_subscriber.clone())?;
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            dispatcher.remove_subscriber(&event_subscriber)?;
            assert!(!dispatcher.has_listeners(Some(PRE_FOO)));
            Ok(())
        }

        #[test]
        fn testRemoveSubscriberWithMultipleListeners() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let event_subscriber: Arc<dyn EventSubscriber> =
                Arc::new(TestEventSubscriberWithMultipleListeners);
            dispatcher.add_subscriber(event_subscriber.clone())?;
            assert!(dispatcher.has_listeners(Some(PRE_FOO)));
            assert_eq!(dispatcher.get_listeners(PRE_FOO).len(), 2);
            dispatcher.remove_subscriber(&event_subscriber)?;
            assert!(!dispatcher.has_listeners(Some(PRE_FOO)));
            Ok(())
        }

        #[test]
        fn testEventReceivesTheDispatcherInstanceAsArgument() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener = Arc::new(TestWithDispatcher::default());
            dispatcher.add_listener("test", Listener::method(&object(&listener), "foo"), 0)?;
            assert_eq!(None, *listener.name.lock().unwrap());
            assert_eq!(None, *listener.dispatcher.lock().unwrap());
            dispatcher.dispatch(&mut Event::new(), Some("test"))?;
            assert_eq!(Some("test".to_owned()), *listener.name.lock().unwrap());
            assert_eq!(
                Some(address(&dispatcher)),
                *listener.dispatcher.lock().unwrap()
            );
            Ok(())
        }

        /// @see <https://bugs.php.net/62976>
        #[test]
        fn testWorkaroundForPhpBug62976() -> Result<(), Throwable> {
            let dispatcher = create_event_dispatcher();
            dispatcher.add_listener("bug.62976", invokable(&Arc::new(CallableClass)), 0)?;
            dispatcher.remove_listener("bug.62976", &noop())?;
            assert!(dispatcher.has_listeners(Some("bug.62976")));
            Ok(())
        }

        #[test]
        fn testHasListenersWhenAddedCallbackListenerIsRemoved() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener = noop();
            dispatcher.add_listener("foo", listener.clone(), 0)?;
            dispatcher.remove_listener("foo", &listener)?;
            assert!(!dispatcher.has_listeners(None));
            Ok(())
        }

        #[test]
        fn testGetListenersWhenAddedCallbackListenerIsRemoved() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener = noop();
            dispatcher.add_listener("foo", listener.clone(), 0)?;
            dispatcher.remove_listener("foo", &listener)?;
            assert_eq!(dispatcher.get_all_listeners(), vec![]);
            Ok(())
        }

        #[test]
        fn testHasListenersWithoutEventsReturnsFalseAfterHasListenersWithEventHasBeenCalled() {
            let (dispatcher, _) = set_up();
            assert!(!dispatcher.has_listeners(Some("foo")));
            assert!(!dispatcher.has_listeners(None));
        }

        #[test]
        fn testHasListenersIsLazy() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let called = Arc::new(AtomicUsize::new(0));
            let count = called.clone();
            let listener = Listener::lazy(
                move || {
                    count.fetch_add(1, SeqCst);
                    object(&Arc::new(CallableClass))
                },
                "onFoo",
            );
            dispatcher.add_listener("foo", listener, 0)?;
            assert!(dispatcher.has_listeners(None));
            assert!(dispatcher.has_listeners(Some("foo")));
            assert_eq!(0, called.load(SeqCst));
            Ok(())
        }

        #[test]
        fn testDispatchLazyListener() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let test = Arc::new(TestWithDispatcher::default());
            let called = Arc::new(AtomicUsize::new(0));
            let factory: Factory = {
                let called = called.clone();
                let test = object(&test);
                Factory::new(move || {
                    called.fetch_add(1, SeqCst);
                    test.clone()
                })
            };
            dispatcher.add_listener("foo", Listener::Lazy(factory.clone(), "foo"), 0)?;
            assert_eq!(0, called.load(SeqCst));
            dispatcher.dispatch(&mut Event::new(), Some("foo"))?;
            assert!(!test.invoked.load(SeqCst));
            dispatcher.dispatch(&mut Event::new(), Some("foo"))?;
            assert_eq!(1, called.load(SeqCst));

            dispatcher.add_listener("bar", Listener::Lazy(factory, "__invoke"), 0)?;
            assert_eq!(1, called.load(SeqCst));
            dispatcher.dispatch(&mut Event::new(), Some("bar"))?;
            assert!(test.invoked.load(SeqCst));
            dispatcher.dispatch(&mut Event::new(), Some("bar"))?;
            assert_eq!(2, called.load(SeqCst));
            Ok(())
        }

        #[test]
        fn testRemoveFindsLazyListeners() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let test = object(&Arc::new(TestWithDispatcher::default()));
            let factory: Factory = {
                let test = test.clone();
                Factory::new(move || test.clone())
            };

            dispatcher.add_listener("foo", Listener::Lazy(factory.clone(), "foo"), 0)?;
            assert!(dispatcher.has_listeners(Some("foo")));
            dispatcher.remove_listener("foo", &Listener::method(&test, "foo"))?;
            assert!(!dispatcher.has_listeners(Some("foo")));

            dispatcher.add_listener("foo", Listener::method(&test, "foo"), 0)?;
            assert!(dispatcher.has_listeners(Some("foo")));
            dispatcher.remove_listener("foo", &Listener::Lazy(factory, "foo"))?;
            assert!(!dispatcher.has_listeners(Some("foo")));
            Ok(())
        }

        #[test]
        fn testPriorityFindsLazyListeners() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let test = object(&Arc::new(TestWithDispatcher::default()));
            let factory: Factory = {
                let test = test.clone();
                Factory::new(move || test.clone())
            };

            dispatcher.add_listener("foo", Listener::Lazy(factory.clone(), "foo"), 3)?;
            assert_eq!(
                Some(3),
                dispatcher.get_listener_priority("foo", &Listener::method(&test, "foo"))
            );
            dispatcher.remove_listener("foo", &Listener::Lazy(factory.clone(), "foo"))?;

            dispatcher.add_listener("foo", Listener::method(&test, "foo"), 5)?;
            assert_eq!(
                Some(5),
                dispatcher.get_listener_priority("foo", &Listener::Lazy(factory, "foo"))
            );
            Ok(())
        }

        #[test]
        fn testGetLazyListeners() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let test = object(&Arc::new(TestWithDispatcher::default()));
            let factory: Factory = {
                let test = test.clone();
                Factory::new(move || test.clone())
            };

            dispatcher.add_listener("foo", Listener::Lazy(factory.clone(), "foo"), 3)?;
            assert_eq!(
                vec![Listener::method(&test, "foo")],
                dispatcher.get_listeners("foo")
            );

            dispatcher.remove_listener("foo", &Listener::method(&test, "foo"))?;
            dispatcher.add_listener("bar", Listener::Lazy(factory, "foo"), 3)?;
            assert_eq!(
                vec![("bar".to_owned(), vec![Listener::method(&test, "foo")])],
                dispatcher.get_all_listeners()
            );
            Ok(())
        }

        #[test]
        fn testGetListenersWhenLazyListenerAddsListeners() -> Result<(), Throwable> {
            let dispatcher = Arc::new(create_event_dispatcher());
            let test1 = object(&Arc::new(TestWithDispatcher::default()));
            let test2 = object(&Arc::new(TestWithDispatcher::default()));
            let test3 = object(&Arc::new(TestWithDispatcher::default()));
            let test4 = object(&Arc::new(TestWithDispatcher::default()));
            let factory = {
                let dispatcher: Weak<EventDispatcher> = Arc::downgrade(&dispatcher);
                let (test2, test3, test4) = (test2.clone(), test3.clone(), test4.clone());
                Listener::lazy(
                    move || {
                        if let Some(dispatcher) = dispatcher.upgrade() {
                            let _ =
                                dispatcher.add_listener("foo", Listener::method(&test3, "foo"), 5);
                            let _ =
                                dispatcher.add_listener("foo", Listener::method(&test4, "foo"), 0);
                        }
                        test2.clone()
                    },
                    "foo",
                )
            };

            dispatcher.add_listener("foo", Listener::method(&test1, "foo"), 0)?;
            dispatcher.add_listener("foo", factory, 0)?;

            let expected = vec![
                Listener::method(&test3, "foo"),
                Listener::method(&test1, "foo"),
                Listener::method(&test2, "foo"),
                Listener::method(&test4, "foo"),
            ];

            assert_eq!(expected, dispatcher.get_listeners("foo"));
            assert_eq!(expected, dispatcher.get_listeners("foo"));
            Ok(())
        }

        #[test]
        fn testMutatingWhilePropagationIsStopped() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let test_loaded = Arc::new(AtomicBool::new(false));
            let test = Arc::new(TestEventListener::default());
            dispatcher.add_listener(
                "foo",
                Closure::from_method(object(&test), "postFoo").into(),
                0,
            )?;
            let lazy = {
                let (test, test_loaded) = (object(&test), test_loaded.clone());
                Listener::lazy(
                    move || {
                        test_loaded.store(true, SeqCst);
                        test.clone()
                    },
                    "preFoo",
                )
            };
            dispatcher.add_listener("foo", lazy, 0)?;

            dispatcher.dispatch(&mut Event::new(), Some("foo"))?;

            assert!(test.post_foo_invoked.load(SeqCst));
            assert!(!test.pre_foo_invoked.load(SeqCst));

            assert_eq!(
                Some(0),
                dispatcher.get_listener_priority(
                    "foo",
                    &Closure::from_method(object(&test), "postFoo").into()
                )
            );

            test.pre_foo();
            dispatcher.dispatch(&mut Event::new(), Some("foo"))?;

            assert!(test_loaded.load(SeqCst));
            Ok(())
        }

        #[test]
        fn testNamedClosures() -> Result<(), Throwable> {
            let (dispatcher, _) = set_up();
            let listener = object(&Arc::new(TestEventListener::default()));

            let callback1: Listener = Closure::from_method(listener.clone(), "__invoke").into();
            let callback2: Listener = Closure::from_method(listener, "__invoke").into();
            let callback3: Listener =
                Closure::from_method(object(&Arc::new(TestEventListener::default())), "__invoke")
                    .into();

            assert!(!callback1.identical(&callback2));
            assert!(!callback1.identical(&callback3));
            assert!(!callback2.identical(&callback3));

            dispatcher.add_listener("foo", callback1.clone(), 3)?;
            dispatcher.add_listener("foo", callback2.clone(), 2)?;
            dispatcher.add_listener("foo", callback3.clone(), 1)?;

            assert_eq!(Some(3), dispatcher.get_listener_priority("foo", &callback1));
            assert_eq!(Some(3), dispatcher.get_listener_priority("foo", &callback2));

            dispatcher.remove_listener("foo", &callback1)?;

            assert_eq!(
                vec![("foo".to_owned(), vec![callback3])],
                dispatcher.get_all_listeners()
            );
            Ok(())
        }
    };
}

mod event_dispatcher_test {
    event_dispatcher_tests!(EventDispatcher::new());
}

mod child_event_dispatcher_test {
    event_dispatcher_tests!(EventDispatcher::with_call_listeners(ChildEventDispatcher));
}

// Behaviour beyond the upstream tests, from tests/data/edges.txt.

fn both() -> [EventDispatcher; 2] {
    [
        EventDispatcher::new(),
        EventDispatcher::with_call_listeners(ChildEventDispatcher),
    ]
}

#[test]
fn anonymous_closures_are_equal_only_to_themselves() -> Result<(), Throwable> {
    for dispatcher in both() {
        dispatcher.add_listener("x", noop(), 0)?;
        dispatcher.remove_listener("x", &noop())?;
        assert!(dispatcher.has_listeners(Some("x")));
    }
    Ok(())
}

#[test]
fn closures_of_the_same_method_are_equal() -> Result<(), Throwable> {
    let o = object(&Arc::new(TestEventListener::default()));
    let p = object(&Arc::new(TestEventListener::default()));
    let dispatcher = EventDispatcher::new();
    dispatcher.add_listener("x", Closure::from_method(o.clone(), "__invoke").into(), 0)?;
    // `$o(...) == $o->__invoke(...)`, but not `$p(...)` or `$o->preFoo(...)`.
    let same = Closure::from_method(o.clone(), "__INVOKE").into();
    let other_object = Closure::from_method(p, "__invoke").into();
    let other_method = Closure::from_method(o, "preFoo").into();
    assert_eq!(None, dispatcher.get_listener_priority("x", &other_object));
    assert_eq!(None, dispatcher.get_listener_priority("x", &other_method));
    assert_eq!(Some(0), dispatcher.get_listener_priority("x", &same));
    Ok(())
}

#[test]
fn events_are_listed_in_first_sorted_order() -> Result<(), Throwable> {
    for dispatcher in both() {
        let o = invokable(&Arc::new(TestEventListener::default()));
        let p = invokable(&Arc::new(TestEventListener::default()));
        dispatcher.add_listener("pre", o.clone(), 0)?;
        dispatcher.add_listener("post", o, 0)?;
        dispatcher.get_listeners("post");
        let names = |d: &EventDispatcher| -> Vec<String> {
            d.get_all_listeners()
                .into_iter()
                .map(|(name, _)| name)
                .collect()
        };
        assert_eq!(names(&dispatcher), ["post", "pre"]);
        dispatcher.add_listener("pre", p, 0)?;
        assert_eq!(names(&dispatcher), ["post", "pre"]);
    }
    Ok(())
}

#[test]
fn priority_lookup_scans_buckets_in_insertion_order_until_sorted() -> Result<(), Throwable> {
    for dispatcher in both() {
        let o = invokable(&Arc::new(TestEventListener::default()));
        dispatcher.add_listener("e", o.clone(), -5)?;
        dispatcher.add_listener("e", o.clone(), 7)?;
        assert_eq!(Some(-5), dispatcher.get_listener_priority("e", &o));
        dispatcher.get_listeners("e");
        assert_eq!(Some(7), dispatcher.get_listener_priority("e", &o));
        // removeListener() removes every match, at every priority.
        dispatcher.remove_listener("e", &o)?;
        assert!(!dispatcher.has_listeners(Some("e")));
    }
    Ok(())
}

#[test]
fn the_default_event_name_is_the_event_class() -> Result<(), Throwable> {
    for dispatcher in both() {
        let seen = Arc::new(Mutex::new(String::new()));
        let record = seen.clone();
        dispatcher.add_listener(
            Event::CLASS,
            Listener::closure(move |_, name, _| {
                *record.lock().unwrap() = name.to_owned();
                Ok(())
            }),
            0,
        )?;
        dispatcher.dispatch(&mut Event::new(), None)?;
        assert_eq!(
            *seen.lock().unwrap(),
            "Symfony\\Contracts\\EventDispatcher\\Event"
        );
    }
    Ok(())
}

struct StdClass;
impl EventObject for StdClass {
    fn class(&self) -> &'static str {
        "stdClass"
    }
}

#[test]
fn an_event_without_propagation_control_reaches_every_listener() -> Result<(), Throwable> {
    for dispatcher in both() {
        let count = Arc::new(AtomicUsize::new(0));
        for _ in 0..2 {
            let count = count.clone();
            dispatcher.add_listener(
                "s",
                Listener::closure(move |_, _, _| {
                    count.fetch_add(1, SeqCst);
                    Ok(())
                }),
                0,
            )?;
        }
        dispatcher.dispatch(&mut StdClass, Some("s"))?;
        assert_eq!(count.load(SeqCst), 2);
    }
    Ok(())
}

#[test]
fn a_listener_exception_ends_the_dispatch() -> Result<(), Throwable> {
    for dispatcher in both() {
        let after = Arc::new(AtomicBool::new(false));
        dispatcher.add_listener("t", Listener::closure(|_, _, _| Err("boom".into())), 1)?;
        let flag = after.clone();
        dispatcher.add_listener(
            "t",
            Listener::closure(move |_, _, _| {
                flag.store(true, SeqCst);
                Ok(())
            }),
            0,
        )?;
        let error = dispatcher.dispatch(&mut Event::new(), Some("t")).err();
        assert_eq!(error.map(|e| e.to_string()), Some("boom".to_owned()));
        assert!(!after.load(SeqCst));
    }
    Ok(())
}

#[test]
fn an_undefined_method_is_an_error() -> Result<(), Throwable> {
    for dispatcher in both() {
        dispatcher.add_listener(
            "u",
            Listener::method(&object(&Arc::new(CallableClass)), "nope"),
            0,
        )?;
        let error = dispatcher.dispatch(&mut Event::new(), Some("u")).err();
        assert_eq!(
            error.map(|e| e.to_string()),
            Some("Call to undefined method CallableClass::nope()".to_owned())
        );
    }
    Ok(())
}

#[test]
fn a_dispatch_calls_the_listeners_of_its_start() -> Result<(), Throwable> {
    for dispatcher in both() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let entry = |name: &'static str| {
            let log = log.clone();
            Listener::closure(move |_, _, _| {
                log.lock().unwrap().push(name);
                Ok(())
            })
        };
        let added = entry("added");
        let first = {
            let log = log.clone();
            Listener::closure(move |_, _, dispatcher| {
                log.lock().unwrap().push("first");
                Ok(dispatcher.add_listener("r", added.clone(), 0)?)
            })
        };
        dispatcher.add_listener("r", first, 0)?;
        dispatcher.add_listener("r", entry("second"), 0)?;
        dispatcher.dispatch(&mut Event::new(), Some("r"))?;
        assert_eq!(*log.lock().unwrap(), ["first", "second"]);
        log.lock().unwrap().clear();
        dispatcher.dispatch(&mut Event::new(), Some("r"))?;
        assert_eq!(*log.lock().unwrap(), ["first", "second", "added"]);

        let later = entry("later");
        let remover = {
            let (log, later) = (log.clone(), later.clone());
            Listener::closure(move |_, _, dispatcher| {
                log.lock().unwrap().push("first");
                Ok(dispatcher.remove_listener("q", &later)?)
            })
        };
        dispatcher.add_listener("q", remover, 0)?;
        dispatcher.add_listener("q", later, 0)?;
        log.lock().unwrap().clear();
        dispatcher.dispatch(&mut Event::new(), Some("q"))?;
        assert_eq!(*log.lock().unwrap(), ["first", "later"]);
    }
    Ok(())
}
