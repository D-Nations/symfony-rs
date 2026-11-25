//! `CompiledEventDispatcher` against the ported `EventDispatcher` given
//! the same `addListener()` calls: generated scripts of dispatches, reads
//! and changes, some made by listeners during a dispatch, compared call by
//! call (20,000 scripts). The plain dispatcher's behaviour is the pinned PHP's (its own
//! tests and recorded vectors); this checks the compiled one matches it.
//!
//! Upstream: none

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use symfony_event_dispatcher::{
    Callable, Closure, CompiledEventDispatcher, Event, EventDispatcher, EventDispatcherInterface,
    EventObject, EventSubscriber, Factory, Listener, ListenerMap, ListenerPipeline, Subscription,
    Throwable, UndefinedMethod,
};

const EVENTS: [&str; 4] = ["e1", "e2", "e3", Event::CLASS];
const CLASSES: [&str; 3] = ["O0", "O1", "O2"];
const METHODS: [&str; 3] = ["a", "b", "__invoke"];
const PRIORITIES: [i64; 6] = [-1, 0, 0, 0, 1, 5];
/// The closure listeners add during a dispatch.
const ADDED: usize = 3;

/// `xorshift64*`, so the scripts are the same on every run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32;
        usize::try_from(x).unwrap_or(0) % n
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

#[derive(Clone, Copy, Debug)]
enum Spec {
    Method(usize, usize),
    Invokable(usize),
    Closure(usize),
    /// `$object->method(...)`, made anew each time.
    Bound(usize, usize),
    /// `[$factory, 'method']`; factory `i` returns object `i`.
    Lazy(usize, usize),
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Log,
    Stop,
    Throw,
    Add(usize, i64),
    Remove(Spec, usize),
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Dispatch(usize),
    DispatchUnnamed,
    Get(usize),
    GetAll,
    Priority(usize, Spec),
    Has(Option<usize>),
    Add(usize, Spec, i64),
    Remove(usize, Spec),
    Subscribe(usize),
    Unsubscribe(usize),
    Table,
}

#[derive(Debug)]
struct Script {
    object_actions: [[Action; 3]; 3],
    closure_actions: [Action; 4],
    registrations: Vec<(usize, Spec, i64)>,
    subscribed: Option<usize>,
    ops: Vec<Op>,
}

fn spec(rng: &mut Rng) -> Spec {
    match rng.below(5) {
        0 => Spec::Method(rng.below(3), rng.below(3)),
        1 => Spec::Invokable(rng.below(3)),
        2 => Spec::Closure(rng.below(3)),
        3 => Spec::Bound(rng.below(3), rng.below(3)),
        _ => Spec::Lazy(rng.below(3), rng.below(3)),
    }
}

fn action(rng: &mut Rng) -> Action {
    match rng.below(12) {
        0 => Action::Stop,
        1 => Action::Throw,
        2 => Action::Add(rng.below(EVENTS.len()), rng.pick(&PRIORITIES)),
        3 => Action::Remove(spec(rng), rng.below(EVENTS.len())),
        _ => Action::Log,
    }
}

fn script(rng: &mut Rng) -> Script {
    let object_actions = [(); 3].map(|()| [(); 3].map(|()| action(rng)));
    let mut closure_actions = [(); 4].map(|()| action(rng));
    // The added closure only logs, so adding cannot loop.
    closure_actions[ADDED] = Action::Log;
    let registrations = (0..rng.below(9))
        .map(|_| (rng.below(EVENTS.len()), spec(rng), rng.pick(&PRIORITIES)))
        .collect();
    let subscribed = (rng.below(3) == 0).then(|| rng.below(3));
    let ops = (0..=rng.below(10))
        .map(|_| match rng.below(16) {
            0..=5 => Op::Dispatch(rng.below(EVENTS.len())),
            6 => Op::DispatchUnnamed,
            7 => Op::Get(rng.below(EVENTS.len())),
            8 => Op::GetAll,
            9 => Op::Priority(rng.below(EVENTS.len()), spec(rng)),
            10 => Op::Has((rng.below(2) == 0).then(|| rng.below(EVENTS.len()))),
            11 => Op::Add(rng.below(EVENTS.len()), spec(rng), rng.pick(&PRIORITIES)),
            12 => Op::Remove(rng.below(EVENTS.len()), spec(rng)),
            13 => Op::Subscribe(rng.below(3)),
            14 => Op::Unsubscribe(rng.below(3)),
            _ => Op::Table,
        })
        .collect();
    Script {
        object_actions,
        closure_actions,
        registrations,
        subscribed,
        ops,
    }
}

/// What the listeners of one run share.
#[derive(Default)]
struct World {
    log: Mutex<Vec<String>>,
    objects: OnceLock<Vec<Arc<Recorder>>>,
    closures: OnceLock<Vec<Closure>>,
    factories: OnceLock<Vec<Factory>>,
    object_actions: OnceLock<[[Action; 3]; 3]>,
}

