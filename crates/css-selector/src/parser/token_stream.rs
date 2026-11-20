//! Port of Symfony\Component\CssSelector\Parser\TokenStream.php.
//!
//! As upstream, only tokens read through a peek are recorded as used;
//! the `:scope` position check depends on that.
//!
//! Upstream: symfony/css-selector Parser/TokenStream.php

use super::Token;
use crate::exception::Exception;

#[derive(Debug, Clone, Default)]
pub struct TokenStream {
    tokens: Vec<Token>,
    used: Vec<Token>,
    cursor: usize,
    peeked: Option<Token>,
    peeking: bool,
}

impl TokenStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, token: Token) -> &mut Self {
        self.tokens.push(token);
        self
    }

    pub fn freeze(&mut self) -> &mut Self {
        self
    }

    /// The pushed tokens (upstream's tests read them by reflection).
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn get_next(&mut self) -> Result<Token, Exception> {
        if self.peeking {
            self.peeking = false;
            if let Some(peeked) = &self.peeked {
                self.used.push(peeked.clone());
                return Ok(peeked.clone());
            }
        }
        match self.tokens.get(self.cursor) {
            Some(token) => {
                self.cursor += 1;
                Ok(token.clone())
            }
            None => Err(Exception::internal("Unexpected token stream end.")),
        }
    }

    pub fn get_peek(&mut self) -> Result<Token, Exception> {
        if !self.peeking {
            self.peeked = Some(self.get_next()?);
            self.peeking = true;
        }
        match &self.peeked {
            Some(token) => Ok(token.clone()),
            None => Err(Exception::internal("Unexpected token stream end.")),
        }
    }

    pub fn get_used(&self) -> &[Token] {
        &self.used
    }

    pub fn get_next_identifier(&mut self) -> Result<Vec<u8>, Exception> {
        let next = self.get_next()?;
        if !next.is_identifier() {
            return Err(Exception::unexpected_token(b"identifier", &next));
        }
        Ok(next.get_value().unwrap_or_default().to_vec())
    }

    /// The next identifier, or `None` for `*`.
    pub fn get_next_identifier_or_star(&mut self) -> Result<Option<Vec<u8>>, Exception> {
        let next = self.get_next()?;
        if next.is_identifier() {
            return Ok(Some(next.get_value().unwrap_or_default().to_vec()));
        }
        if next.is_delimiter(&[b"*"]) {
            return Ok(None);
        }
        Err(Exception::unexpected_token(b"identifier or \"*\"", &next))
    }

    pub fn skip_whitespace(&mut self) -> Result<(), Exception> {
        if self.get_peek()?.is_whitespace() {
            self.get_next()?;
        }
        Ok(())
    }
}
