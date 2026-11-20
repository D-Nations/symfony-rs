//! Port of Symfony\Component\CssSelector\Parser\Parser.php and
//! ParserInterface.php: CSS selector source to [`SelectorNode`]s.
//!
//! Upstream: symfony/css-selector Parser/Parser.php Parser/ParserInterface.php

pub mod handler;
mod reader;
pub mod shortcut;
mod token;
mod token_stream;
mod tokenizer;

use std::cell::Cell;

pub use reader::Reader;
pub use token::{Token, TokenType};
pub use token_stream::TokenStream;
pub use tokenizer::{Tokenizer, TokenizerEscaping, TokenizerPatterns};

use crate::exception::Exception;
use crate::node::{
    AttributeNode, ClassNode, CombinedSelectorNode, ElementNode, FunctionNode, HashNode,
    MatchingNode, NegationNode, Node, PseudoNode, RelationNode, SelectorNode,
    SpecificityAdjustmentNode,
};
use crate::php::{cat, strtolower, trim, truthy};

/// `ParserInterface`.
pub trait ParserInterface {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception>;
}

const HAS_NESTING_LIMIT: usize = 16;
const NESTING_LIMIT: usize = 16;

/// The CSS selector parser. The nesting depths are kept between calls,
/// as upstream's properties are (and restored when a call fails).
pub struct Parser {
    tokenizer: Tokenizer,
    has_nesting_depth: Cell<usize>,
    nesting_depth: Cell<usize>,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Restores a depth counter when a nested parse returns, as upstream's
/// `finally` blocks do.
struct DepthGuard<'a>(&'a Cell<usize>);

impl<'a> DepthGuard<'a> {
    fn enter(depth: &'a Cell<usize>) -> Self {
        depth.set(depth.get() + 1);
        Self(depth)
    }
}

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

/// Upstream passes a null attribute name on to `AttributeNode`, whose
/// parameter is a string, so PHP throws a `TypeError` there; only
/// `[*|=value]` gets that far without a name.
fn required(attribute: Option<&[u8]>) -> Result<&[u8], Exception> {
    attribute.ok_or_else(|| {
        Exception::new(
            crate::exception::Kind::TypeError,
            "Symfony\\Component\\CssSelector\\Node\\AttributeNode::__construct(): Argument #3 ($attribute) must be of type string, null given",
        )
    })
}

/// `(int) $string` behind `is_numeric()`, as `parseSeries()`'s `$int`.
fn series_int(string: &[u8]) -> Result<i64, Exception> {
    if !php_value::numeric::is_numeric(string) {
        return Err(Exception::string_as_function_argument());
    }
    Ok(php_value::numeric::string_to_int(string))
}

impl Parser {
    pub fn new(tokenizer: Option<Tokenizer>) -> Self {
        Self {
            tokenizer: tokenizer.unwrap_or_default(),
            has_nesting_depth: Cell::new(0),
            nesting_depth: Cell::new(0),
        }
    }

    /// The `:has()` depth (upstream's tests read and set it by reflection).
    pub fn has_nesting_depth(&self) -> usize {
        self.has_nesting_depth.get()
    }

    pub fn set_has_nesting_depth(&self, depth: usize) {
        self.has_nesting_depth.set(depth);
    }

    /// The `:is()` / `:where()` depth (likewise).
    pub fn nesting_depth(&self) -> usize {
        self.nesting_depth.get()
    }

    pub fn set_nesting_depth(&self, depth: usize) {
        self.nesting_depth.set(depth);
    }

    /// Parses the arguments of `:nth-child()` and friends into `[a, b]`.
    pub fn parse_series(tokens: &[Token]) -> Result<(i64, i64), Exception> {
        if tokens.iter().any(Token::is_string) {
            return Err(Exception::string_as_function_argument());
        }
        let joined: Vec<u8> = tokens
            .iter()
            .flat_map(|t| t.get_value().unwrap_or_default().iter().copied())
            .collect();
        let joined = trim(&joined);

        match joined {
            b"odd" => return Ok((2, 1)),
            b"even" => return Ok((2, 0)),
            b"n" => return Ok((1, 0)),
            _ if !joined.contains(&b'n') => return Ok((0, series_int(joined)?)),
            _ => {}
        }

        let split: Vec<&[u8]> = joined.split(|&b| b == b'n').collect();
        let first = split.first().copied().unwrap_or_default();
        let a = if truthy(first) {
            if first == b"-" || first == b"+" {
                series_int(&cat(&[first, b"1"]))?
            } else {
                series_int(first)?
            }
        } else {
            1
        };
        let b = match split.get(1) {
            Some(second) if truthy(second) => series_int(second)?,
            _ => 0,
        };
        Ok((a, b))
    }

