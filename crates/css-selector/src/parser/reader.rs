//! Port of Symfony\Component\CssSelector\Parser\Reader.php.
//!
//! Upstream: symfony/css-selector Parser/Reader.php

use regex::bytes::Regex;

use crate::php::{strpos, substr};

/// A cursor over the selector source (bytes).
#[derive(Debug, Clone)]
pub struct Reader {
    source: Vec<u8>,
    length: usize,
    position: usize,
}

impl Reader {
    pub fn new(source: &[u8]) -> Self {
        Self {
            source: source.to_vec(),
            length: source.len(),
            position: 0,
        }
    }

    pub fn is_eof(&self) -> bool {
        self.position >= self.length
    }

    pub fn get_position(&self) -> usize {
        self.position
    }

    /// Sets the position, as upstream's `ReaderTest` does by reflection.
    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    pub fn get_remaining_length(&self) -> i64 {
        let length = i64::try_from(self.length).unwrap_or(i64::MAX);
        length - i64::try_from(self.position).unwrap_or(i64::MAX)
    }

    pub fn get_substring(&self, length: usize, offset: usize) -> &[u8] {
        substr(&self.source, self.position.saturating_add(offset), length)
    }

    /// The offset of `string` from the current position, or `None`
    /// (PHP's `false`).
    pub fn get_offset(&self, string: &[u8]) -> Option<usize> {
        strpos(&self.source, string, self.position).map(|p| p - self.position)
    }

    /// `preg_match()` from the current position, where a leading `^`
    /// anchors at the position (upstream turns it into `\G`): the matched
    /// groups as PHP lists them (unmatched groups `""`, trailing ones
    /// dropped), or `None`.
    pub fn find_pattern(&self, pattern: &Regex) -> Option<Vec<Vec<u8>>> {
        let rest = self.source.get(self.position..)?;
        let captures = pattern.captures(rest)?;
        let mut groups: Vec<Option<&[u8]>> =
            captures.iter().map(|g| g.map(|m| m.as_bytes())).collect();
        while groups.len() > 1 && groups.last().is_some_and(Option::is_none) {
            groups.pop();
        }
        Some(
            groups
                .into_iter()
                .map(|g| g.unwrap_or_default().to_vec())
                .collect(),
        )
    }

    pub fn move_forward(&mut self, length: usize) {
        self.position += length;
    }

    pub fn move_to_end(&mut self) {
        self.position = self.length;
    }
}
