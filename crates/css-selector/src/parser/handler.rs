//! Port of Symfony\Component\CssSelector\Parser\Handler\*.php.
//!
//! Upstream: symfony/css-selector Parser/Handler/HandlerInterface.php Parser/Handler/CommentHandler.php Parser/Handler/HashHandler.php Parser/Handler/IdentifierHandler.php Parser/Handler/NumberHandler.php Parser/Handler/StringHandler.php Parser/Handler/WhitespaceHandler.php

use super::{Reader, Token, TokenStream, TokenType, TokenizerEscaping, TokenizerPatterns};
use crate::exception::Exception;

/// `HandlerInterface`: consumes what it recognises at the reader's
/// position and says whether it did.
pub trait HandlerInterface {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception>;
}

/// Comments are skipped, never pushed.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommentHandler;

impl HandlerInterface for CommentHandler {
    fn handle(&self, reader: &mut Reader, _stream: &mut TokenStream) -> Result<bool, Exception> {
        if reader.get_substring(2, 0) != b"/*" {
            return Ok(false);
        }
        match reader.get_offset(b"*/") {
            None => reader.move_to_end(),
            Some(offset) => reader.move_forward(offset + 2),
        }
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HashHandler {
    patterns: TokenizerPatterns,
    escaping: TokenizerEscaping,
}

impl HashHandler {
    pub fn new(patterns: TokenizerPatterns, escaping: TokenizerEscaping) -> Self {
        Self { patterns, escaping }
    }
}

impl HandlerInterface for HashHandler {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception> {
        let Some(found) = reader.find_pattern(self.patterns.get_hash_pattern()) else {
            return Ok(false);
        };
        let group = found.get(1).map(Vec::as_slice).unwrap_or_default();
        let value = self.escaping.escape_unicode(group);
        stream.push(Token::of(TokenType::Hash, &value, reader.get_position()));
        reader.move_forward(found[0].len());
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentifierHandler {
    patterns: TokenizerPatterns,
    escaping: TokenizerEscaping,
}

impl IdentifierHandler {
    pub fn new(patterns: TokenizerPatterns, escaping: TokenizerEscaping) -> Self {
        Self { patterns, escaping }
    }
}

impl HandlerInterface for IdentifierHandler {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception> {
        let Some(found) = reader.find_pattern(self.patterns.get_identifier_pattern()) else {
            return Ok(false);
        };
        let value = self.escaping.escape_unicode(&found[0]);
        stream.push(Token::of(
            TokenType::Identifier,
            &value,
            reader.get_position(),
        ));
        reader.move_forward(found[0].len());
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NumberHandler {
    patterns: TokenizerPatterns,
}

impl NumberHandler {
    pub fn new(patterns: TokenizerPatterns) -> Self {
        Self { patterns }
    }
}

impl HandlerInterface for NumberHandler {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception> {
        let Some(found) = reader.find_pattern(self.patterns.get_number_pattern()) else {
            return Ok(false);
        };
        stream.push(Token::of(
            TokenType::Number,
            &found[0],
            reader.get_position(),
        ));
        reader.move_forward(found[0].len());
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StringHandler {
    patterns: TokenizerPatterns,
    escaping: TokenizerEscaping,
}

impl StringHandler {
    pub fn new(patterns: TokenizerPatterns, escaping: TokenizerEscaping) -> Self {
        Self { patterns, escaping }
    }
}

impl HandlerInterface for StringHandler {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception> {
        let quote = reader.get_substring(1, 0).to_vec();
        let Some(pattern) = self.patterns.get_quoted_string_pattern(&quote) else {
            return Ok(false);
        };
        reader.move_forward(1);
        let Some(found) = reader.find_pattern(pattern) else {
            return Err(Exception::internal(format!(
                "Should have found at least an empty match at {}.",
                reader.get_position()
            )));
        };
        let length = found[0].len();
        let start = reader.get_position() as i64 - 1;
        // check unclosed strings
        if length as i64 == reader.get_remaining_length() {
            return Err(Exception::unclosed_string(start));
        }
        // check quotes pairs validity
        if quote != reader.get_substring(1, length) {
            return Err(Exception::unclosed_string(start));
        }
        let string = self.escaping.escape_unicode_and_new_line(&found[0]);
        stream.push(Token::of(TokenType::String, &string, reader.get_position()));
        reader.move_forward(length + 1);
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WhitespaceHandler;

impl HandlerInterface for WhitespaceHandler {
    fn handle(&self, reader: &mut Reader, stream: &mut TokenStream) -> Result<bool, Exception> {
        let Some(found) = reader.find_pattern(TokenizerPatterns::new().get_whitespace_pattern())
        else {
            return Ok(false);
        };
        stream.push(Token::of(
            TokenType::Whitespace,
            &found[0],
            reader.get_position(),
        ));
        reader.move_forward(found[0].len());
        Ok(true)
    }
}