    fn parse_selector_list(
        &self,
        stream: &mut TokenStream,
        is_argument: bool,
    ) -> Result<Vec<SelectorNode>, Exception> {
        stream.skip_whitespace()?;
        let mut selectors = Vec::new();
        loop {
            if is_argument && stream.get_peek()?.is_delimiter(&[b")"]) {
                break;
            }
            selectors.push(self.parser_selector_node(stream, is_argument, false)?);
            if stream.get_peek()?.is_delimiter(&[b","]) {
                stream.get_next()?;
                stream.skip_whitespace()?;
            } else {
                break;
            }
        }
        Ok(selectors)
    }

    fn parser_selector_node(
        &self,
        stream: &mut TokenStream,
        is_argument: bool,
        inside_relative_selector: bool,
    ) -> Result<SelectorNode, Exception> {
        let (mut result, mut pseudo_element) =
            self.parse_simple_selector(stream, false, is_argument, inside_relative_selector)?;

        loop {
            stream.skip_whitespace()?;
            let peek = stream.get_peek()?;
            if peek.is_file_end()
                || peek.is_delimiter(&[b","])
                || (is_argument && peek.is_delimiter(&[b")"]))
            {
                break;
            }
            if let Some(pseudo) = &pseudo_element {
                return Err(Exception::pseudo_element_found(
                    pseudo,
                    b"not at the end of a selector",
                ));
            }
            let combinator = if peek.is_delimiter(&[b"+", b">", b"~"]) {
                let value = stream.get_next()?.get_value().unwrap_or_default().to_vec();
                stream.skip_whitespace()?;
                value
            } else {
                b" ".to_vec()
            };
            let (next_selector, next_pseudo) =
                self.parse_simple_selector(stream, false, is_argument, inside_relative_selector)?;
            pseudo_element = next_pseudo;
            result = CombinedSelectorNode::new(result, &combinator, next_selector).into();
        }

        Ok(SelectorNode::new(result, pseudo_element.as_deref()))
    }

    fn parse_relative_selector(
        &self,
        stream: &mut TokenStream,
    ) -> Result<Vec<(Vec<u8>, Node)>, Exception> {
        if self.has_nesting_depth.get() >= HAS_NESTING_LIMIT {
            return Err(Exception::nested_has());
        }
        let _depth = DepthGuard::enter(&self.has_nesting_depth);

        let mut arguments = Vec::new();
        loop {
            stream.skip_whitespace()?;
            let mut peek = stream.get_peek()?;
            let combinator = if peek.is_delimiter(&[b"+", b">", b"~"]) {
                let value = stream.get_next()?.get_value().unwrap_or_default().to_vec();
                stream.skip_whitespace()?;
                peek = stream.get_peek()?;
                value
            } else {
                b" ".to_vec()
            };
            if peek.is_string() || peek.is_number() {
                let next = stream.get_next()?;
                return Err(Exception::unexpected_token(b"an argument", &next));
            }
            let selector = self.parser_selector_node(stream, true, true)?;
            if let Some(pseudo) = selector.get_pseudo_element() {
                return Err(Exception::pseudo_element_found(pseudo, b"inside :has()"));
            }
            arguments.push((combinator, Node::Selector(selector)));
            if stream.get_peek()?.is_delimiter(&[b","]) {
                stream.get_next()?;
                continue;
            }
            break;
        }

        let next = stream.get_next()?;
        if !next.is_delimiter(&[b")"]) {
            return Err(Exception::unexpected_token(b"\")\"", &next));
        }
        Ok(arguments)
    }

