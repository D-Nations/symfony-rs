//! Port of Symfony\Component\EventDispatcher\GenericEvent.php.
//!
//! PHP's `mixed $subject` and `array $arguments` become the type
//! parameters `S` and `V`. Arguments keep PHP array order: a new key goes
//! last, an existing key keeps its place. `ArrayAccess` is the `offset_*`
//! methods; `offsetGet` of a missing key throws, so it cannot be `Index`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher GenericEvent.php

use std::fmt;

use symfony_event_dispatcher_contracts::{Event, EventObject};

/// PHP's `\InvalidArgumentException` from `getArgument()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidArgumentException {
    pub message: String,
}

impl fmt::Display for InvalidArgumentException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InvalidArgumentException {}

/// Encapsulates an event's subject and arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericEvent<S, V> {
    event: Event,
    subject: S,
    arguments: Vec<(String, V)>,
}

impl<S, V> GenericEvent<S, V> {
    pub const CLASS: &'static str = "Symfony\\Component\\EventDispatcher\\GenericEvent";

    pub fn new(subject: S, arguments: impl IntoIterator<Item = (String, V)>) -> Self {
        let mut event = Self {
            event: Event::new(),
            subject,
            arguments: Vec::new(),
        };
        event.set_arguments(arguments);
        event
    }

    pub fn get_subject(&self) -> &S {
        &self.subject
    }

    pub fn get_argument(&self, key: &str) -> Result<&V, InvalidArgumentException> {
        self.position(key)
            .map(|i| &self.arguments[i].1)
            .ok_or_else(|| InvalidArgumentException {
                message: format!("Argument \"{key}\" not found."),
            })
    }

    pub fn set_argument(&mut self, key: impl Into<String>, value: V) -> &mut Self {
        let key = key.into();
        match self.position(&key) {
            Some(i) => self.arguments[i].1 = value,
            None => self.arguments.push((key, value)),
        }
        self
    }

    pub fn get_arguments(&self) -> &[(String, V)] {
        &self.arguments
    }

    /// Replaces all arguments; a repeated key keeps its first place and
    /// its last value, as in a PHP array literal.
    pub fn set_arguments(&mut self, arguments: impl IntoIterator<Item = (String, V)>) -> &mut Self {
        self.arguments.clear();
        for (key, value) in arguments {
            self.set_argument(key, value);
        }
        self
    }

    pub fn has_argument(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    pub fn offset_get(&self, key: &str) -> Result<&V, InvalidArgumentException> {
        self.get_argument(key)
    }

    pub fn offset_set(&mut self, key: impl Into<String>, value: V) {
        self.set_argument(key, value);
    }

    pub fn offset_unset(&mut self, key: &str) {
        if let Some(i) = self.position(key) {
            self.arguments.remove(i);
        }
    }

    pub fn offset_exists(&self, key: &str) -> bool {
        self.has_argument(key)
    }

    /// `IteratorAggregate`: the arguments in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.arguments
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    pub fn stop_propagation(&mut self) {
        self.event.stop_propagation();
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.arguments.iter().position(|(k, _)| k == key)
    }
}

impl<'a, S, V> IntoIterator for &'a GenericEvent<S, V> {
    type Item = (&'a str, &'a V);
    type IntoIter = Box<dyn Iterator<Item = (&'a str, &'a V)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<S: 'static, V: 'static> EventObject for GenericEvent<S, V> {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn is_propagation_stopped(&self) -> bool {
        self.event.is_propagation_stopped()
    }
}
