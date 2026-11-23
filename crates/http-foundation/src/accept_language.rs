//! The browser's preferred language, from the `Accept-Language` header.
//!
//! Ports the parts of `vendor/symfony/http-foundation` that
//! `RedirectToPreferredLocaleSubscriber` relies on:
//! `Request::getPreferredLanguage()` and `getLanguages()`, on
//! `accept_header` (`AcceptHeader::fromString()->all()`).
//!
//! Symfony's rules are kept, including the surprising ones, each recorded
//! from the original for 77 headers (`data/http/accept_language.php`):
//!
//! - a language with `q=0` still counts, after the others;
//! - a language listed twice (with the same attributes other than `q`)
//!   keeps its first place in the list but takes the later entry's quality
//!   and position for sorting;
//! - a malformed `q` reads as PHP's `(float)` cast does (`q=abc` is 0,
//!   `q=0.8abc` is 0.8, a bare `q` is 1);
//! - quotes and backslashes are removed from every value.
//!
//! The header is read as bytes: a byte that is not ASCII never matches a
//! locale code, so no text decoding is needed.
//!
//! Upstream: symfony/http-foundation Request.php

use crate::accept_header::AcceptHeader;

/// `AcceptHeader::fromString($header)->all()`: the values, best first.
fn sorted_values(header: &[u8]) -> Vec<Vec<u8>> {
    AcceptHeader::from_string(Some(header))
        .all()
        .iter()
        .map(|(_, item)| item.value().to_vec())
        .collect()
}

/// `Request::getLanguageComponents()`: language, script and region, or
/// the whole text (lower-cased, `_` as `-`) when it is not a language tag.
fn components(locale: &[u8]) -> (Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>) {
    let text: Vec<u8> = locale
        .iter()
        .map(|byte| match byte.to_ascii_lowercase() {
            b'_' => b'-',
            other => other,
        })
        .collect();
    match match_tag(&text) {
        Some((language, script, region)) => (
            language.to_vec(),
            script.map(|script| {
                let mut script = script.to_vec();
                if let Some(first) = script.first_mut() {
                    first.make_ascii_uppercase();
                }
                script
            }),
            region.map(<[u8]>::to_ascii_uppercase),
        ),
        None => (text, None, None),
    }
}

type Tag<'a> = (&'a [u8], Option<&'a [u8]>, Option<&'a [u8]>);

/// `^([a-zA-Z]{2,3}|i-[a-zA-Z]{5,})(?:-([a-zA-Z]{4}))?(?:-([a-zA-Z]{2}))?(?:-(.+))?$`,
/// tried in the order PCRE backtracks.
fn match_tag(text: &[u8]) -> Option<Tag<'_>> {
    let letters = |from: usize| {
        text.get(from..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| byte.is_ascii_alphabetic())
            .count()
    };
    let subtag = |at: usize, length: usize| -> Option<&[u8]> {
        (text.get(at) == Some(&b'-') && letters(at + 1) >= length)
            .then(|| text.get(at + 1..at + 1 + length))
            .flatten()
    };
    let rest_matches =
        |at: usize| at == text.len() || (text.get(at) == Some(&b'-') && at + 1 < text.len());

    let mut languages: Vec<(&[u8], usize)> = Vec::new();
    for length in [3, 2] {
        if letters(0) >= length {
            languages.push((&text[..length], length));
        }
    }
    if text.starts_with(b"i-") {
        for length in (5..=letters(2)).rev() {
            languages.push((&text[2..2 + length], 2 + length));
        }
    }
    for (language, after_language) in languages {
        for script in [subtag(after_language, 4), None] {
            let after_script = after_language + script.map_or(0, |script| 1 + script.len());
            for region in [subtag(after_script, 2), None] {
                let after_region = after_script + region.map_or(0, |region| 1 + region.len());
                if rest_matches(after_region) {
                    return Some((language, script, region));
                }
            }
        }
    }
    None
}

/// `implode('_', array_filter([...]))`: PHP's `array_filter()` drops
/// empty strings and `"0"`.
fn join(parts: &[Option<&[u8]>]) -> Vec<u8> {
    let kept: Vec<&[u8]> = parts
        .iter()
        .flatten()
        .copied()
        .filter(|part| !part.is_empty() && *part != b"0")
        .collect();
    kept.join(&b'_')
}

/// `Request::formatLocale()`
fn format_locale(locale: &[u8]) -> Vec<u8> {
    let (language, script, region) = components(locale);
    join(&[Some(&language), script.as_deref(), region.as_deref()])
}

/// `Request::getLanguageCombinations()`
fn combinations(locale: &[u8]) -> Vec<Vec<u8>> {
    let (language, script, region) = components(locale);
    let (script, region) = (script.as_deref(), region.as_deref());
    let mut combinations: Vec<Vec<u8>> = Vec::new();
    for combination in [
        join(&[Some(&language), script, region]),
        join(&[Some(&language), script]),
        join(&[Some(&language), region]),
        language.clone(),
    ] {
        if !combinations.contains(&combination) {
            combinations.push(combination);
        }
    }
    combinations
}

/// `Request::getLanguages()`: the header's languages, best first, as
/// locale codes (`fr-FR` becomes `fr_FR`), without repeats.
pub fn languages(header: &[u8]) -> Vec<Vec<u8>> {
    let mut languages: Vec<Vec<u8>> = Vec::new();
    for value in sorted_values(header) {
        let language = format_locale(&value);
        if !languages.contains(&language) {
            languages.push(language);
        }
    }
    languages
}

/// `Request::getPreferredLanguage($locales)`: the first of `locales` that
/// matches one of the header's languages (`fr_FR`, then `fr`), or the first
/// of `locales`. `locales` must not be empty.
pub fn preferred_language<'a>(header: Option<&[u8]>, locales: &[&'a str]) -> Option<&'a str> {
    let first = locales.first().copied()?;
    for language in languages(header.unwrap_or_default()) {
        for combination in combinations(&language) {
            for locale in locales {
                let locale_bytes = locale.as_bytes();
                let prefix_matches = locale_bytes.starts_with(&combination)
                    && locale_bytes.get(combination.len()) == Some(&b'_');
                if locale_bytes == combination.as_slice() || prefix_matches {
                    return Some(locale);
                }
            }
        }
    }
    Some(first)
}
