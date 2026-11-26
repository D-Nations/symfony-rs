//! Port of Doctrine\DBAL\Schema\Name\Parser.php (the interface: the
//! `parse` methods here), Name\Parsers.php (free functions; the parsers
//! keep no state, so PHP's cached instances are not needed),
//! Name\Parser\GenericNameParser.php, Name\Parser\UnqualifiedNameParser.php
//! and Name\Parser\OptionallyQualifiedNameParser.php.
//!
//! PHP's generic parser is one PCRE pattern anchored at the offset; the
//! matcher below takes the same alternatives in the same order, with the
//! same backtracking: a doubled quote is an escaped quote unless the name
//! would then be left unterminated, in which case the last doubled quote's
//! first half closes it.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Name/Parser.php src/Schema/Name/Parsers.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/GenericNameParser.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/UnqualifiedNameParser.php
//! Upstream: doctrine/dbal src/Schema/Name/Parser/OptionallyQualifiedNameParser.php

use crate::exception::Exception;
use crate::schema::exception;

use super::{GenericName, Identifier, OptionallyQualifiedName, UnqualifiedName};

/// PHP's `AssertionError` (assertions on, as under PHPUnit and the PHP
/// CLI without an ini file).
pub const ASSERTION_ERROR: &str = "AssertionError";

/// One matched identifier: its kind and the text between the quotes.
enum Matched<'a> {
    Quoted { inner: &'a [u8], quote: u8 },
    Unquoted(&'a [u8]),
}

/// A quoted identifier opened at `start` (`open` at `input[start]`,
/// closed by `close`, a doubled `close` standing for one): the end of the
/// match and the text between the quotes.
fn match_quoted(input: &[u8], start: usize, close: u8) -> Option<(usize, &[u8])> {
    let mut i = start + 1;
    // The position of the first half of the last doubled quote taken.
    let mut last_pair = None;
    loop {
        while i < input.len() && input[i] != close {
            i += 1;
        }
        if i >= input.len() {
            // Unterminated: backtrack to the last doubled quote, whose
            // first half then closes the identifier.
            let pair = last_pair?;
            return Some((pair + 1, &input[start + 1..pair]));
        }
        if input.get(i + 1) == Some(&close) {
            last_pair = Some(i);
            i += 2;
            continue;
        }
        return Some((i + 1, &input[start + 1..i]));
    }
}

fn is_unquoted_byte(byte: u8) -> bool {
    !matches!(
        byte,
        b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b'.' | b'"' | b'`' | b'[' | b']'
    )
}

/// `IDENTIFIER_PATTERN` matched at `offset`: the end of the match.
fn match_identifier(input: &[u8], offset: usize) -> Option<(usize, Matched<'_>)> {
    match input.get(offset)? {
        b'"' => {
            match_quoted(input, offset, b'"').map(|(end, inner)| (end, Matched::Quoted { inner, quote: b'"' }))
        }
        b'`' => {
            match_quoted(input, offset, b'`').map(|(end, inner)| (end, Matched::Quoted { inner, quote: b'`' }))
        }
        b'[' => {
            match_quoted(input, offset, b']').map(|(end, inner)| (end, Matched::Quoted { inner, quote: b']' }))
        }
        _ => {
            let end = offset
                + input[offset..]
                    .iter()
                    .take_while(|&&byte| is_unquoted_byte(byte))
                    .count();
            (end > offset).then(|| (end, Matched::Unquoted(&input[offset..end])))
        }
    }
}

fn replace_doubled(inner: &[u8], quote: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(inner.len());
    let mut i = 0;
    while i < inner.len() {
        out.push(inner[i]);
        if inner[i] == quote && inner.get(i + 1) == Some(&quote) {
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

fn text(bytes: &[u8]) -> Result<String, Exception> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        Exception::new(
            Exception::UNSUPPORTED,
            "an identifier that is not UTF-8 after unquoting",
        )
    })
}

/// `GenericNameParser::parse($input)`: parses a generic SQL name.
pub fn parse_generic_name(input: &str) -> Result<GenericName, Exception> {
    let bytes = input.as_bytes();
    let length = bytes.len();
    let mut offset = 0;
    let mut identifiers = Vec::new();
    loop {
        if offset >= length {
            return Err(exception::expected_next_identifier());
        }
        let Some((end, matched)) = match_identifier(bytes, offset) else {
            return Err(exception::unable_to_parse_identifier(offset));
        };
        let identifier = match matched {
            Matched::Quoted { inner, quote } if !inner.is_empty() => {
                Identifier::quoted(&text(&replace_doubled(inner, quote))?)?
            }
            Matched::Unquoted(value) => Identifier::unquoted(&text(value)?)?,
            Matched::Quoted { .. } => {
                return Err(Exception::new(
                    ASSERTION_ERROR,
                    "assert(isset($matches['unquoted']) && strlen($matches['unquoted']) > 0)",
                ));
            }
        };
        identifiers.push(identifier);
        offset = end;
        if offset >= length {
            break;
        }
        let character = bytes[offset];
        if character != b'.' {
            return Err(exception::expected_dot(offset, character));
        }
        offset += 1;
    }
    let mut identifiers = identifiers.into_iter();
    let Some(first) = identifiers.next() else {
        return Err(exception::expected_next_identifier());
    };
    Ok(GenericName::new(first, identifiers.collect()))
}

/// `Parsers::parseUnqualifiedName($input)`.
pub fn parse_unqualified_name(input: &str) -> Result<UnqualifiedName, Exception> {
    let name = parse_generic_name(input)?;
    let identifiers = name.get_identifiers();
    if identifiers.len() > 1 {
        return Err(exception::invalid_unqualified_name(identifiers.len()));
    }
    Ok(UnqualifiedName::new(identifiers[0].clone()))
}

/// `Parsers::parseOptionallyQualifiedName($input)`.
pub fn parse_optionally_qualified_name(input: &str) -> Result<OptionallyQualifiedName, Exception> {
    let name = parse_generic_name(input)?;
    match name.get_identifiers() {
        [name] => Ok(OptionallyQualifiedName::new(name.clone(), None)),
        [qualifier, name] => Ok(OptionallyQualifiedName::new(
            name.clone(),
            Some(qualifier.clone()),
        )),
        identifiers => Err(exception::invalid_optionally_qualified_name(
            identifiers.len(),
        )),
    }
}