impl World {
    fn push(&self, line: String) {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line);
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn object(&self, i: usize) -> Arc<dyn Callable> {
        self.objects.get().map_or_else(
            || unreachable_object(),
            |objects| objects[i].clone() as Arc<dyn Callable>,
        )
    }

    fn listener(&self, spec: Spec) -> Listener {
        match spec {
            Spec::Method(o, m) => Listener::Method(self.object(o), METHODS[m]),
            Spec::Invokable(o) => Listener::Invokable(self.object(o)),
            Spec::Closure(c) => Listener::Closure(self.closures.get().map_or_else(
                || Closure::new(|_, _, _| Ok(())),
                |closures| closures[c].clone(),
            )),
            Spec::Bound(o, m) => {
                Listener::Closure(Closure::from_method(self.object(o), METHODS[m]))
            }
            Spec::Lazy(f, m) => Listener::Lazy(
                self.factories
                    .get()
                    .map_or_else(|| Factory::new(unreachable_object), |f2| f2[f].clone()),
                METHODS[m],
            ),
        }
    }

    /// A listener as the trace shows it, the same in both runs.
    fn describe(&self, listener: &Listener) -> String {
        match listener {
            Listener::Closure(closure) => {
                let index = self.closures.get().and_then(|closures| {
                    closures.iter().position(|c| {
                        Listener::Closure(c.clone()).identical(&Listener::Closure(closure.clone()))
                    })
                });
                index.map_or_else(|| format!("{listener:?}"), |i| format!("closure{i}"))
            }
            Listener::Lazy(_, method) => {
                let index = self.factories.get().and_then(|factories| {
                    factories
                        .iter()
                        .position(|f| Listener::Lazy(f.clone(), method).identical(listener))
                });
                format!("lazy{index:?}:{method}")
            }
            other => format!("{other:?}"),
        }
    }

    /// What a listener does when called.
    fn act(
        &self,
        who: &str,
        action: Action,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        self.push(format!("{who}@{event_name}"));
        match action {
            Action::Log => {}
            Action::Stop => {
                if let Some(event) = event.downcast_mut::<Event>() {
                    event.stop_propagation();
                }
            }
            Action::Throw => return Err(format!("{who} threw").into()),
            Action::Add(e, priority) => {
                let added = self.listener(Spec::Closure(ADDED));
                dispatcher.add_listener(EVENTS[e], added, priority)?;
            }
            Action::Remove(spec, e) => {
                dispatcher.remove_listener(EVENTS[e], &self.listener(spec))?;
            }
        }
        Ok(())
    }
}

fn unreachable_object() -> Arc<dyn Callable> {
    Arc::new(Recorder {
        index: 0,
        world: Arc::default(),
    })
}

struct Recorder {
    index: usize,
    world: Arc<World>,
}

impl Callable for Recorder {
    fn class(&self) -> &'static str {
        CLASSES[self.index]
    }

    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        let Some(m) = METHODS.iter().position(|&known| known == method) else {
            return Err(UndefinedMethod::error(self.class(), method));
        };
        let action = self
            .world
            .object_actions
            .get()
            .map_or(Action::Log, |actions| actions[self.index][m]);
        let who = format!("{}::{method}", self.class());
        self.world.act(&who, action, event, event_name, dispatcher)
    }
}

impl EventSubscriber for Recorder {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        vec![
            (EVENTS[self.index], Subscription::Method("a")),
            (
                EVENTS[(self.index + 1) % EVENTS.len()],
                Subscription::Methods(vec![("b", 1), ("a", -1)]),
            ),
        ]
    }
}

fn world(script: &Script) -> Arc<World> {
    let world = Arc::new(World::default());
    let objects = (0..3)
        .map(|index| {
            Arc::new(Recorder {
                index,
                world: world.clone(),
            })
        })
        .collect();
    let _ = world.objects.set(objects);
    let closures = (0..4)
        .map(|i| {
            let w = Arc::downgrade(&world);
            let action = script.closure_actions[i];
            Closure::new(move |event, name, dispatcher| match w.upgrade() {
                Some(w) => w.act(&format!("closure{i}"), action, event, name, dispatcher),
                None => Ok(()),
            })
        })
        .collect();
    let _ = world.closures.set(closures);
    let factories = (0..3)
        .map(|i| {
            let w = Arc::downgrade(&world);
            Factory::new(move || match w.upgrade() {
                Some(w) => {
                    w.push(format!("factory{i}"));
                    w.object(i)
                }
                None => unreachable_object(),
            })
        })
        .collect();
    let _ = world.factories.set(factories);
    let _ = world.object_actions.set(script.object_actions);
    world
}

