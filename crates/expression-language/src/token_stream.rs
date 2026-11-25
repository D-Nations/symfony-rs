//! Port of Symfony\Component\ExpressionLanguage\TokenStream.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language TokenStream.php

use std::fmt;

use php_value::PhpString;

use crate::{
    syntax_error::Error,
    token::{Token, TokenType},
};

/// The tokens of one expression, and a pointer to the current one.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenStream {
    tokens: Vec<Token>,
    expression: PhpString,
    position: usize,
}

impl TokenStream {
    /// A stream on `tokens`, which must hold at least one token (the
    /// lexer always ends them with an end-of-expression token).
    pub fn new(tokens: Vec<Token>, expression: impl Into<PhpString>) -> Self {
        let mut tokens = tokens;
        if tokens.is_empty() {
            tokens.push(Token {
                kind: TokenType::Eof,
                value: php_value::Value::Null,
                cursor: None,
            });
        }
        Self {
            tokens,
            expression: expression.into(),
            position: 0,
        }
    }

    /// `$stream->current`.
    pub fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    /// The tokens.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Moves to the next token; at the end, "Unexpected end of
    /// expression.".
    #[expect(
        clippy::should_implement_trait,
        reason = "upstream's name; it fails at the end instead of ending"
    )]
    pub fn next(&mut self) -> Result<(), Error> {
        if self.position + 1 >= self.tokens.len() {
            return Err(Error::syntax(
                "Unexpected end of expression.",
                self.current().cursor(),
                self.expression.as_bytes(),
            ));
        }
        self.position += 1;
        Ok(())
    }

    /// Checks the current token's type (and value), then moves on.
    pub fn expect(
        &mut self,
        kind: TokenType,
        value: Option<&str>,
        message: Option<&str>,
    ) -> Result<(), Error> {
        let token = self.current();
        if !token.test(kind, value) {
            let mut text = Vec::new();
            if let Some(m) = message.filter(|m| !m.is_empty() && *m != "0") {
                text.extend_from_slice(m.as_bytes());
                text.extend_from_slice(b". ");
            }
            text.extend_from_slice(b"Unexpected token \"");
            text.extend_from_slice(token.kind.as_str().as_bytes());
            text.extend_from_slice(b"\" of value \"");
            text.extend_from_slice(token.value_string().as_bytes());
            text.extend_from_slice(b"\" (\"");
            text.extend_from_slice(kind.as_str().as_bytes());
            text.extend_from_slice(b"\" expected");
            if let Some(v) = value.filter(|v| !v.is_empty() && *v != "0") {
                text.extend_from_slice(format!(" with value \"{v}\"").as_bytes());
            }
            text.extend_from_slice(b").");
            return Err(Error::syntax(
                text,
                token.cursor(),
                self.expression.as_bytes(),
            ));
        }
        self.next()
    }

    /// `isEOF()`.
    pub fn is_eof(&self) -> bool {
        self.current().kind == TokenType::Eof
    }

    /// `getExpression()`.
    pub fn expression(&self) -> &PhpString {
        &self.expression
    }
}

/// `__toString()`: the tokens, one per line.
impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, token) in self.tokens.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{token}")?;
        }
        Ok(())
    }
}
