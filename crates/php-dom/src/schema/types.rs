//! Simple-type validation: whitespace processing, the lexical spaces of
//! the built-in types the schemas use, the enumeration, pattern and
//! inclusive-bound facets, lists and unions, with libxml's messages.
//!
//! Behaviour pinned by the recording that differs from the XML Schema
//! recommendation: `dateTime` values are not whitespace-collapsed (a
//! leading or trailing space is invalid); `NMTOKENS` accepts an empty list;
//! `anyURI` accepts spaces.
//!
//! Upstream: none

use super::{Builtin, Schema, Variety};
use crate::chars::{is_name_char, is_name_start};
use std::fmt::Write as _;

/// A value the validator cannot decide (a character class needing
/// Unicode data this crate does not carry).
#[derive(Debug)]
pub(crate) struct Undecidable;

/// The errors of one value, as (code, message) without the
/// `Element '…'[, attribute '…']: ` prefix.
pub(crate) type ValueErrors = Vec<(u32, String)>;

fn collapse(s: &str) -> String {
    s.split([' ', '\t', '\n', '\r'])
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize(prim: Builtin, raw: &str) -> String {
    match prim {
        Builtin::AnySimpleType | Builtin::String | Builtin::DateTime => raw.to_owned(),
        _ => collapse(raw),
    }
}

fn nmtoken(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| is_name_char(c as u32))
}

fn ncname(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c != ':' && is_name_start(c as u32) => {}
        _ => return false,
    }
    chars.all(|c| c != ':' && is_name_char(c as u32))
}

