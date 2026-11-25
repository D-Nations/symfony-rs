//! `BcMath\Number` (PHP 8.4): construction from a string or an int, its
//! string form, `scale` and `<=>`. Arithmetic is not ported.
//!
//! A string is well-formed when it is `[+-]?[0-9]*(\.[0-9]*)?` with nothing
//! around it (so `""`, `"+"` and `"."` are zero); leading zeros are
//! dropped, the fraction's digits are kept (`"5.5000"` has scale 4), and
//! zero has no sign. Written from the PHP manual and the answers of PHP
//! 8.4.27 with the bcmath extension (the oracle image does not build it;
//! the cases are pinned in this module's tests).
//!
//! Upstream: none

use std::cmp::Ordering;

use crate::{
    array::PhpArray,
    string::PhpString,
    value::{PhpObject, Thrown, Value},
};

/// A `BcMath\Number`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcNumber {
    negative: bool,
    /// Integer digits without leading zeros (empty for 0).
    integer: Vec<u8>,
    /// Fraction digits, as written.
    fraction: Vec<u8>,
}

impl BcNumber {
    pub const CLASS: &'static str = "BcMath\\Number";

    /// `new BcMath\Number($num)` under `strict_types`: the `ValueError` or
    /// `TypeError` it throws otherwise.
    pub fn new(num: &Value) -> Result<Self, Thrown> {
        match num {
            Value::String(s) => Self::parse(s.as_bytes()).ok_or_else(|| {
                Thrown::new(
                    "ValueError",
                    "BcMath\\Number::__construct(): Argument #1 ($num) is not well-formed",
                    0,
                )
            }),
            Value::Int(i) => Self::parse(i.to_string().as_bytes()).ok_or_else(|| {
                Thrown::new(
                    "ValueError",
                    "BcMath\\Number::__construct(): Argument #1 ($num) is not well-formed",
                    0,
                )
            }),
            other => Err(Thrown::new(
                "TypeError",
                format!(
                    "BcMath\\Number::__construct(): Argument #1 ($num) must be of type string|int, {} given",
                    other.value_name()
                ),
                0,
            )),
        }
    }

    /// A well-formed number string; `None` otherwise.
    pub fn parse(text: &[u8]) -> Option<Self> {
        let (negative, rest) = match text.first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let (integer, fraction) = match rest.iter().position(|&b| b == b'.') {
            Some(dot) => (&rest[..dot], &rest[dot + 1..]),
            None => (rest, &[][..]),
        };
        if !integer.iter().chain(fraction).all(u8::is_ascii_digit) {
            return None;
        }
        let integer: Vec<u8> = integer.iter().copied().skip_while(|&d| d == b'0').collect();
        let zero = integer.is_empty() && fraction.iter().all(|&d| d == b'0');
        Some(Self {
            negative: negative && !zero,
            integer,
            fraction: fraction.to_vec(),
        })
    }

    /// `$number->scale`.
    pub fn scale(&self) -> usize {
        self.fraction.len()
    }

    /// `(string) $number`.
    pub fn to_php_string(&self) -> PhpString {
        let mut out = Vec::new();
        if self.negative {
            out.push(b'-');
        }
        if self.integer.is_empty() {
            out.push(b'0');
        } else {
            out.extend_from_slice(&self.integer);
        }
        if !self.fraction.is_empty() {
            out.push(b'.');
            out.extend_from_slice(&self.fraction);
        }
        PhpString::from(out)
    }

    /// `$a <=> $b`: by value, whatever the scales.
    pub fn compare(&self, other: &Self) -> Ordering {
        let magnitude = |a: &Self, b: &Self| {
            a.integer
                .len()
                .cmp(&b.integer.len())
                .then_with(|| a.integer.cmp(&b.integer))
                .then_with(|| {
                    let len = a.fraction.len().max(b.fraction.len());
                    let digit = |n: &Self, i: usize| n.fraction.get(i).copied().unwrap_or(b'0');
                    (0..len)
                        .map(|i| digit(a, i).cmp(&digit(b, i)))
                        .find(|o| o.is_ne())
                        .unwrap_or(Ordering::Equal)
                })
        };
        match (self.negative, other.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => magnitude(self, other),
            (true, true) => magnitude(other, self),
        }
    }
}

impl PhpObject for BcNumber {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("value", self.to_php_string().into());
        properties.insert(
            "scale",
            i64::try_from(self.scale()).unwrap_or(i64::MAX).into(),
        );
        properties
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(BcNumber::to_php_string(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PHP 8.4.27 with bcmath: (text, string form or `None` for `ValueError`,
    /// scale).
    const CASES: &[(&str, Option<&str>, usize)] = &[
        ("5.5", Some("5.5"), 1),
        ("5.5000", Some("5.5000"), 4),
        ("05.5", Some("5.5"), 1),
        ("+5.5", Some("5.5"), 1),
        ("-0", Some("0"), 0),
        (".5", Some("0.5"), 1),
        ("5.", Some("5"), 0),
        ("", Some("0"), 0),
        ("foo", None, 0),
        (" 5", None, 0),
        ("5 ", None, 0),
        ("1e5", None, 0),
        ("-.5", Some("-0.5"), 1),
        ("0.0", Some("0.0"), 1),
        ("-0.00", Some("0.00"), 2),
        ("00", Some("0"), 0),
        ("+", Some("0"), 0),
        ("-", Some("0"), 0),
        (".", Some("0"), 0),
    ];

    #[test]
    fn parse_and_string_form() {
        let five_and_a_half = BcNumber::parse(b"5.5").unwrap();
        for &(text, string, scale) in CASES {
            let parsed = BcNumber::parse(text.as_bytes());
            assert_eq!(
                parsed.as_ref().map(BcNumber::to_php_string),
                string.map(PhpString::from),
                "{text:?}"
            );
            if let Some(n) = parsed {
                assert_eq!(n.scale(), scale, "{text:?}");
                // PHP: only "5.5", "5.5000", "05.5" and "+5.5" equal 5.5.
                let equal = matches!(text, "5.5" | "5.5000" | "05.5" | "+5.5");
                assert_eq!(n.compare(&five_and_a_half).is_eq(), equal, "{text:?}");
            }
        }
    }

    #[test]
    fn construct_errors() {
        assert_eq!(
            BcNumber::new(&Value::Bool(true)).unwrap_err().message,
            PhpString::from(
                "BcMath\\Number::__construct(): Argument #1 ($num) must be of type string|int, true given"
            )
        );
        assert_eq!(
            BcNumber::new(&Value::Float(5.5)).unwrap_err().class,
            "TypeError"
        );
        assert_eq!(
            BcNumber::new(&Value::from("foo")).unwrap_err().message,
            PhpString::from("BcMath\\Number::__construct(): Argument #1 ($num) is not well-formed")
        );
        assert_eq!(
            BcNumber::new(&Value::Int(-12)).unwrap().to_php_string(),
            PhpString::from("-12")
        );
    }
}
