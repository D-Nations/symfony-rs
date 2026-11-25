//! Port of Symfony\Component\Finder\Comparator\NumberComparator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! The magnitudes follow Richard Clamp's Perl `Number::Compare`.
//!
//! Upstream: symfony/finder Comparator/NumberComparator.php

use std::{ops::Deref, sync::LazyLock};

use php_value::numeric::{float_to_string, is_numeric, string_to_float, string_to_int};

use super::Comparator;
use crate::{exception::Error, pcre::Pcre};

static TEST: LazyLock<Option<Pcre>> =
    LazyLock::new(|| Pcre::new(br"#^\s*(==|!=|[<>]=?)?\s*([0-9\.]+)\s*([kmg]i?)?\s*$#i").ok());

/// A number test such as `> 10K`, `<= 1Mi` or `3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberComparator(Comparator);

impl NumberComparator {
    /// `new NumberComparator($test)`; `None` is PHP's `null`.
    #[expect(
        clippy::float_cmp,
        clippy::cast_precision_loss,
        reason = "PHP compares and converts ints and floats this way"
    )]
    pub fn new(test: Option<&[u8]>) -> Result<Self, Error> {
        let not_understood = || {
            Error::InvalidArgument(format!(
                "Don't understand \"{}\" as a number test.",
                test.map_or("null".into(), String::from_utf8_lossy)
            ))
        };
        let Some(test) = test else {
            return Err(not_understood());
        };
        let pattern = TEST
            .as_ref()
            .ok_or_else(|| Error::Logic("NumberComparator pattern".into()))?;
        let groups = pattern
            .captures_from(test, 0)
            .ok()
            .flatten()
            .ok_or_else(not_understood)?;
        let group = |i: usize| -> &[u8] {
            match groups.get(i).copied().flatten() {
                Some((s, e)) => &test[s..e],
                None => b"",
            }
        };
        let target = group(2);
        if !is_numeric(target) {
            return Err(Error::InvalidArgument(format!(
                "Invalid number \"{}\".",
                String::from_utf8_lossy(target)
            )));
        }
        let mut target = target.to_vec();
        if groups.get(3).copied().flatten().is_some() {
            let factor: i64 = match group(3).to_ascii_lowercase().as_slice() {
                b"k" => 1000,
                b"ki" => 1024,
                b"m" => 1_000_000,
                b"mi" => 1024 * 1024,
                b"g" => 1_000_000_000,
                _ => 1024 * 1024 * 1024,
            };
            // `$target *= $factor` on a numeric string, then the string the
            // `string $target` parameter receives
            let integer_form = !target.iter().any(|&c| matches!(c, b'.' | b'e' | b'E'));
            let product = if integer_form {
                let i = string_to_int(&target);
                i.checked_mul(factor)
                    .map(|v| v.to_string())
                    .filter(|_| i != i64::MAX || string_to_float(&target) == i64::MAX as f64)
            } else {
                None
            };
            target = product
                .unwrap_or_else(|| float_to_string(string_to_float(&target) * factor as f64))
                .into_bytes();
        }
        let operator = String::from_utf8_lossy(group(1)).into_owned();
        let operator = if operator.is_empty() { "==" } else { &operator };
        Ok(Self(Comparator::new(target, operator)?))
    }
}

impl Deref for NumberComparator {
    type Target = Comparator;

    fn deref(&self) -> &Comparator {
        &self.0
    }
}
