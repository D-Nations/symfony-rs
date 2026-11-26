//! A PHP `\Generator` object: the pairs a generator function yields,
//! produced one at a time as `foreach` asks for them, for the
//! `StreamedJsonResponse` data that holds generators. Written from the PHP
//! manual and answers recorded from PHP 8.4.27
//! (`tests/data/streamed_json.json`); no PHP source code (PLAN.md
//! decision 8).
//!
//! The body is a Rust iterator, which runs only when the next pair is
//! asked for, as a generator's body runs up to its next `yield`: what it
//! echoes lands between the pairs it yields. An `Err` is what the body
//! threw; the generator is then closed.
//!
//! `foreach` over a generator first rewinds it: the body runs to its first
//! `yield`. A generator stopped there (the loop's body threw on the first
//! pair) gives that pair again. Otherwise PHP throws:
//!
//! - `\Exception` "Cannot traverse an already closed generator" once it
//!   has returned or thrown;
//! - `\Exception` "Cannot rewind a generator that was already run" once
//!   it went past its first `yield`;
//! - `\Error` "Cannot resume an already running generator" for a
//!   generator traversed from inside its own traversal.
//!
//! A generator has no properties (`json_encode()` writes `{}`) and is
//! `\Traversable` and `\Iterator`. [`PhpObject::traverse`] runs it to the
//! end as one `foreach` does. Its keys are given with each value:
//! [`Generator::values`] numbers them 0, 1, … as a `yield` without a key
//! does.
//!
//! Upstream: none

use std::{
    fmt,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{PhpArray, PhpObject, Thrown, Value};

/// What a generator's body yields next: a key and a value, or what it
/// threw.
pub type Yield = Result<(Value, Value), Thrown>;

/// A generator's body.
pub type Body = Box<dyn Iterator<Item = Yield> + Send>;

/// `Generator::class`.
pub const CLASS: &str = "Generator";

enum State {
    /// Not started.
    Fresh(Body),
    /// Stopped at its first `yield`.
    AtFirst(Body, (Value, Value)),
    /// Stopped at a later `yield`.
    Advanced(Body),
    /// Being traversed.
    Running,
    /// Returned or threw.
    Closed,
}

/// A `\Generator`.
pub struct Generator {
    state: Mutex<State>,
}

impl fmt::Debug for Generator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Generator")
    }
}

impl Generator {
    /// A generator whose body is `body`.
    pub fn new(body: impl Iterator<Item = Yield> + Send + 'static) -> Self {
        Self {
            state: Mutex::new(State::Fresh(Box::new(body))),
        }
    }

    /// A generator yielding `values` without keys: 0, 1, ….
    pub fn values(values: impl Iterator<Item = Result<Value, Thrown>> + Send + 'static) -> Self {
        Self::new(
            values
                .zip(0_i64..)
                .map(|(v, k)| v.map(|v| (Value::Int(k), v))),
        )
    }

    /// The generator as a PHP value.
    pub fn object(self) -> Value {
        Value::Object(Arc::new(self))
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, state: State) {
        *self.state() = state;
    }

    /// `foreach ($generator as $key => $value)`, with `body` as the loop's
    /// body. An error from either ends the loop and is returned (what the
    /// generator throws as `thrown` makes it); one from `body` leaves the
    /// generator stopped at the pair it was given.
    pub fn for_each<E>(
        &self,
        mut body: impl FnMut(Value, Value) -> Result<(), E>,
        thrown: impl Fn(Thrown) -> E,
    ) -> Result<(), E> {
        let (mut rest, current) = {
            let mut state = self.state();
            match std::mem::replace(&mut *state, State::Running) {
                State::Fresh(rest) => (rest, None),
                State::AtFirst(rest, pair) => (rest, Some(pair)),
                State::Advanced(rest) => {
                    *state = State::Advanced(rest);
                    return Err(thrown(Thrown::new(
                        "Exception",
                        "Cannot rewind a generator that was already run",
                        0,
                    )));
                }
                State::Running => {
                    return Err(thrown(Thrown::new(
                        "Error",
                        "Cannot resume an already running generator",
                        0,
                    )));
                }
                State::Closed => {
                    *state = State::Closed;
                    return Err(thrown(Thrown::new(
                        "Exception",
                        "Cannot traverse an already closed generator",
                        0,
                    )));
                }
            }
        };
        let mut current = match current {
            Some(pair) => pair,
            None => match self.resume(&mut rest) {
                Ok(Some(pair)) => pair,
                Ok(None) => return Ok(()),
                Err(error) => return Err(thrown(error)),
            },
        };
        let mut first = true;
        loop {
            let (key, value) = current;
            let kept = first.then(|| (key.clone(), value.clone()));
            if let Err(error) = body(key, value) {
                // The generator stays at the yield it gave.
                self.set(match kept {
                    Some(pair) => State::AtFirst(rest, pair),
                    None => State::Advanced(rest),
                });
                return Err(error);
            }
            first = false;
            current = match self.resume(&mut rest) {
                Ok(Some(pair)) => pair,
                Ok(None) => return Ok(()),
                Err(error) => return Err(thrown(error)),
            };
        }
    }

