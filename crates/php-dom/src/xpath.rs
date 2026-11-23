//! `DOMXPath::evaluate()` for the location paths Symfony's XML loaders
//! build (QtFileLoader's `//TS/context/name[text()="…"]`), over a
//! [`Document`]. Written from PHP 8.4.27's observed answers on libxml
//! 2.9.14 (symfony-translation's recorded `evaluate` cases) and XPath 1.0.
//!
//! The subset: unions (`|`) of absolute (`/`, `//`) and relative location
//! paths; steps on the child axis (`/`) and its `//` abbreviation; node
//! tests `*`, an unprefixed name (an element with that local name and no
//! namespace), `text()` (text and CDATA nodes) and `node()`; predicates
//! `[text() = "literal"]`, `[@name]` and `[n]` (position on the child
//! axis). A relative path starts at the document element, as
//! `DOMXPath::evaluate()` without a context node does. The result is a node
//! set in document order.
//!
//! Anything else is [`XPathError::Refused`]: it may be valid XPath that
//! this crate does not evaluate. [`XPathError::Invalid`] is an expression
//! that no completion makes valid (an unterminated literal, an empty
//! expression, input ending inside a step or predicate, bytes that are not
//! UTF-8), for which PHP's `evaluate()` warns and returns `false`. As in
//! PHP, the expression ends at its first NUL byte.
//!
//! Upstream: none

use std::collections::HashSet;
use std::fmt;

use crate::dom::{Document, NodeId, NodeType};

/// Why [`evaluate`] gives no node set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XPathError {
    /// PHP's `false`: libxml rejects the expression.
    Invalid,
    /// An expression outside the subset; the text says which part.
    Refused(String),
}

impl fmt::Display for XPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XPathError::Invalid => f.write_str("Invalid expression"),
            XPathError::Refused(what) => write!(f, "XPath outside the evaluated subset: {what}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Slash,
    DoubleSlash,
    Pipe,
    LBracket,
    RBracket,
    LParen,
    RParen,
    At,
    Equals,
    Star,
    Name(String),
    Literal(String),
    Number(String),
    /// Any other operator or character XPath knows (`.`, `::`, `!=`, …).
    Other(String),
}

fn tokenize(expr: &str) -> Result<Vec<Token>, XPathError> {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '/' if chars.get(i + 1) == Some(&'/') => {
                out.push(Token::DoubleSlash);
                i += 2;
            }
            '/' => {
                out.push(Token::Slash);
                i += 1;
            }
            '|' => {
                out.push(Token::Pipe);
                i += 1;
            }
            '[' => {
                out.push(Token::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Token::RBracket);
                i += 1;
            }
            '(' => {
                out.push(Token::LParen);
                i += 1;
            }
            ')' => {
                out.push(Token::RParen);
                i += 1;
            }
            '@' => {
                out.push(Token::At);
                i += 1;
            }
            '=' => {
                out.push(Token::Equals);
                i += 1;
            }
            '*' => {
                out.push(Token::Star);
                i += 1;
            }
            '"' | '\'' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|&q| q == c)
                    .ok_or(XPathError::Invalid)?;
                out.push(Token::Literal(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
            }
            c if c.is_ascii_digit() => {
                let len = chars[i..]
                    .iter()
                    .take_while(|d| d.is_ascii_digit() || **d == '.')
                    .count();
                out.push(Token::Number(chars[i..i + len].iter().collect()));
                i += len;
            }
            c if is_name_start(c) => {
                let len = chars[i..].iter().take_while(|&&n| is_name_char(n)).count();
                let name: String = chars[i..i + len].iter().collect();
                i += len;
                // a QName (`p:name`) or an axis (`child::`) is refused
                if chars.get(i) == Some(&':') {
                    return Err(XPathError::Refused(format!("{name}:")));
                }
                out.push(Token::Name(name));
            }
            other => {
                out.push(Token::Other(other.to_string()));
                i += 1;
            }
        }
    }
    Ok(out)
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '\u{B7}')
}

#[derive(Clone, Debug)]
enum NodeTest {
    Any,
    Name(String),
    Text,
    Node,
}

#[derive(Clone, Debug)]
enum Predicate {
    TextEquals(String),
    HasAttribute(String),
    Position(usize),
}

#[derive(Clone, Debug)]
struct Step {
    /// `//` before the step: every descendant instead of every child.
    descendants: bool,
    test: NodeTest,
    predicates: Vec<Predicate>,
}

#[derive(Clone, Debug)]
struct Path {
    absolute: bool,
    steps: Vec<Step>,
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Result<Token, XPathError> {
        let t = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or(XPathError::Invalid)?;
        self.at += 1;
        Ok(t)
    }

    fn expect(&mut self, want: &Token) -> Result<(), XPathError> {
        let t = self.next()?;
        if &t == want { Ok(()) } else { Err(refused(&t)) }
    }

    fn union(&mut self) -> Result<Vec<Path>, XPathError> {
        let mut paths = vec![self.path()?];
        while let Some(t) = self.peek() {
            if *t != Token::Pipe {
                return Err(refused(t));
            }
            self.at += 1;
            paths.push(self.path()?);
        }
        Ok(paths)
    }

