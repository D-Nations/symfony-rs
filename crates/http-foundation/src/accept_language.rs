//! The browser's preferred language, from the `Accept-Language` header.
//!
//! Ports the parts of `vendor/symfony/http-foundation` that
//! `RedirectToPreferredLocaleSubscriber` relies on:
//! `Request::getPreferredLanguage()` and `getLanguages()`, and the header
//! parsing beneath them (`AcceptHeader`, `AcceptHeaderItem`,
//! `HeaderUtils::split()`, from `header_utils`).
//!
//! Symfony's rules are kept, including the surprising ones, each recorded
//! from the original for 77 headers (`data/http/accept_language.php`):
//!
//! - a language with `q=0` still counts, after the others;
//! - a language listed twice keeps its first place in the list but takes
//!   the later entry's quality and position for sorting;
//! - a malformed `q` reads as PHP's `(float)` cast does (`q=abc` is 0,
//!   `q=0.8abc` is 0.8, a bare `q` is 1);
//! - quotes and backslashes are removed from every value.
//!
//! The header is read as bytes: a byte that is not ASCII never matches a
//! locale code, so no text decoding is needed.
//!
//! Upstream: symfony/http-foundation Request.php AcceptHeader.php AcceptHeaderItem.php HeaderUtils.php

use crate::header_utils::{Token, tokenize, unquote_bytes as unquote};

/// The tokens between occurrences of `separator`, skipping empty groups.
fn groups(tokens: &[Token], separator: u8) -> Vec<&[Token]> {
    tokens
        .split(|token| matches!(token, Token::Separator(found, _) if *found == separator))
        .filter(|group| !group.is_empty())
        .collect()
}

/// `name` or `name=value` (the innermost level of `groupParts()`): the
/// text before the first `=`, and everything after it.
fn name_and_value(tokens: &[Token]) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut name = Vec::new();
    let mut value: Option<Vec<u8>> = None;
    for token in tokens {
        match (token, value.as_mut()) {
            (Token::Separator(b'=', _), None) => value = Some(Vec::new()),
            (Token::Value(text) | Token::Separator(_, text), Some(value)) => {
                value.extend(unquote(text));
            }
            (Token::Value(text) | Token::Separator(_, text), None) => name.extend(unquote(text)),
        }
    }
    (name, value)
}

/// PHP's `(float)` cast of a string: the longest leading number, else 0.
fn php_float(text: &[u8]) -> f64 {
    let text = String::from_utf8_lossy(text);
    let text = text.trim_start_matches([' ', '\t', '\n', '\r', '\x0B', '\x0C']);
    let bytes = text.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let integer = digits(end);
    end += integer;
    let mut fraction = 0;
    if bytes.get(end) == Some(&b'.') {
        fraction = digits(end + 1);
        if integer > 0 || fraction > 0 {
            end += 1 + fraction;
        }
    }
    if integer == 0 && fraction == 0 {
        return 0.0;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let exponent_digits = digits(exponent);
        if exponent_digits > 0 {
            end = exponent + exponent_digits;
        }
    }
    text[..end].parse().unwrap_or(0.0)
}

/// One language of the header (`AcceptHeaderItem`).
struct Item {
    value: Vec<u8>,
    quality: f64,
    index: usize,
}

/// `AcceptHeader::fromString($header)->all()`: the values, best first.
fn sorted_values(header: &[u8]) -> Vec<Vec<u8>> {
    let tokens = tokenize(header, b",;=");
    // Items are kept by value: a repeated value replaces the earlier item
    // in its place.
    let mut items: Vec<Item> = Vec::new();
    let parsed = groups(&tokens, b',')
        .into_iter()
        .map(|item| groups(item, b';'))
        .filter(|parts| !parts.is_empty());
    // The index counts only the items that remain, as `fromString()` does.
    for (index, parts) in parsed.enumerate() {
        let mut parts = parts.into_iter().map(name_and_value);
        let Some((value, _)) = parts.next() else {
            continue;
        };
        let mut quality = 1.0;
        for (name, attribute) in parts {
            if name.eq_ignore_ascii_case(b"q") {
                quality = attribute.as_deref().map_or(1.0, php_float);
            }
        }
        let item = Item {
            value,
            quality,
            index,
        };
        match items
            .iter_mut()
            .find(|existing| existing.value == item.value)
        {
            Some(existing) => *existing = item,
            None => items.push(item),
        }
    }
    items.sort_by(|a, b| b.quality.total_cmp(&a.quality).then(a.index.cmp(&b.index)));
    items.into_iter().map(|item| item.value).collect()
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn php_float_casts() {
        for (text, value) in [
            ("0.8", 0.8),
            ("0.8abc", 0.8),
            ("abc", 0.0),
            ("", 0.0),
            (" .5", 0.5),
            ("1e1", 10.0),
            ("1e", 1.0),
            ("-1", -1.0),
            ("5.", 5.0),
            (".", 0.0),
        ] {
            assert_eq!(
                php_float(text.as_bytes()).to_bits(),
                f64::to_bits(value),
                "{text}"
            );
        }
    }
}