fn subscriber(world: &World, i: usize) -> Arc<dyn EventSubscriber> {
    world.objects.get().map_or_else(
        || {
            Arc::new(Recorder {
                index: 0,
                world: Arc::default(),
            })
        },
        |o| o[i].clone(),
    )
}

fn result(result: Result<(), impl std::fmt::Display>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(e) => format!("error: {e}"),
    }
}

/// Runs the ops, returning one line per op and what listeners logged.
fn run(world: &World, ops: &[Op], dispatcher: &dyn EventDispatcherInterface) -> Vec<String> {
    let mut trace = Vec::new();
    for op in ops {
        let line = match *op {
            Op::Dispatch(e) => {
                let mut event = Event::new();
                let r = dispatcher.dispatch(&mut event, Some(EVENTS[e]));
                format!("{} stopped={}", result(r), event.is_propagation_stopped())
            }
            Op::DispatchUnnamed => result(dispatcher.dispatch(&mut Event::new(), None)),
            Op::Get(e) => {
                let listeners = dispatcher.get_listeners(EVENTS[e]);
                listeners
                    .iter()
                    .map(|l| world.describe(l))
                    .collect::<Vec<_>>()
                    .join(",")
            }
            Op::GetAll => dispatcher
                .get_all_listeners()
                .iter()
                .map(|(name, listeners)| {
                    let names: Vec<String> = listeners.iter().map(|l| world.describe(l)).collect();
                    format!("{name}=[{}]", names.join(","))
                })
                .collect::<Vec<_>>()
                .join(";"),
            Op::Priority(e, spec) => format!(
                "{:?}",
                dispatcher.get_listener_priority(EVENTS[e], &world.listener(spec))
            ),
            Op::Has(e) => format!("{}", dispatcher.has_listeners(e.map(|e| EVENTS[e]))),
            Op::Add(e, spec, priority) => {
                result(dispatcher.add_listener(EVENTS[e], world.listener(spec), priority))
            }
            Op::Remove(e, spec) => {
                result(dispatcher.remove_listener(EVENTS[e], &world.listener(spec)))
            }
            Op::Subscribe(i) => result(dispatcher.add_subscriber(subscriber(world, i))),
            Op::Unsubscribe(i) => result(dispatcher.remove_subscriber(&subscriber(world, i))),
            Op::Table => dispatcher.as_event_dispatcher().map_or_else(
                || "none".to_owned(),
                |plain| {
                    plain
                        .get_listeners_with_priority()
                        .iter()
                        .map(|(name, listeners)| {
                            let names: Vec<String> = listeners
                                .iter()
                                .map(|(l, p)| format!("{}:{p}", world.describe(l)))
                                .collect();
                            format!("{name}=[{}]", names.join(","))
                        })
                        .collect::<Vec<_>>()
                        .join(";")
                },
            ),
        };
        trace.push(format!("{op:?} -> {line} | {}", world.take().join(" ")));
    }
    trace
}

fn plain(world: &World, script: &Script) -> Result<EventDispatcher, Throwable> {
    let dispatcher = EventDispatcher::new();
    if let Some(i) = script.subscribed {
        dispatcher.add_subscriber(subscriber(world, i))?;
    }
    for &(e, spec, priority) in &script.registrations {
        dispatcher.add_listener(EVENTS[e], world.listener(spec), priority)?;
    }
    Ok(dispatcher)
}

fn compiled(world: &World, script: &Script) -> CompiledEventDispatcher {
    let mut map = ListenerMap::new();
    if let Some(i) = script.subscribed {
        map.add_subscriber(&subscriber(world, i));
    }
    for &(e, spec, priority) in &script.registrations {
        map.add_listener(EVENTS[e], world.listener(spec), priority);
    }
    CompiledEventDispatcher::from(map)
}

#[test]
fn compiled_dispatcher_runs_as_the_plain_one() -> Result<(), Throwable> {
    let mut rng = Rng(0x2026_1126);
    let mut failures = Vec::new();
    let mut compiled_dispatches = 0;
    for n in 0..20_000 {
        let script = script(&mut rng);

        let world_a = world(&script);
        let expected = run(&world_a, &script.ops, &plain(&world_a, &script)?);

        let world_b = world(&script);
        let dispatcher = compiled(&world_b, &script);
        let mut actual = Vec::new();
        for op in &script.ops {
            if !dispatcher.is_thawed() && matches!(op, Op::Dispatch(_) | Op::DispatchUnnamed) {
                compiled_dispatches += 1;
            }
            actual.extend(run(&world_b, std::slice::from_ref(op), &dispatcher));
        }

        if expected != actual {
            failures.push(format!(
                "script {n}: {script:?}\n  expected {expected:#?}\n  actual {actual:#?}"
            ));
        }
    }
    let total = failures.len();
    failures.truncate(3);
    assert!(
        failures.is_empty(),
        "{total} differences:\n{}",
        failures.join("\n")
    );
    assert!(
        compiled_dispatches > 10_000,
        "{compiled_dispatches} compiled dispatches"
    );
    Ok(())
}

