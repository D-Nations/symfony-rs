//! Port of Doctrine\DBAL\SQL\Parser.php and its `Parser\Visitor.php`
//! interface.
//!
//! The SQL parser that focuses on identifying prepared statement
//! parameters. It implements parsing other tokens like string literals and
//! comments only as a way to not confuse their contents with the parameter
//! placeholders.
//!
//! Upstream builds one PCRE pattern and calls `preg_match()` with `\G` at
//! each offset. The matchers below are written by hand to give the same
//! token at each offset: the first alternative that matches wins, as in
//! PCRE's ordered alternation. Lookbehinds see the bytes before the offset,
//! as `preg_match()` with an offset does. Bytes, not characters: the
//! pattern has no `u` flag. Each token is checked against real PHP on the
//! upstream cases and on generated statements (`tests/parser_vectors.rs`).
//!
//! `RegularExpressionError` (a PCRE error such as a backtrack limit) has
//! no equivalent: these matchers do not backtrack, so `parse()` cannot
//! fail on its own. See README.md, Differences.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/SQL/Parser.php src/SQL/Parser/Visitor.php

/// SQL parser visitor.
///
/// Each method may fail; the first error stops `Parser::parse()` and is
/// returned, as an exception thrown by a PHP visitor leaves `parse()`.
pub trait Visitor {
    type Error;

    /// Accepts an SQL fragment containing a positional parameter
    fn accept_positional_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error>;

    /// Accepts an SQL fragment containing a named parameter
    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error>;

    /// Accepts other SQL fragments
    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error>;
}

/// `SPECIAL_CHARS`: `:\?\'"`\\[\\-\\/` in PHP, the class `[:?'"`\[\-/]`.
fn is_special(byte: u8) -> bool {
    matches!(byte, b':' | b'?' | b'\'' | b'"' | b'`' | b'[' | b'-' | b'/')
}

/// PCRE's `\w` without the `u` flag (PHP's built-in C tables).
fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Named,
    Positional,
    Other,
}

/// The parser of one SQL dialect: `MySQL` string escaping (a backslash
/// escapes the next byte) or ANSI (a quote is escaped by doubling it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parser {
    my_sql_string_escaping: bool,
}

impl Parser {
    pub fn new(my_sql_string_escaping: bool) -> Self {
        Self {
            my_sql_string_escaping,
        }
    }

    /// Parses the given SQL statement
    pub fn parse<V: Visitor + ?Sized>(self, sql: &[u8], visitor: &mut V) -> Result<(), V::Error> {
        let mut offset = 0;
        while offset < sql.len() {
            let (token, length) = self.token(sql, offset);
            let fragment = &sql[offset..offset + length];
            match token {
                Token::Named => visitor.accept_named_parameter(fragment)?,
                Token::Positional => visitor.accept_positional_parameter(fragment)?,
                Token::Other => visitor.accept_other(fragment)?,
            }
            offset += length;
        }
        Ok(())
    }

    /// `(?P<named>…)|(?P<positional>…)|(?P<other>sqlPattern|SPECIAL)` at
    /// `offset`, which is below `sql.len()`. Every byte is either OTHER or
    /// SPECIAL, so some alternative always matches at least one byte.
    fn token(self, sql: &[u8], offset: usize) -> (Token, usize) {
        if let Some(length) = named_parameter(sql, offset) {
            return (Token::Named, length);
        }
        if positional_parameter(sql, offset) {
            return (Token::Positional, 1);
        }
        let length = self
            .string_literal(sql, offset, b'\'')
            .or_else(|| self.string_literal(sql, offset, b'"'))
            .or_else(|| backtick_identifier(sql, offset))
            .or_else(|| bracket_identifier(sql, offset))
            .or_else(|| multichar(sql, offset))
            .or_else(|| one_line_comment(sql, offset))
            .or_else(|| multi_line_comment(sql, offset))
            .or_else(|| other(sql, offset))
            // SPECIAL: one byte of the class.
            .unwrap_or(1);
        (Token::Other, length)
    }

    fn string_literal(self, sql: &[u8], offset: usize, delimiter: u8) -> Option<usize> {
        if self.my_sql_string_escaping {
            my_sql_string_literal(sql, offset, delimiter)
        } else {
            ansi_sql_string_literal(sql, offset, delimiter)
        }
    }
}