    fn path(&mut self) -> Result<Path, XPathError> {
        let mut steps = Vec::new();
        let absolute = match self.peek() {
            Some(Token::Slash) => {
                self.at += 1;
                // `/` alone is the document node
                if matches!(self.peek(), None | Some(Token::Pipe)) {
                    return Ok(Path {
                        absolute: true,
                        steps,
                    });
                }
                steps.push(self.step(false)?);
                true
            }
            Some(Token::DoubleSlash) => {
                self.at += 1;
                steps.push(self.step(true)?);
                true
            }
            _ => {
                steps.push(self.step(false)?);
                false
            }
        };
        loop {
            match self.peek() {
                Some(Token::Slash) => {
                    self.at += 1;
                    steps.push(self.step(false)?);
                }
                Some(Token::DoubleSlash) => {
                    self.at += 1;
                    steps.push(self.step(true)?);
                }
                _ => return Ok(Path { absolute, steps }),
            }
        }
    }

    fn step(&mut self, descendants: bool) -> Result<Step, XPathError> {
        let test = match self.next()? {
            Token::Star => NodeTest::Any,
            Token::Name(name) if self.peek() == Some(&Token::LParen) => {
                self.at += 1;
                self.expect(&Token::RParen)?;
                match name.as_str() {
                    "text" => NodeTest::Text,
                    "node" => NodeTest::Node,
                    _ => return Err(XPathError::Refused(format!("{name}()"))),
                }
            }
            Token::Name(name) => NodeTest::Name(name),
            t => return Err(refused(&t)),
        };
        let mut predicates = Vec::new();
        while self.peek() == Some(&Token::LBracket) {
            self.at += 1;
            predicates.push(self.predicate()?);
            self.expect(&Token::RBracket)?;
        }
        Ok(Step {
            descendants,
            test,
            predicates,
        })
    }

    fn predicate(&mut self) -> Result<Predicate, XPathError> {
        match self.next()? {
            Token::Name(name) if name == "text" => {
                self.expect(&Token::LParen)?;
                self.expect(&Token::RParen)?;
                self.expect(&Token::Equals)?;
                match self.next()? {
                    Token::Literal(value) => Ok(Predicate::TextEquals(value)),
                    t => Err(refused(&t)),
                }
            }
            Token::At => match self.next()? {
                Token::Name(name) => Ok(Predicate::HasAttribute(name)),
                t => Err(refused(&t)),
            },
            Token::Number(n) => match n.parse::<usize>() {
                Ok(position) => Ok(Predicate::Position(position)),
                Err(_) => Err(XPathError::Refused(n)),
            },
            t => Err(refused(&t)),
        }
    }
}

fn refused(t: &Token) -> XPathError {
    XPathError::Refused(format!("{t:?}"))
}

/// `$xpath->evaluate($expression)` on a `DOMXPath` for `doc`, without a
/// context node: the nodes found, in document order.
pub fn evaluate(doc: &Document, expression: &[u8]) -> Result<Vec<NodeId>, XPathError> {
    let expression = match expression.iter().position(|&b| b == 0) {
        Some(nul) => &expression[..nul],
        None => expression,
    };
    let expression = std::str::from_utf8(expression).map_err(|_| XPathError::Invalid)?;
    let tokens = tokenize(expression)?;
    let mut parser = Parser { tokens, at: 0 };
    let paths = parser.union()?;

    let mut found = HashSet::new();
    for path in &paths {
        let start = if path.absolute {
            doc.root()
        } else {
            match doc.document_element() {
                Some(e) => e,
                None => continue,
            }
        };
        let mut set = vec![start];
        for step in &path.steps {
            set = apply(doc, &set, step);
        }
        found.extend(set);
    }
    Ok(document_order(doc)
        .into_iter()
        .filter(|n| found.contains(n))
        .collect())
}

/// Every node of `doc`, in document order.
fn document_order(doc: &Document) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![doc.root()];
    while let Some(n) = stack.pop() {
        out.push(n);
        stack.extend(doc.children(n).iter().rev().copied());
    }
    out
}

fn apply(doc: &Document, set: &[NodeId], step: &Step) -> Vec<NodeId> {
    let mut parents = Vec::new();
    if step.descendants {
        // descendant-or-self::node()/child::…
        let mut seen = HashSet::new();
        for &n in set {
            let mut stack = vec![n];
            while let Some(d) = stack.pop() {
                if seen.insert(d) {
                    parents.push(d);
                }
                stack.extend(doc.children(d).iter().rev().copied());
            }
        }
    } else {
        parents.extend_from_slice(set);
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for parent in parents {
        let mut candidates: Vec<NodeId> = doc
            .children(parent)
            .iter()
            .copied()
            .filter(|&c| matches_test(doc, c, &step.test))
            .collect();
        for predicate in &step.predicates {
            candidates = candidates
                .iter()
                .enumerate()
                .filter(|&(i, &c)| holds(doc, c, i + 1, predicate))
                .map(|(_, &c)| c)
                .collect();
        }
        for c in candidates {
            if seen.insert(c) {
                out.push(c);
            }
        }
    }
    out
}

fn matches_test(doc: &Document, n: NodeId, test: &NodeTest) -> bool {
    let kind = doc.node_type(n);
    match test {
        NodeTest::Node => true,
        NodeTest::Text => matches!(kind, NodeType::Text | NodeType::CData),
        NodeTest::Any => kind == NodeType::Element,
        NodeTest::Name(name) => {
            kind == NodeType::Element
                && doc.local_name(n) == Some(name.as_str())
                && doc.namespace_uri(n).is_none()
        }
    }
}

fn holds(doc: &Document, n: NodeId, position: usize, predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Position(p) => position == *p,
        Predicate::TextEquals(value) => doc.children(n).iter().any(|&c| {
            matches!(doc.node_type(c), NodeType::Text | NodeType::CData)
                && doc.node_value(c) == Some(value.as_str())
        }),
        Predicate::HasAttribute(name) => doc
            .attributes(n)
            .iter()
            .any(|a| a.local_name == *name && a.namespace_uri.is_none()),
    }
}
