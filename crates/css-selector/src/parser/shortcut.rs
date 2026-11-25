//! Port of Symfony\Component\CssSelector\Parser\Shortcut\*.php: fast
//! paths for the commonest selectors. Upstream's possessive quantifiers
//! become greedy ones: in each pattern the following item can never match
//! what the quantified class consumed, so the matches are the same.
//!
//! Upstream: symfony/css-selector Parser/Shortcut/ClassParser.php Parser/Shortcut/ElementParser.php Parser/Shortcut/EmptyStringParser.php Parser/Shortcut/HashParser.php

use std::sync::OnceLock;

use regex::bytes::{Captures, Regex};

use super::ParserInterface;
use crate::exception::Exception;
use crate::node::{ClassNode, ElementNode, HashNode, SelectorNode};
use crate::php::{or_null, trim};

fn compile(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(e) => panic!("invalid built-in pattern {pattern}: {e}"),
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the callers take an Option, as for other PHP groups"
)]
fn group<'a>(caps: &Captures<'a>, i: usize) -> Option<&'a [u8]> {
    // PHP lists an unmatched group as "".
    Some(caps.get(i).map_or(&b""[..], |m| m.as_bytes()))
}

/// `.class`, `element.class`, `ns|element.class`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassParser;

impl ParserInterface for ClassParser {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| compile(r"(?i-u)^(?:([a-z]+)\|)?([\w-]+|\*)?\.([\w-]+)$"));
        Ok(match re.captures(trim(source)) {
            Some(caps) => vec![SelectorNode::new(
                ClassNode::new(
                    ElementNode::new(
                        or_null(group(&caps, 1)).as_deref(),
                        or_null(group(&caps, 2)).as_deref(),
                    ),
                    group(&caps, 3).unwrap_or_default(),
                ),
                None,
            )],
            None => Vec::new(),
        })
    }
}

/// `element`, `*`, `ns|element`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ElementParser;

impl ParserInterface for ElementParser {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| compile(r"(?i-u)^(?:([a-z]+)\|)?([\w-]+|\*)$"));
        Ok(match re.captures(trim(source)) {
            Some(caps) => vec![SelectorNode::new(
                ElementNode::new(or_null(group(&caps, 1)).as_deref(), group(&caps, 2)),
                None,
            )],
            None => Vec::new(),
        })
    }
}

/// The empty selector, which matches every element.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyStringParser;

impl ParserInterface for EmptyStringParser {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        Ok(if source.is_empty() {
            vec![SelectorNode::new(ElementNode::new(None, Some(b"*")), None)]
        } else {
            Vec::new()
        })
    }
}

/// `#id`, `element#id`, `ns|element#id`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HashParser;

impl ParserInterface for HashParser {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| compile(r"(?i-u)^(?:([a-z]+)\|)?([\w-]+|\*)?#([\w-]+)$"));
        Ok(match re.captures(trim(source)) {
            Some(caps) => vec![SelectorNode::new(
                HashNode::new(
                    ElementNode::new(
                        or_null(group(&caps, 1)).as_deref(),
                        or_null(group(&caps, 2)).as_deref(),
                    ),
                    group(&caps, 3).unwrap_or_default(),
                ),
                None,
            )],
            None => Vec::new(),
        })
    }
}
