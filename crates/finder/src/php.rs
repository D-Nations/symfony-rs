//! PHP built-ins Finder relies on, written from PHP's documentation and
//! checked against PHP 8.4.27 (tests/data/php.json): PHP 8 comparison of
//! ints, floats and strings, `strcmp()`, `strcasecmp()`, `strnatcmp()`,
//! `strnatcasecmp()` and `pathinfo()`'s file name. No PHP source is used.
//!
//! Upstream: none

use std::cmp::Ordering;

use php_value::numeric::{is_numeric, string_to_float, string_to_int};

/// An operand of a PHP comparison.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Int(i64),
    Float(f64),
    String(Vec<u8>),
}

impl From<i64> for Scalar {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<&str> for Scalar {
    fn from(s: &str) -> Self {
        Self::String(s.as_bytes().to_vec())
    }
}

impl From<String> for Scalar {
    fn from(s: String) -> Self {
        Self::String(s.into_bytes())
    }
}

#[derive(Debug, Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

/// The number a numeric string stands for (an int when it is written as
/// one and fits).
fn numeric_value(s: &[u8]) -> Num {
    let integer_form = !s.iter().any(|&c| matches!(c, b'.' | b'e' | b'E'));
    let f = string_to_float(s);
    if integer_form {
        let i = string_to_int(s);
        if (i != i64::MAX && i != i64::MIN) || (i as f64) == f {
            return Num::Int(i);
        }
    }
    Num::Float(f)
}

fn cmp_num(a: Num, b: Num) -> Ordering {
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => x.cmp(&y),
        (x, y) => {
            let (x, y) = (to_f64(x), to_f64(y));
            x.partial_cmp(&y).unwrap_or(Ordering::Less)
        }
    }
}

fn to_f64(n: Num) -> f64 {
    match n {
        Num::Int(i) => i as f64,
        Num::Float(f) => f,
    }
}

fn num_to_string(n: Num) -> Vec<u8> {
    match n {
        Num::Int(i) => i.to_string().into_bytes(),
        Num::Float(f) => php_value::numeric::float_to_string(f).into_bytes(),
    }
}

/// `$a <=> $b` (PHP 8 rules for ints, floats and strings).
pub fn compare(a: &Scalar, b: &Scalar) -> Ordering {
    let num = |s: &Scalar| match s {
        Scalar::Int(i) => Some(Num::Int(*i)),
        Scalar::Float(f) => Some(Num::Float(*f)),
        Scalar::String(s) if is_numeric(s) => Some(numeric_value(s)),
        Scalar::String(_) => None,
    };
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        cmp_num(x, y)
    } else {
        let text = |s: &Scalar| match s {
            Scalar::Int(i) => num_to_string(Num::Int(*i)),
            Scalar::Float(f) => num_to_string(Num::Float(*f)),
            Scalar::String(s) => s.clone(),
        };
        text(a).cmp(&text(b))
    }
}

/// `$a == $b`.
pub fn loose_eq(a: &Scalar, b: &Scalar) -> bool {
    compare(a, b) == Ordering::Equal
}

/// The sign of `strcmp($a, $b)`.
pub fn strcmp(a: &[u8], b: &[u8]) -> Ordering {
    a.cmp(b)
}

/// The sign of `strcasecmp($a, $b)` (ASCII case folding).
pub fn strcasecmp(a: &[u8], b: &[u8]) -> Ordering {
    a.iter()
        .map(u8::to_ascii_lowercase)
        .cmp(b.iter().map(u8::to_ascii_lowercase))
}

/// The sign of `strnatcmp($a, $b)`.
pub use php_value::string::{strnatcasecmp, strnatcmp};

/// `pathinfo($filename, PATHINFO_FILENAME)` of a base name.
pub fn filename_without_extension(basename: &[u8]) -> &[u8] {
    match basename.iter().rposition(|&c| c == b'.') {
        Some(dot) => &basename[..dot],
        None => basename,
    }
}

/// `SplFileInfo::getExtension()` of a base name.
pub use php_value::path::basename_extension as extension;
