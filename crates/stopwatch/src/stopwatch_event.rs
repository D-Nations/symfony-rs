//! Port of Symfony\Component\Stopwatch\StopwatchEvent.php.
//!
//! A `StopwatchEvent` is a shared handle, as a PHP object is: the event
//! returned by `start()` is the one `getEvent()` returns later, and either
//! can stop it. `same()` is PHP's `===`; `==` compares contents, as
//! PHPUnit's `assertEquals` does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/stopwatch StopwatchEvent.php

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    LogicException,
    php::{Decimal, Fixed2, Host, Number, round},
    stopwatch_period::StopwatchPeriod,
};

#[derive(Clone, PartialEq)]
struct State {
    periods: Vec<StopwatchPeriod>,
    origin: f64,
    category: String,
    started: Vec<f64>,
    more_precision: bool,
    name: String,
}

#[derive(Clone)]
pub struct StopwatchEvent {
    state: Arc<Mutex<State>>,
    host: Arc<dyn Host>,
}

impl StopwatchEvent {
    /// `new StopwatchEvent($origin, $category, $morePrecision, $name)`:
    /// `origin` in milliseconds; no category means `default`, no name
    /// means `default`.
    pub fn new(
        host: Arc<dyn Host>,
        origin: f64,
        category: Option<&str>,
        more_precision: bool,
        name: Option<&str>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                periods: Vec::new(),
                origin: format_time(origin),
                category: category.unwrap_or("default").to_owned(),
                started: Vec::new(),
                more_precision,
                name: name.unwrap_or("default").to_owned(),
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

    pub fn get_category(&self) -> String {
        self.lock().category.clone()
    }

    /// The origin in milliseconds.
    pub fn get_origin(&self) -> f64 {
        self.lock().origin
    }

    /// Starts a new event period.
    pub fn start(&self) -> &Self {
        let mut state = self.lock();
        let now = self.now(&state);
        state.started.push(now);
        self
    }

    /// Stops the last started event period.
    pub fn stop(&self) -> Result<&Self, LogicException> {
        let mut state = self.lock();
        let Some(start) = state.started.pop() else {
            return Err(LogicException::new(
                "stop() called but start() has not been called before.",
            ));
        };
        let period = self.period(&state, start);
        state.periods.push(period);
        Ok(self)
    }

    pub fn is_started(&self) -> bool {
        !self.lock().started.is_empty()
    }

    /// Stops the current period and then starts a new one.
    pub fn lap(&self) -> Result<&Self, LogicException> {
        Ok(self.stop()?.start())
    }

    /// Stops all non already stopped periods.
    pub fn ensure_stopped(&self) {
        while self.stop().is_ok() {}
    }

    pub fn get_periods(&self) -> Vec<StopwatchPeriod> {
        self.lock().periods.clone()
    }

    pub fn get_last_period(&self) -> Option<StopwatchPeriod> {
        self.lock().periods.last().cloned()
    }

    /// The relative time of the start of the first period in milliseconds.
    pub fn get_start_time(&self) -> Number {
        let state = self.lock();
        if let Some(period) = state.periods.first() {
            return period.get_start_time();
        }
        if let Some(&started) = state.started.first() {
            return Number::Float(started);
        }
        Number::Int(0)
    }

    /// The relative time of the end of the last period in milliseconds.
    pub fn get_end_time(&self) -> Number {
        self.lock()
            .periods
            .last()
            .map_or(Number::Int(0), StopwatchPeriod::get_end_time)
    }

    /// The duration of the event in milliseconds, including all periods;
    /// periods still running count until now.
    pub fn get_duration(&self) -> Number {
        let state = self.lock();
        let running = state
            .started
            .iter()
            .rev()
            .map(|&start| self.period(&state, start));
        state
            .periods
            .iter()
            .cloned()
            .chain(running.collect::<Vec<_>>())
            .fold(Number::Int(0), |total, period| {
                total + period.get_duration()
            })
    }

    /// The max memory usage of all periods in bytes.
    pub fn get_memory(&self) -> i64 {
        self.lock()
            .periods
            .iter()
            .map(StopwatchPeriod::get_memory)
            .fold(0, i64::max)
    }

    pub fn get_name(&self) -> String {
        self.lock().name.clone()
    }

    /// `getNow()`: the current time relative to the origin in milliseconds.
    fn now(&self, state: &State) -> f64 {
        format_time(self.host.microtime() * 1000.0 - state.origin)
    }

    fn period(&self, state: &State, start: f64) -> StopwatchPeriod {
        let end = self.now(state);
        StopwatchPeriod::new(
            Number::Float(start),
            Number::Float(end),
            state.more_precision,
            self.host.memory_get_usage(),
        )
    }
}

/// `formatTime()`.
fn format_time(time: f64) -> f64 {
    round(time, 1)
}

impl PartialEq for StopwatchEvent {
    fn eq(&self, other: &Self) -> bool {
        if self.same(other) {
            return true;
        }
        // Copy one side first: never hold two event locks at once.
        let mine = self.lock().clone();
        mine == *other.lock()
    }
}

impl fmt::Debug for StopwatchEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.lock();
        f.debug_struct("StopwatchEvent")
            .field("name", &state.name)
            .field("category", &state.category)
            .field("origin", &state.origin)
            .field("started", &state.started)
            .field("periods", &state.periods)
            .finish()
    }
}

/// `__toString()`: `category/name: 0.00 MiB - 0 ms`.
impl fmt::Display for StopwatchEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let memory = self.get_memory();
        let duration = self.get_duration();
        let state = self.lock();
        write!(
            f,
            "{}/{}: {} MiB - {} ms",
            state.category,
            state.name,
            Fixed2(Number::Int(memory).as_f64() / 1024.0 / 1024.0),
            Decimal(duration)
        )
    }
}
