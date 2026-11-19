//! Port of Symfony\Component\Stopwatch\Stopwatch.php.
//!
//! `reset()` is service-contracts' ResetInterface, which is not ported yet.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/stopwatch Stopwatch.php

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    LogicException,
    php::{Host, SystemHost},
    section::Section,
    stopwatch_event::StopwatchEvent,
};

pub const ROOT: &str = "__root__";

#[derive(Clone, PartialEq)]
struct State {
    /// `$this->sections`: the root, then stopped sections by id, in PHP
    /// array order (re-stopping an id keeps its place).
    sections: Vec<(String, Section)>,
    /// `$this->activeSections`: the root, then each open section.
    active: Vec<Section>,
}

/// Profiles code: named events, grouped in sections.
pub struct Stopwatch {
    more_precision: bool,
    host: Arc<dyn Host>,
    state: Mutex<State>,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Stopwatch {
    pub const ROOT: &'static str = ROOT;

    /// With `more_precision`, times keep their decimals (floats) instead
    /// of being truncated to whole milliseconds.
    pub fn new(more_precision: bool) -> Self {
        Self::with_host(Arc::new(SystemHost), more_precision)
    }

    pub fn with_host(host: Arc<dyn Host>, more_precision: bool) -> Self {
        let state = Self::fresh(&host, more_precision);
        Self {
            more_precision,
            host,
            state: Mutex::new(state),
        }
    }

    fn fresh(host: &Arc<dyn Host>, more_precision: bool) -> State {
        let root = Section::new(host.clone(), None, more_precision);
        State {
            sections: vec![(ROOT.to_owned(), root.clone())],
            active: vec![root],
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Nothing panics while this lock is held.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `end($this->activeSections)`.
    fn current(&self) -> Section {
        let state = self.lock();
        match state.active.last() {
            Some(section) => section.clone(),
            // `active` always holds the root; rebuild it rather than fail.
            None => Section::new(self.host.clone(), None, self.more_precision),
        }
    }

    pub fn get_sections(&self) -> Vec<(String, Section)> {
        self.lock().sections.clone()
    }

    /// Creates a new section, or re-opens the section `id` of the current
    /// level.
    pub fn open_section(&self, id: Option<&str>) -> Result<(), LogicException> {
        let current = self.current();
        if let Some(id) = id
            && current.get(id).is_none()
        {
            return Err(LogicException::new(format!(
                "The section \"{id}\" has been started at an other level and cannot be opened."
            )));
        }
        self.start("__section__.child", Some("section"));
        let section = current.open(id);
        self.lock().active.push(section);
        self.start("__section__", None);
        Ok(())
    }

    /// Stops the last started section; its events are then read with
    /// `get_section_events(id)`.
    pub fn stop_section(&self, id: &str) -> Result<(), LogicException> {
        self.stop("__section__")?;
        let section = {
            let mut state = self.lock();
            if state.active.len() == 1 {
                return Err(LogicException::new("There is no started section to stop."));
            }
            let Some(section) = state.active.pop() else {
                return Err(LogicException::new("There is no started section to stop."));
            };
            section.set_id(id);
            match state.sections.iter_mut().find(|(key, _)| key == id) {
                Some(entry) => entry.1 = section.clone(),
                None => state.sections.push((id.to_owned(), section.clone())),
            }
            section
        };
        drop(section);
        self.stop("__section__.child")?;
        Ok(())
    }

    /// Starts an event.
    pub fn start(&self, name: &str, category: Option<&str>) -> StopwatchEvent {
        self.current().start_event(name, category)
    }

    pub fn is_started(&self, name: &str) -> bool {
        self.current().is_event_started(name)
    }

    pub fn stop(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        self.current().stop_event(name)
    }

    /// Stops then restarts an event.
    pub fn lap(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        self.current().lap(name)
    }

    pub fn get_event(&self, name: &str) -> Result<StopwatchEvent, LogicException> {
        self.current().get_event(name)
    }

    /// The events of a stopped section; none for an unknown id.
    pub fn get_section_events(&self, id: &str) -> Vec<(String, StopwatchEvent)> {
        let section = self
            .lock()
            .sections
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, section)| section.clone());
        section
            .map(|section| section.get_events())
            .unwrap_or_default()
    }

    pub fn get_root_section_events(&self) -> Vec<(String, StopwatchEvent)> {
        self.get_section_events(ROOT)
    }

    /// Resets the stopwatch to its original state.
    pub fn reset(&self) {
        let fresh = Self::fresh(&self.host, self.more_precision);
        *self.lock() = fresh;
    }
}

/// PHPUnit's `assertEquals`: same contents.
impl PartialEq for Stopwatch {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        let mine = self.lock().clone();
        let theirs = other.lock().clone();
        self.more_precision == other.more_precision && mine == theirs
    }
}

impl fmt::Debug for Stopwatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.lock().clone();
        f.debug_struct("Stopwatch")
            .field("more_precision", &self.more_precision)
            .field("sections", &state.sections)
            .field("active", &state.active)
            .finish()
    }
}