/// A small static fixture for the seam: a pipeline that calls its two
/// listeners directly, keeping a [`ListenerMap`] of the same listeners for
/// the thaw, against the plain dispatcher.
struct Counter {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

impl Counter {
    fn on_event(&self, event: &mut Event, event_name: &str) {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("{}@{event_name}", self.name));
        if self.name == "stopper" {
            event.stop_propagation();
        }
    }
}

impl Callable for Counter {
    fn class(&self) -> &'static str {
        self.name
    }

    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        event_name: &str,
        _dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        if method != "onEvent" {
            return Err(UndefinedMethod::error(self.name, method));
        }
        let event = event.downcast_mut::<Event>().ok_or("not an Event")?;
        self.on_event(event, event_name);
        Ok(())
    }
}

struct TypedPipeline {
    first: Arc<Counter>,
    stopper: Arc<Counter>,
    last: Arc<Counter>,
    map: ListenerMap,
}

impl ListenerPipeline for TypedPipeline {
    fn has_listeners(&self, event_name: Option<&str>) -> bool {
        self.map.has_listeners(event_name)
    }

    fn dispatch(
        &self,
        event_name: &str,
        event: &mut dyn EventObject,
        _dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        if !self.map.note_dispatch(event_name) {
            return Ok(());
        }
        let event = event.downcast_mut::<Event>().ok_or("not an Event")?;
        // `go` at 10, then `go` and `stop` at 0 in registration order.
        match event_name {
            "go" => {
                for counter in [&self.first, &self.last] {
                    if event.is_propagation_stopped() {
                        break;
                    }
                    counter.on_event(event, event_name);
                }
            }
            "stop" => {
                for counter in [&self.stopper, &self.last] {
                    if event.is_propagation_stopped() {
                        break;
                    }
                    counter.on_event(event, event_name);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn thaw(&self) -> EventDispatcher {
        self.map.thaw()
    }
}

#[test]
fn a_typed_pipeline_runs_as_the_plain_dispatcher() -> Result<(), Throwable> {
    let registrations = |first: &Arc<Counter>, stopper: &Arc<Counter>, last: &Arc<Counter>| {
        let first: Arc<dyn Callable> = first.clone();
        let stopper: Arc<dyn Callable> = stopper.clone();
        let last: Arc<dyn Callable> = last.clone();
        vec![
            ("go", Listener::Method(last.clone(), "onEvent"), 0),
            ("go", Listener::Method(first, "onEvent"), 10),
            ("stop", Listener::Method(stopper, "onEvent"), 0),
            ("stop", Listener::Method(last, "onEvent"), 0),
        ]
    };
    let counters = |log: &Arc<Mutex<Vec<String>>>| {
        ["first", "stopper", "last"].map(|name| {
            Arc::new(Counter {
                name,
                log: log.clone(),
            })
        })
    };
    let script = |dispatcher: &dyn EventDispatcherInterface,
                  log: &Arc<Mutex<Vec<String>>>|
     -> Result<Vec<String>, Throwable> {
        for name in ["go", "stop", "none", "go"] {
            dispatcher.dispatch(&mut Event::new(), Some(name))?;
        }
        let mut seen = log.lock().unwrap_or_else(PoisonError::into_inner).clone();
        seen.push(format!("{:?}", dispatcher.get_listeners("go")));
        seen.push(format!("{}", dispatcher.has_listeners(Some("stop"))));
        Ok(seen)
    };

    let log = Arc::new(Mutex::new(Vec::new()));
    let [first, stopper, last] = counters(&log);
    let plain = EventDispatcher::new();
    for (name, listener, priority) in registrations(&first, &stopper, &last) {
        plain.add_listener(name, listener, priority)?;
    }
    let expected = script(&plain, &log)?;

    let log = Arc::new(Mutex::new(Vec::new()));
    let [first, stopper, last] = counters(&log);
    let mut map = ListenerMap::new();
    for (name, listener, priority) in registrations(&first, &stopper, &last) {
        map.add_listener(name, listener, priority);
    }
    let compiled = CompiledEventDispatcher::new(TypedPipeline {
        first,
        stopper,
        last,
        map,
    });
    let actual = script(&compiled, &log)?;

    assert_eq!(
        vec![
            "first@go",
            "last@go",
            "stopper@stop",
            "first@go",
            "last@go",
            "[[first, \"onEvent\"], [last, \"onEvent\"]]",
            "true",
        ],
        expected
    );
    assert_eq!(expected, actual);
    assert!(compiled.is_thawed());
    Ok(())
}