    /// Parses the next simple selector (hash, class, pseudo, negation);
    /// returns it with its pseudo-element.
    fn parse_simple_selector(
        &self,
        stream: &mut TokenStream,
        inside_negation: bool,
        is_argument: bool,
        inside_relative_selector: bool,
    ) -> Result<(Node, Option<Vec<u8>>), Exception> {
        stream.skip_whitespace()?;

        let selector_start = stream.get_used().len();
        let mut result: Node = Self::parse_element_node(stream)?.into();
        let mut pseudo_element: Option<Vec<u8>> = None;

        loop {
            let peek = stream.get_peek()?;
            if peek.is_whitespace()
                || peek.is_file_end()
                || peek.is_delimiter(&[b",", b"+", b">", b"~"])
                || (is_argument && peek.is_delimiter(&[b")"]))
            {
                break;
            }
            if let Some(pseudo) = &pseudo_element {
                return Err(Exception::pseudo_element_found(
                    pseudo,
                    b"not at the end of a selector",
                ));
            }

            if peek.is_hash() {
                let next = stream.get_next()?;
                result = HashNode::new(result, next.get_value().unwrap_or_default()).into();
            } else if peek.is_delimiter(&[b"."]) {
                stream.get_next()?;
                let name = stream.get_next_identifier()?;
                result = ClassNode::new(result, &name).into();
            } else if peek.is_delimiter(&[b"["]) {
                stream.get_next()?;
                result = Self::parse_attribute_node(result, stream)?.into();
            } else if peek.is_delimiter(&[b":"]) {
                stream.get_next()?;

                if stream.get_peek()?.is_delimiter(&[b":"]) {
                    stream.get_next()?;
                    pseudo_element = Some(stream.get_next_identifier()?);
                    continue;
                }

                let identifier = stream.get_next_identifier()?;
                let lower = strtolower(&identifier);
                if [&b"first-line"[..], b"first-letter", b"before", b"after"]
                    .contains(&lower.as_slice())
                {
                    // Special case: CSS 2.1 pseudo-elements can have a
                    // single ':'. Any new pseudo-element must have two.
                    pseudo_element = Some(identifier);
                    continue;
                }

                if !stream.get_peek()?.is_delimiter(&[b"("]) {
                    result = PseudoNode::new(result, &identifier).into();
                    if result.to_php_string() == b"Pseudo[Element[*]:scope]" {
                        Self::check_scope_position(stream, inside_relative_selector)?;
                    }
                    continue;
                }

                stream.get_next()?;
                stream.skip_whitespace()?;

                if lower == b"not" {
                    if inside_negation {
                        return Err(Exception::nested_not());
                    }
                    let (argument, argument_pseudo_element) =
                        self.parse_simple_selector(stream, true, true, false)?;
                    let next = stream.get_next()?;
                    if let Some(pseudo) = argument_pseudo_element {
                        return Err(Exception::pseudo_element_found(&pseudo, b"inside :not()"));
                    }
                    if !next.is_delimiter(&[b")"]) {
                        return Err(Exception::unexpected_token(b"\")\"", &next));
                    }
                    result = NegationNode::new(result, argument).into();
                } else if lower == b"is" || lower == b"where" {
                    let selectors = self.parse_nested_selector_list(stream, &lower)?;
                    let next = stream.get_next()?;
                    if !next.is_delimiter(&[b")"]) {
                        return Err(Exception::unexpected_token(b"\")\"", &next));
                    }
                    let selectors = selectors.into_iter().map(Node::Selector).collect();
                    result = if lower == b"is" {
                        MatchingNode::new(result, selectors).into()
                    } else {
                        SpecificityAdjustmentNode::new(result, selectors).into()
                    };
                } else if lower == b"has" {
                    let arguments = self.parse_relative_selector(stream)?;
                    result = RelationNode::new(result, arguments).into();
                } else {
                    let mut arguments = Vec::new();
                    let next = loop {
                        stream.skip_whitespace()?;
                        let next = stream.get_next()?;
                        if next.is_identifier()
                            || next.is_string()
                            || next.is_number()
                            || next.is_delimiter(&[b"+", b"-"])
                        {
                            arguments.push(next);
                        } else if next.is_delimiter(&[b")"]) {
                            break next;
                        } else {
                            return Err(Exception::unexpected_token(b"an argument", &next));
                        }
                    };
                    if arguments.is_empty() {
                        return Err(Exception::unexpected_token(b"at least one argument", &next));
                    }
                    result = FunctionNode::new(result, &identifier, arguments).into();
                }
            } else {
                return Err(Exception::unexpected_token(b"selector", &peek));
            }
        }

        if stream.get_used().len() == selector_start {
            let peek = stream.get_peek()?;
            return Err(Exception::unexpected_token(b"selector", &peek));
        }

        Ok((result, pseudo_element))
    }

    /// `:scope` only at the start of a selector (or of a `:has()`
    /// argument), judged from the used tokens as upstream does.
    fn check_scope_position(
        stream: &TokenStream,
        inside_relative_selector: bool,
    ) -> Result<(), Exception> {
        let used = stream.get_used();
        let count = used.len();
        let separators: &[&[u8]] = if inside_relative_selector {
            &[b",", b"(", b">", b"+", b"~"]
        } else {
            &[b","]
        };
        let at = |i: usize| &used[i];
        let ok = count == 2
            || (count == 3 && at(0).is_whitespace())
            || (count >= 3 && at(count - 3).is_delimiter(separators))
            || (count >= 4
                && at(count - 3).is_whitespace()
                && at(count - 4).is_delimiter(separators));
        if ok {
            Ok(())
        } else {
            Err(Exception::not_at_the_start_of_a_selector(b"scope"))
        }
    }

