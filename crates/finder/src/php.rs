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
    match (num(a), num(b)) {
        (Some(x), Some(y)) => cmp_num(x, y),
        _ => {
            let text = |s: &Scalar| match s {
                Scalar::Int(i) => num_to_string(Num::Int(*i)),
                Scalar::Float(f) => num_to_string(Num::Float(*f)),
                Scalar::String(s) => s.clone(),
            };
            text(a).cmp(&text(b))
        }
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
pub fn strnatcmp(a: &[u8], b: &[u8]) -> Ordering {
    natural(a, b, false)
}

/// The sign of `strnatcasecmp($a, $b)`.
pub fn strnatcasecmp(a: &[u8], b: &[u8]) -> Ordering {
    natural(a, b, true)
}

fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Natural order: runs of digits compare as numbers ("2" < "10"), except
/// that a run starting with "0" compares digit by digit as a fraction;
/// whitespace is skipped (but not right after a run of digits) and leading
/// zeros at the very start are ignored.
fn natural(a: &[u8], b: &[u8], fold: bool) -> Ordering {
    if a.is_empty() || b.is_empty() {
        return a.len().cmp(&b.len());
    }
    let (mut i, mut j) = (0usize, 0usize);
    let mut leading = true;
    let mut after_digits = false;
    loop {
        let at = |s: &[u8], k: usize| s.get(k).copied().unwrap_or(0);
        // leading zeros of the first number are not significant
        if leading {
            while at(a, i) == b'0' && i + 1 < a.len() && at(a, i + 1).is_ascii_digit() {
                i += 1;
            }
            while at(b, j) == b'0' && j + 1 < b.len() && at(b, j + 1).is_ascii_digit() {
                j += 1;
            }
            leading = false;
        }
        // Whitespace is skipped, except right after a run of digits
        // (observed: "2 B" < "2-", but "a B" > "a-").
        if !after_digits {
            while is_c_space(at(a, i)) {
                i += 1;
            }
            while is_c_space(at(b, j)) {
                j += 1;
            }
        }
        after_digits = false;
        let (ca, cb) = (at(a, i), at(b, j));
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let fractional = ca == b'0' || cb == b'0';
            let result = if fractional {
                compare_left(a, &mut i, b, &mut j)
            } else {
                compare_right(a, &mut i, b, &mut j)
            };
            if result != Ordering::Equal {
                return result;
            }
            match (i >= a.len(), j >= b.len()) {
                (true, true) => return Ordering::Equal,
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {
                    after_digits = true;
                    continue;
                }
            }
        }
        let (ca, cb) = if fold {
            (ca.to_ascii_uppercase(), cb.to_ascii_uppercase())
        } else {
            (ca, cb)
        };
        match ca.cmp(&cb) {
            Ordering::Equal => {}
            o => return o,
        }
        i += 1;
        j += 1;
        match (i >= a.len(), j >= b.len()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
    }
}

/// Digit runs aligned on the right: the longer run is larger; equal
/// lengths are decided by the first differing digit.
fn compare_right(a: &[u8], i: &mut usize, b: &[u8], j: &mut usize) -> Ordering {
    let mut bias = Ordering::Equal;
    loop {
        let da = a.get(*i).is_some_and(u8::is_ascii_digit);
        let db = b.get(*j).is_some_and(u8::is_ascii_digit);
        match (da, db) {
            (false, false) => return bias,
            (false, true) => return Ordering::Less,
            (true, false) => return Ordering::Greater,
            (true, true) => {
                if bias == Ordering::Equal {
                    bias = a[*i].cmp(&b[*j]);
                }
            }
        }
        *i += 1;
        *j += 1;
    }
}

/// Digit runs aligned on the left (fractions): the first differing digit
/// decides; a run that ends first is smaller.
fn compare_left(a: &[u8], i: &mut usize, b: &[u8], j: &mut usize) -> Ordering {
    loop {
        let da = a.get(*i).is_some_and(u8::is_ascii_digit);
        let db = b.get(*j).is_some_and(u8::is_ascii_digit);
        match (da, db) {
            (false, false) => return Ordering::Equal,
            (false, true) => return Ordering::Less,
            (true, false) => return Ordering::Greater,
            (true, true) => match a[*i].cmp(&b[*j]) {
                Ordering::Equal => {}
                o => return o,
            },
        }
        *i += 1;
        *j += 1;
    }
}

/// `pathinfo($filename, PATHINFO_FILENAME)` of a base name.
pub fn filename_without_extension(basename: &[u8]) -> &[u8] {
    match basename.iter().rposition(|&c| c == b'.') {
        Some(dot) => &basename[..dot],
        None => basename,
    }
}

/// `SplFileInfo::getExtension()` of a base name.
pub fn extension(basename: &[u8]) -> &[u8] {
    match basename.iter().rposition(|&c| c == b'.') {
        Some(dot) => &basename[dot + 1..],
        None => b"",
    }
}