/// `NAMED_PARAMETER`: `:[a-zA-Z0-9_]+`.
fn named_parameter(sql: &[u8], offset: usize) -> Option<usize> {
    if sql[offset] != b':' {
        return None;
    }
    let length = sql[offset + 1..]
        .iter()
        .take_while(|&&b| is_word(b))
        .count();
    (length > 0).then_some(1 + length)
}

/// `POSITIONAL_PARAMETER`: `(?<!\?)\?(?!\?)`.
fn positional_parameter(sql: &[u8], offset: usize) -> bool {
    sql[offset] == b'?'
        && (offset == 0 || sql[offset - 1] != b'?')
        && sql.get(offset + 1) != Some(&b'?')
}

/// `<d>((\\.)|(?![<d>\\]).)*<d>`, with `s`: a backslash escapes any next
/// byte. Without a closing delimiter there is no match: no shorter
/// repetition can end at a delimiter, since the second branch never
/// consumes one and the first consumes it only after a backslash.
fn my_sql_string_literal(sql: &[u8], offset: usize, delimiter: u8) -> Option<usize> {
    if sql[offset] != delimiter {
        return None;
    }
    let mut i = offset + 1;
    while let Some(&byte) = sql.get(i) {
        if byte == delimiter {
            return Some(i + 1 - offset);
        }
        if byte == b'\\' {
            if i + 1 >= sql.len() {
                return None;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

/// `<d>[^<d>]*<d>`.
fn ansi_sql_string_literal(sql: &[u8], offset: usize, delimiter: u8) -> Option<usize> {
    delimited(sql, offset, delimiter, delimiter)
}

/// `` BACKTICK_IDENTIFIER ``: `` `[^`]*` ``.
fn backtick_identifier(sql: &[u8], offset: usize) -> Option<usize> {
    delimited(sql, offset, b'`', b'`')
}

/// `BRACKET_IDENTIFIER`: `(?<!\b(?i:ARRAY))\[(?:[^\]])*\]`.
fn bracket_identifier(sql: &[u8], offset: usize) -> Option<usize> {
    if sql[offset] != b'[' || after_array_keyword(sql, offset) {
        return None;
    }
    delimited(sql, offset, b'[', b']')
}

/// The lookbehind `\b(?i:ARRAY)` ending at `offset`: the five bytes before
/// it spell ARRAY in any case, and a word boundary precedes them (the `A`
/// is a word byte, so the byte before it, if any, is not).
fn after_array_keyword(sql: &[u8], offset: usize) -> bool {
    let Some(start) = offset.checked_sub(5) else {
        return false;
    };
    sql[start..offset].eq_ignore_ascii_case(b"ARRAY") && (start == 0 || !is_word(sql[start - 1]))
}

/// `open [^close]* close`.
fn delimited(sql: &[u8], offset: usize, open: u8, close: u8) -> Option<usize> {
    if sql[offset] != open {
        return None;
    }
    let rest = &sql[offset + 1..];
    rest.iter().position(|&b| b == close).map(|end| end + 2)
}

/// `MULTICHAR`: `:{2,}`.
fn multichar(sql: &[u8], offset: usize) -> Option<usize> {
    let length = sql[offset..].iter().take_while(|&&b| b == b':').count();
    (length >= 2).then_some(length)
}

/// `ONE_LINE_COMMENT`: `--[^\r\n]*`.
fn one_line_comment(sql: &[u8], offset: usize) -> Option<usize> {
    if !sql[offset..].starts_with(b"--") {
        return None;
    }
    let rest = &sql[offset + 2..];
    let length = rest
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(rest.len());
    Some(2 + length)
}

/// `MULTI_LINE_COMMENT`: `/\*([^*]+|\*+[^/*])*\**\*/`, which ends at the
/// first `*/` after the opening `/*` (the repetition cannot consume a `*`
/// followed by `/`). Without one there is no match.
fn multi_line_comment(sql: &[u8], offset: usize) -> Option<usize> {
    if !sql[offset..].starts_with(b"/*") {
        return None;
    }
    let body = offset + 2;
    sql[body..]
        .windows(2)
        .position(|pair| pair == b"*/")
        .map(|end| body + end + 2 - offset)
}

/// `OTHER`: `[^SPECIAL_CHARS]+`.
fn other(sql: &[u8], offset: usize) -> Option<usize> {
    let length = sql[offset..]
        .iter()
        .take_while(|&&b| !is_special(b))
        .count();
    (length > 0).then_some(length)
}