    fn parse_nested_selector_list(
        &self,
        stream: &mut TokenStream,
        identifier: &[u8],
    ) -> Result<Vec<SelectorNode>, Exception> {
        if self.nesting_depth.get() >= NESTING_LIMIT {
            return Err(Exception::syntax(cat(&[
                b"Got too deeply nested :",
                identifier,
                b"().",
            ])));
        }
        let _depth = DepthGuard::enter(&self.nesting_depth);
        self.parse_selector_list(stream, true)
    }

    fn parse_element_node(stream: &mut TokenStream) -> Result<ElementNode, Exception> {
        let peek = stream.get_peek()?;
        let (namespace, element) = if peek.is_identifier() || peek.is_delimiter(&[b"*"]) {
            let namespace = if peek.is_identifier() {
                stream.get_next()?.get_value().map(<[u8]>::to_vec)
            } else {
                stream.get_next()?;
                None
            };
            if stream.get_peek()?.is_delimiter(&[b"|"]) {
                stream.get_next()?;
                (namespace, stream.get_next_identifier_or_star()?)
            } else {
                (None, namespace)
            }
        } else {
            (None, None)
        };
        Ok(ElementNode::new(namespace.as_deref(), element.as_deref()))
    }

    fn parse_attribute_node(
        selector: Node,
        stream: &mut TokenStream,
    ) -> Result<AttributeNode, Exception> {
        stream.skip_whitespace()?;
        let mut attribute = stream.get_next_identifier_or_star()?;

        if attribute.is_none() && !stream.get_peek()?.is_delimiter(&[b"|"]) {
            let peek = stream.get_peek()?;
            return Err(Exception::unexpected_token(b"\"|\"", &peek));
        }

        let namespace;
        let mut operator: Option<Vec<u8>>;
        if stream.get_peek()?.is_delimiter(&[b"|"]) {
            stream.get_next()?;
            if stream.get_peek()?.is_delimiter(&[b"="]) {
                namespace = None;
                stream.get_next()?;
                operator = Some(b"|=".to_vec());
            } else {
                namespace = attribute;
                attribute = Some(stream.get_next_identifier()?);
                operator = None;
            }
        } else {
            namespace = None;
            operator = None;
        }
        if operator.is_none() {
            stream.skip_whitespace()?;
            let next = stream.get_next()?;
            if next.is_delimiter(&[b"]"]) {
                return Ok(AttributeNode::new(
                    selector,
                    namespace.as_deref(),
                    required(attribute.as_deref())?,
                    b"exists",
                    None,
                ));
            } else if next.is_delimiter(&[b"="]) {
                operator = Some(b"=".to_vec());
            } else if next.is_delimiter(&[b"^", b"$", b"*", b"~", b"|", b"!"])
                && stream.get_peek()?.is_delimiter(&[b"="])
            {
                operator = Some(cat(&[next.get_value().unwrap_or_default(), b"="]));
                stream.get_next()?;
            } else {
                return Err(Exception::unexpected_token(b"operator", &next));
            }
        }

        stream.skip_whitespace()?;
        let mut value = stream.get_next()?;
        if value.is_number() {
            // if the value is a number, it's casted into a string
            value = Token::new(
                Some(TokenType::String),
                Some(value.get_value().unwrap_or_default()),
                value.get_position(),
            );
        }
        if !(value.is_identifier() || value.is_string()) {
            return Err(Exception::unexpected_token(b"string or identifier", &value));
        }

        stream.skip_whitespace()?;
        let next = stream.get_next()?;
        if !next.is_delimiter(&[b"]"]) {
            return Err(Exception::unexpected_token(b"\"]\"", &next));
        }

        Ok(AttributeNode::new(
            selector,
            namespace.as_deref(),
            required(attribute.as_deref())?,
            operator.as_deref().unwrap_or_default(),
            value.get_value(),
        ))
    }
}

impl ParserInterface for Parser {
    fn parse(&self, source: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        let mut reader = Reader::new(source);
        let mut stream = self.tokenizer.tokenize(&mut reader)?;
        self.parse_selector_list(&mut stream, false)
    }
}
