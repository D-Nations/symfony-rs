//! Port of Symfony\Component\Stopwatch\Section.php.
//!
//! A `Section` is a shared handle, as a PHP object is: the same section is
//! a child of its parent, an active section, and a stopped section by id.
//! Events keep the order in which they were first started.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/stopwatch Section.php

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{LogicException, php::Host, stopwatch_event::StopwatchEvent};

#[derive(Clone, PartialEq)]
struct State {
    events: Vec<(String, StopwatchEvent)>,
    id: Option<String>,
    children: Vec<Section>,
    origin: Option<f64>,
    more_precision: bool,
}

#[derive(Clone)]
pub struct Section {
    state: Arc<Mutex<State>>,
    host: Arc<dyn Host>,
}

impl Section {
    /// `origin` (milliseconds) is the origin of this section's events;
    /// `None` gives each event its own start time.
    pub fn new(host: Arc<dyn Host>, origin: Option<f64>, more_precision: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                events: Vec::new(),
                id: None,
                children: Vec::new(),
                origin,
                more_precision,
            })),
            host,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Nothing panics while this lock is held.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// PHP `===`.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    /// The child section with this id.
    pub fn get(&self, id: &str) -> Option<Self> {
        let children = self.lock().children.clone();
        children
            .into_iter()
            .find(|child| child.get_id().as_deref() == Some(id))
    }

    /// Creates a child section, or re-opens the child with this id.
    #[must_use]
    pub fn open(&self, id: Option<&str>) -> Self {
        if let Some(section) = id.and_then(|id| self.get(id)) {
            return section;
        }
        let mut state = self.lock();
        let section = Self::new(
            self.host.clone(),
            Some(self.host.microtime() * 1000.0),
            state.more_precision,
        );
        state.children.push(section.clone());
        section
    }

    pub fn get_id(&self) -> Option<String> {
        self.lock().id.clone()
    }

    pub fn set_id(&self, id: &str) -> &Self {
        self.lock().id = Some(id.to_owned());
        self
    }

    /// Starts an event, creating it on first use.
    pub fn start_event(&self, name: &str, category: Option<&str>) -> StopwatchEvent {
        let event = {
            let mut state = self.lock();
            if let Some((_, event)) = state.events.iter().find(|(n, _)| n == name) {
                event.clone()
            } else {
                // `$this->origin ?: microtime(true) * 1000`: 0.0 is falsy.
                let origin = match state.origin {
                    Some(origin) if origin != 0.0 => origin,
                    _ => self.host.microtime() * 1000.0,
                };
                let event = StopwatchEvent::new(
                    self.host.clone(),
                    origin,
                    category,
                    state.more_precision,
                    Some(name),
                );
                state.events.push((name.to_owned(), event.clone()));
                event
            }
        };
        event.start();
        event
    }

    pub fn is_event_started(&self, name: &str) -> bool {
        self.find(name).is_some_and(|event| event.is_started())
    }

    /// Stops an event; `Event "%s" is not started.` if it was never started.
    pub fn stop_event(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        let event = self
            .find(name)
            .ok_or_else(|| LogicException::new(format!("Event \"{name}\" is not started.")))?;
        event.stop()?;
        Ok(event)
    }

    /// Stops then restarts an event.
    pub fn lap(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        let event = self.stop_event(name)?;
        event.start();
        Ok(event)
    }

    /// `Event "%s" is not known.` for an event never started.
    pub fn get_event(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        self.find(name)
            .ok_or_else(|| LogicException::new(format!("Event \"{name}\" is not known.")))
    }

    pub fn get_events(&self) -> Vec<(String, StopwatchEvent)> {
        self.lock().events.clone()
    }

    fn find(&self, name: &str) -> Option<StopwatchEvent> {
        self.lock()
            .events
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, event)| event.clone())
    }
}

/// PHPUnit's `assertEquals`: same contents.
impl PartialEq for Section {
    fn eq(&self, other: &Self) -> bool {
        if self.same(other) {
            return true;
        }
        // Copy each side in turn: never hold two section locks at once.
        let mine = self.lock().clone();
        let theirs = other.lock().clone();
        mine == theirs
    }
}

impl fmt::Debug for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.lock().clone();
        f.debug_struct("Section")
            .field("id", &state.id)
            .field("origin", &state.origin)
            .field("events", &state.events)
            .field("children", &state.children)
            .finish()
    }
}
