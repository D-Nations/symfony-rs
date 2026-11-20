//! `Fixture\Service\Greeter` (oracle/apps/converter-fixture/src/Service/Greeter.php).
//!
//! Its own properties are readonly, but it holds the request-scoped
//! `VisitCounter`, so it is request-scoped too (converter contract 0.2,
//! section 4: what a service reaches counts as its state).
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpArray, PhpString};

use super::{
    greeting::Greeting, greeting_exception::GreetingException, visit_counter::VisitCounter,
};
use crate::runtime::php::ucfirst;

#[derive(Debug)]
pub struct Greeter {
    greeting: PhpString,
    max_length: i64,
    tags: PhpArray,
    visits: VisitCounter,
}

impl Greeter {
    pub fn new(greeting: PhpString, max_length: i64, tags: PhpArray, visits: VisitCounter) -> Self {
        Self {
            greeting,
            max_length,
            tags,
            visits,
        }
    }

    pub fn greet(
        &self,
        name: &PhpString,
        punctuation: &PhpString,
    ) -> Result<Arc<Greeting>, GreetingException> {
        // strlen() fits in an int: a PHP string is shorter than 2^63 bytes.
        let length = i64::try_from(name.len()).unwrap_or(i64::MAX);
        if length > self.max_length {
            return Err(GreetingException::new(format!(
                "Name \"{}\" is longer than {} characters.",
                name.to_string_lossy(),
                self.max_length
            )));
        }
        let text = [
            self.greeting.as_bytes(),
            b", ",
            &ucfirst(name),
            punctuation.as_bytes(),
        ]
        .concat();
        Ok(Arc::new(Greeting {
            text: PhpString::from(text),
            length,
            // int / int, then coerced to the `float` property. The
            // parameter is 12, never 0, so DivisionByZeroError cannot occur.
            ratio: length as f64 / self.max_length as f64,
            tags: self.tags.clone(),
            visits: self.visits.increment(),
        }))
    }
}