fn language(s: &str) -> bool {
    let mut parts = s.split('-');
    let first = parts.next().unwrap_or_default();
    if first.is_empty() || first.len() > 8 || !first.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    parts.all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn any_uri(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let mut escaped = String::with_capacity(s.len());
    for b in s.bytes() {
        if b == b' ' || b >= 0x80 {
            let _ = write!(escaped, "%{b:02X}");
        } else {
            escaped.push(b as char);
        }
    }
    crate::uri::parse(&escaped).is_some()
}

fn hex_binary(s: &str) -> bool {
    s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The digits of an integer literal without sign or leading zeros
/// (`"0"` for zero), and whether it is negative; `None` if not an integer.
fn integer(s: &str) -> Option<(bool, String)> {
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'+') => (false, &s[1..]),
        Some(b'-') => (true, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let trimmed = digits.trim_start_matches('0');
    Some((
        neg,
        if trimmed.is_empty() {
            "0".into()
        } else {
            trimmed.into()
        },
    ))
}

fn positive_integer(s: &str) -> bool {
    matches!(integer(s), Some((false, d)) if d != "0")
}

/// Compares two non-negative integers given as canonical digit strings.
fn cmp_digits(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn date_time(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if b.first() == Some(&b'-') {
        i = 1;
    }
    let ys = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let year = &s[ys..i];
    if year.len() < 4
        || (year.len() > 4 && year.starts_with('0'))
        || year.bytes().all(|d| d == b'0')
    {
        return false;
    }
    let two = |i: usize| -> Option<u32> {
        let d = b.get(i..i + 2)?;
        if d.iter().all(u8::is_ascii_digit) {
            Some(u32::from(d[0] - b'0') * 10 + u32::from(d[1] - b'0'))
        } else {
            None
        }
    };
    let fixed = |i: usize, c: u8| b.get(i) == Some(&c);
    if !fixed(i, b'-') {
        return false;
    }
    let Some(month) = two(i + 1) else {
        return false;
    };
    if !fixed(i + 3, b'-') {
        return false;
    }
    let Some(day) = two(i + 4) else { return false };
    if !fixed(i + 6, b'T') {
        return false;
    }
    let Some(hour) = two(i + 7) else { return false };
    if !fixed(i + 9, b':') {
        return false;
    }
    let Some(minute) = two(i + 10) else {
        return false;
    };
    if !fixed(i + 12, b':') {
        return false;
    }
    let Some(second) = two(i + 13) else {
        return false;
    };
    i += 15;
    let mut frac_zero = true;
    if fixed(i, b'.') {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            if b[i] != b'0' {
                frac_zero = false;
            }
            i += 1;
        }
        if i == fs {
            return false;
        }
    }
    if i < b.len() {
        if b[i] == b'Z' {
            i += 1;
        } else if b[i] == b'+' || b[i] == b'-' {
            let (Some(th), Some(tm)) = (two(i + 1), two(i + 4)) else {
                return false;
            };
            if !fixed(i + 3, b':') || th > 14 || tm > 59 || (th == 14 && tm != 0) {
                return false;
            }
            i += 6;
        } else {
            return false;
        }
    }
    if i != b.len() {
        return false;
    }
    let y: i64 = year.parse().unwrap_or(1);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    if day == 0 || day > days || minute > 59 || second > 59 {
        return false;
    }
    hour < 24 || (hour == 24 && minute == 0 && second == 0 && frac_zero)
}

fn lexical(prim: Builtin, v: &str) -> bool {
    match prim {
        Builtin::AnySimpleType | Builtin::String => true,
        Builtin::NmToken => nmtoken(v),
        Builtin::NcName | Builtin::Id => ncname(v),
        Builtin::Language => language(v),
        Builtin::AnyUri => any_uri(v),
        Builtin::HexBinary => hex_binary(v),
        Builtin::PositiveInteger => positive_integer(v),
        Builtin::DateTime => date_time(v),
        // A list type: never reached through `Variety::Atomic`.
        Builtin::NmTokens => false,
    }
}

fn equal(prim: Builtin, a: &str, b: &str) -> bool {
    match prim {
        Builtin::PositiveInteger => match (integer(a), integer(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
        Builtin::HexBinary => a.eq_ignore_ascii_case(b),
        _ => a == b,
    }
}

impl Schema {
    fn type_label(&self, ty: usize, kind: &str) -> String {
        match &self.simple[ty].name {
            Some(n) => format!("the {kind} type '{n}'"),
            None => format!("the local {kind} type"),
        }
    }

    /// Whether values of `ty` are IDs (unique in the document).
    pub(crate) fn is_id_type(&self, ty: usize) -> bool {
        matches!(
            self.simple[ty].variety,
            Variety::Atomic {
                primitive: Builtin::Id
            }
        )
    }

    /// Validates `raw` against the simple type `ty`. `Ok(Some(v))`: valid,
    /// with its normalized value; `Ok(None)`: invalid, errors pushed to
    /// `errors` (not collected when `None`, as for union members).
    pub(crate) fn check_value(
        &self,
        ty: usize,
        raw: &str,
        errors: &mut Option<&mut ValueErrors>,
    ) -> Result<Option<String>, Undecidable> {
        let t = &self.simple[ty];
        match &t.variety {
            Variety::Atomic { primitive } => {
                let prim = *primitive;
                let norm = normalize(prim, raw);
                if !lexical(prim, &norm) {
                    if let Some(e) = errors {
                        let label = self.type_label(ty, "atomic");
                        e.push((1824, format!("'{raw}' is not a valid value of {label}.")));
                    }
                    return Ok(None);
                }
                for facets in &t.facets {
                    if !facets.enumeration.is_empty()
                        && !facets.enumeration.iter().any(|e| equal(prim, e, &norm))
                    {
                        if let Some(e) = errors {
                            let set: Vec<String> = facets
                                .enumeration
                                .iter()
                                .map(|v| format!("'{v}'"))
                                .collect();
                            e.push((
                                1840,
                                format!(
                                    "[facet 'enumeration'] The value '{norm}' is not an element of the set {{{}}}.",
                                    set.join(", ")
                                ),
                            ));
                        }
                        return Ok(None);
                    }
                    if !facets.patterns.is_empty() {
                        let mut any = false;
                        for p in &facets.patterns {
                            if p.matches(&norm).map_err(|()| Undecidable)? {
                                any = true;
                                break;
                            }
                        }
                        if !any {
                            if let Some(e) = errors {
                                e.push((
                                    1839,
                                    format!(
                                        "[facet 'pattern'] The value '{norm}' is not accepted by the pattern '{}'.",
                                        facets.patterns[0].source
                                    ),
                                ));
                            }
                            return Ok(None);
                        }
                    }
                    let value = integer(&norm).map(|(_, d)| d).unwrap_or_default();
                    if let Some(min) = &facets.min_inclusive {
                        let m = integer(min).map(|(_, d)| d).unwrap_or_default();
                        if cmp_digits(&value, &m).is_lt() {
                            if let Some(e) = errors {
                                e.push((
                                    1833,
                                    format!(
                                        "[facet 'minInclusive'] The value '{norm}' is less than the minimum value allowed ('{min}')."
                                    ),
                                ));
                            }
                            return Ok(None);
                        }
                    }
                    if let Some(max) = &facets.max_inclusive {
                        let m = integer(max).map(|(_, d)| d).unwrap_or_default();
                        if cmp_digits(&value, &m).is_gt() {
                            if let Some(e) = errors {
                                e.push((
                                    1834,
                                    format!(
                                        "[facet 'maxInclusive'] The value '{norm}' is greater than the maximum value allowed ('{max}')."
                                    ),
                                ));
                            }
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(norm))
            }
            Variety::List { item } => {
                let norm = collapse(raw);
                for v in norm.split(' ').filter(|v| !v.is_empty()) {
                    if self.check_value(*item, v, errors)?.is_none() {
                        if let Some(e) = errors {
                            let label = self.type_label(ty, "list");
                            e.push((1825, format!("'{raw}' is not a valid value of {label}.")));
                        }
                        return Ok(None);
                    }
                }
                Ok(Some(norm))
            }
            Variety::Union { members } => {
                for &m in members {
                    if let Some(v) = self.check_value(m, raw, &mut None)? {
                        return Ok(Some(v));
                    }
                }
                if let Some(e) = errors {
                    let label = self.type_label(ty, "union");
                    e.push((1826, format!("'{raw}' is not a valid value of {label}.")));
                }
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_times() {
        for ok in [
            "2008-01-01T00:00:00",
            "2008-01-01T00:00:00Z",
            "2008-01-01T00:00:00+02:00",
            "2008-01-01T00:00:00.123",
            "2008-01-01T24:00:00",
            "-0001-01-01T00:00:00",
            "20080-01-01T00:00:00",
            "2000-02-29T00:00:00",
        ] {
            assert!(date_time(ok), "{ok}");
        }
        for bad in [
            "2008-01-01",
            "2008-13-01T00:00:00",
            "2008-02-30T00:00:00",
            "2008-01-01 00:00:00",
            " 2008-01-01T00:00:00",
            "2008-01-01T00:00:60",
            "2008-01-01T00:00:00+14:01",
            "1900-02-29T00:00:00",
            "2008-01-01T24:00:01",
        ] {
            assert!(!date_time(bad), "{bad}");
        }
    }

    #[test]
    fn lexical_spaces() {
        assert!(language("en-US") && language("x-klingon") && !language("abcdefghi"));
        assert!(!language("en--US") && !language("") && !language("1en"));
        assert!(any_uri("a b") && any_uri("#frag") && !any_uri("%zz") && !any_uri(":x"));
        assert!(hex_binary("0A1f") && hex_binary("") && !hex_binary("abc"));
        assert!(positive_integer("+5") && positive_integer("05") && !positive_integer("0"));
        assert!(!positive_integer("-1") && !positive_integer("1e3"));
        assert!(ncname("_a") && !ncname("a:b") && !ncname("-a"));
        assert!(nmtoken("a.b-c_d:e") && !nmtoken("a b") && !nmtoken(""));
    }
}