    /// Runs the body to its next `yield`; closes the generator when it
    /// returns or throws.
    fn resume(&self, rest: &mut Body) -> Result<Option<(Value, Value)>, Thrown> {
        match rest.next() {
            Some(Ok(pair)) => Ok(Some(pair)),
            Some(Err(thrown)) => {
                self.set(State::Closed);
                Err(thrown)
            }
            None => {
                self.set(State::Closed);
                Ok(None)
            }
        }
    }
}

impl PhpObject for Generator {
    fn class(&self) -> &str {
        CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn instance_of(&self, class: &str) -> bool {
        ["Traversable", "Iterator"]
            .iter()
            .any(|c| c.eq_ignore_ascii_case(class))
    }

    fn traverse(&self) -> Option<Result<Vec<(Value, Value)>, Thrown>> {
        let mut pairs = Vec::new();
        Some(
            self.for_each(
                |key, value| {
                    pairs.push((key, value));
                    Ok(())
                },
                |thrown| thrown,
            )
            .map(|()| pairs),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three() -> Generator {
        Generator::values((1..=3).map(|i| Ok(Value::Int(i))))
    }

    fn thrown(class: &str, message: &str) -> Thrown {
        Thrown::new(class, message, 0)
    }

    #[test]
    fn yields_its_pairs_once() {
        let generator = three();
        assert_eq!(
            generator.traverse(),
            Some(Ok(vec![
                (Value::Int(0), Value::Int(1)),
                (Value::Int(1), Value::Int(2)),
                (Value::Int(2), Value::Int(3)),
            ]))
        );
        assert_eq!(
            generator.traverse(),
            Some(Err(thrown(
                "Exception",
                "Cannot traverse an already closed generator"
            )))
        );
    }

    #[test]
    fn stopped_at_its_first_yield_it_gives_that_pair_again() {
        let generator = three();
        let stop = generator.for_each(|_, _| Err(thrown("RuntimeException", "stop")), |t| t);
        assert_eq!(stop, Err(thrown("RuntimeException", "stop")));
        assert_eq!(
            generator.traverse().map(|r| r.map(|p| p.len())),
            Some(Ok(3))
        );
    }

    #[test]
    fn past_its_first_yield_it_cannot_rewind() {
        let generator = three();
        let mut seen = 0;
        let stop = generator.for_each(
            |_, _| {
                seen += 1;
                if seen == 2 {
                    return Err(thrown("RuntimeException", "stop"));
                }
                Ok(())
            },
            |t| t,
        );
        assert!(stop.is_err());
        assert_eq!(
            generator.traverse(),
            Some(Err(thrown(
                "Exception",
                "Cannot rewind a generator that was already run"
            )))
        );
    }

    #[test]
    fn a_throwing_body_closes_it() {
        let generator = Generator::values(
            [Ok(Value::Int(1)), Err(thrown("LogicException", "body"))].into_iter(),
        );
        assert_eq!(
            generator.traverse(),
            Some(Err(thrown("LogicException", "body")))
        );
        assert_eq!(
            generator.traverse(),
            Some(Err(thrown(
                "Exception",
                "Cannot traverse an already closed generator"
            )))
        );
    }

    #[test]
    fn it_cannot_be_traversed_inside_its_own_traversal() {
        let generator = Arc::new(three());
        let inner = Arc::clone(&generator);
        let result = generator.for_each(
            |_, _| match inner.traverse() {
                Some(Err(thrown)) => Err(thrown),
                _ => Ok(()),
            },
            |t| t,
        );
        assert_eq!(
            result,
            Err(thrown(
                "Error",
                "Cannot resume an already running generator"
            ))
        );
    }

    #[test]
    fn is_traversable_without_properties() {
        let generator = three();
        assert!(generator.instance_of("traversable"));
        assert!(generator.instance_of("Iterator"));
        assert!(!generator.instance_of("JsonSerializable"));
        assert!(generator.properties().is_empty());
    }
}
