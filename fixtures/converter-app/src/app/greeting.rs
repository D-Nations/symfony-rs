//! `Fixture\Service\Greeting` (oracle/apps/converter-fixture/src/Service/Greeting.php).
//!
//! A readonly class: `Arc<Greeting>` (converter contract 0.1, section 3).
//! Typed properties keep their Rust types; the `array` property is a
//! `PhpArray`, as section 2 requires for an array whose shape is not proven.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value, numeric::float_to_string};

use crate::runtime::{NotPorted, php::implode};

#[derive(Debug)]
pub struct Greeting {
    pub text: PhpString,
    pub length: i64,
    pub ratio: f64,
    pub tags: PhpArray,
    pub visits: i64,
}

impl Greeting {
    pub fn render(&self) -> Result<PhpString, NotPorted> {
        let lines = PhpArray::list([
            Value::string(concat(b"text: ", &self.text)),
            Value::string(format!("length: {}", self.length)),
            Value::string(format!("ratio: {}", float_to_string(self.ratio))),
            Value::string(concat(b"tags: ", &implode(b", ", &self.tags)?)),
            Value::string(format!("visits: {}", self.visits)),
        ]);
        let mut out = implode(b"\n", &lines)?.to_vec();
        out.push(b'\n');
        Ok(PhpString::from(out))
    }
}

/// PHP's `.` on two strings.
fn concat(left: &[u8], right: &[u8]) -> Vec<u8> {
    [left, right].concat()
}
