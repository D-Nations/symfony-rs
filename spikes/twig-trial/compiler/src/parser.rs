//! Port of Twig\Parser.php and the core expression parsers (trial subset):
//! the precedence-climbing loop of `Parser::parseExpression()`, literals,
//! sequences and mappings, unary `not`/`-`/`+`, the binary operators of
//! `CoreExtension` with their precedence and associativity, the ternary,
//! `.` attribute access, function calls and filters. Tags: `if`/`elseif`/
//! `else` and `for`/`else`. Anything else is refused with a location.
//!
//! Upstream: twig/twig src/Parser.php src/ExpressionParser/Prefix/LiteralExpressionParser.php src/ExpressionParser/Prefix/UnaryOperatorExpressionParser.php src/ExpressionParser/Infix/BinaryOperatorExpressionParser.php src/ExpressionParser/Infix/ConditionalTernaryExpressionParser.php src/ExpressionParser/Infix/DotExpressionParser.php src/ExpressionParser/Infix/FunctionExpressionParser.php src/ExpressionParser/Infix/FilterExpressionParser.php src/ExpressionParser/Infix/ArgumentsTrait.php src/TokenParser/IfTokenParser.php src/TokenParser/ForTokenParser.php
//! Upstream: twig/twig src/Extension/CoreExtension.php

use crate::{
    Error, Number,
    lexer::{Token, TokenKind},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    Any,
    Method,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    Not,
    Neg,
    Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Add,
    Sub,
    Concat,
    Mul,
    NullCoalesce,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Constant(Constant),
    Name(String),
    GetAttr {
        object: Box<Expr>,
        attribute: String,
        arguments: Vec<Expr>,
        call: CallType,
    },
    /// A sequence or mapping; keys are `None` for sequence elements.
    Array(Vec<(Option<Expr>, Expr)>),
    Unary(Unary, Box<Expr>),
    Binary(Binary, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Function {
        name: String,
        arguments: Vec<Expr>,
        line: usize,
    },
    Filter {
        node: Box<Expr>,
        name: String,
        arguments: Vec<Expr>,
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text(String),
    Print(Expr, usize),
    If {
        /// (condition, its line, body)
        tests: Vec<(Expr, usize, Vec<Node>)>,
        otherwise: Option<Vec<Node>>,
    },
    For {
        key: Option<String>,
        value: String,
        sequence: Expr,
        line: usize,
        body: Vec<Node>,
        otherwise: Option<Vec<Node>>,
    },
}

enum Infix {
    Binary(Binary, u32, bool /* right associative */),
    Ternary,
    Dot,
    Filter,
    Call,
}

/// Precedences from `CoreExtension::getExpressionParsers()`.
fn infix(op: &str) -> Option<(Infix, u32)> {
    let binary = |b, p| Some((Infix::Binary(b, p, false), p));
    match op {
        "or" => binary(Binary::Or, 10),
        "and" => binary(Binary::And, 15),
        "==" => binary(Binary::Equal, 20),
        "!=" => binary(Binary::NotEqual, 20),
        "<" => binary(Binary::Less, 20),
        ">" => binary(Binary::Greater, 20),
        "<=" => binary(Binary::LessEqual, 20),
        ">=" => binary(Binary::GreaterEqual, 20),
        "+" => binary(Binary::Add, 30),
        "-" => binary(Binary::Sub, 30),
        "~" => binary(Binary::Concat, 40),
        "*" => binary(Binary::Mul, 60),
        "??" => Some((Infix::Binary(Binary::NullCoalesce, 300, true), 300)),
        "?" => Some((Infix::Ternary, 0)),
        "." => Some((Infix::Dot, 512)),
        "|" => Some((Infix::Filter, 512)),
        "(" => Some((Infix::Call, 512)),
        _ => None,
    }
}

/// Operators the lexer knows but the trial does not compile.
fn refused_infix(op: &str) -> bool {
    matches!(
        op,
        "?:" | "? :"
            | "xor"
            | "b-or"
            | "b-xor"
            | "b-and"
            | "<=>"
            | "not in"
            | "in"
            | "matches"
            | "starts with"
            | "ends with"
            | "has some"
            | "has every"
            | "==="
            | "!=="
            | ".."
            | "/"
            | "//"
            | "%"
            | "**"
            | "="
            | "is"
            | "is not"
            | "?."
            | "["
            | "=>"
    )
}

pub struct Parser<'a> {
    name: &'a str,
    tokens: Vec<Token>,
    pos: usize,
}

pub fn parse(name: &str, tokens: Vec<Token>) -> Result<Vec<Node>, Error> {
    let mut parser = Parser {
        name,
        tokens,
        pos: 0,
    };
    let (body, end) = parser.subparse(&[])?;
    debug_assert!(end.is_none());
    Ok(body)
}

impl Parser<'_> {
    fn err(&self, message: impl Into<String>, line: usize) -> Error {
        Error::syntax(message, self.name, line)
    }

    fn current(&self) -> &Token {
        // The lexer always ends the stream with Eof.
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn next(&mut self) -> Token {
        let token = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    fn unexpected(&self) -> Error {
        let token = self.current();
        let (kind, value) = token.describe();
        self.err(
            format!("Unexpected token \"{kind}\" of value \"{value}\"."),
            token.line,
        )
    }

    fn test_punct(&self, c: char) -> bool {
        self.current().kind == TokenKind::Punctuation(c)
    }

    fn test_op(&self, op: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Operator(o) if o == op)
    }

    fn expect_punct(&mut self, c: char, message: &str) -> Result<(), Error> {
        if self.test_punct(c) {
            self.next();
            return Ok(());
        }
        let token = self.current();
        let (kind, value) = token.describe();
        Err(self.err(
            format!("{message}. Unexpected token \"{kind}\" of value \"{value}\" (\"punctuation\" expected with value \"{c}\")."),
            token.line,
        ))
    }

    #[expect(clippy::needless_pass_by_value, reason = "Twig's expect($type)")]
    fn expect(&mut self, kind: TokenKind) -> Result<Token, Error> {
        if self.current().kind == kind {
            return Ok(self.next());
        }
        Err(self.unexpected())
    }

    fn expect_name(&mut self) -> Result<String, Error> {
        match self.current().kind.clone() {
            TokenKind::Name(n) => {
                self.next();
                Ok(n)
            }
            _ => Err(self.unexpected()),
        }
    }

    /// `Parser::subparse()`: reads nodes until one of `ends` starts a block
    /// tag, and returns that tag's name (the tag token is consumed).
    fn subparse(&mut self, ends: &[&str]) -> Result<(Vec<Node>, Option<String>), Error> {
        let mut body = Vec::new();
        loop {
            let token = self.next();
            match token.kind {
                TokenKind::Eof => {
                    if !ends.is_empty() {
                        return Err(self.err(
                            format!(
                                "Unexpected end of template. Twig was looking for the following tags \"{}\".",
                                ends.join("\", \"")
                            ),
                            token.line,
                        ));
                    }
                    return Ok((body, None));
                }
                TokenKind::Text(text) => body.push(Node::Text(text)),
                TokenKind::VarStart => {
                    let expr = self.parse_expression(0)?;
                    self.expect(TokenKind::VarEnd)?;
                    body.push(Node::Print(expr, token.line));
                }
                TokenKind::BlockStart => {
                    let tag_token = self.current().clone();
                    let TokenKind::Name(tag) = tag_token.kind else {
                        return Err(self.err("A block must start with a tag name.", tag_token.line));
                    };
                    if ends.contains(&tag.as_str()) {
                        self.next();
                        return Ok((body, Some(tag)));
                    }
                    self.next();
                    body.push(match tag.as_str() {
                        "if" => self.parse_if()?,
                        "for" => self.parse_for(tag_token.line)?,
                        _ => {
                            return Err(self.err(
                                format!(
                                    "The Twig trial compiler does not support the \"{tag}\" tag."
                                ),
                                tag_token.line,
                            ));
                        }
                    });
                }
                _ => {
                    return Err(
                        self.err("Lexer or parser ended up in unsupported state.", token.line)
                    );
                }
            }
        }
    }

    /// `IfTokenParser::parse()`.
    fn parse_if(&mut self) -> Result<Node, Error> {
        let mut tests = Vec::new();
        let mut otherwise = None;
        let mut line = self.current().line;
        let mut condition = self.parse_expression(0)?;
        self.expect(TokenKind::BlockEnd)?;
        loop {
            let (body, end) = self.subparse(&["elseif", "else", "endif"])?;
            tests.push((condition, line, body));
            match end.as_deref() {
                Some("else") => {
                    self.expect(TokenKind::BlockEnd)?;
                    let (body, _) = self.subparse(&["endif"])?;
                    otherwise = Some(body);
                    break;
                }
                Some("elseif") => {
                    line = self.current().line;
                    condition = self.parse_expression(0)?;
                    self.expect(TokenKind::BlockEnd)?;
                }
                _ => break,
            }
        }
        self.expect(TokenKind::BlockEnd)?;
        Ok(Node::If { tests, otherwise })
    }

    /// `ForTokenParser::parse()`.
    fn parse_for(&mut self, line: usize) -> Result<Node, Error> {
        let first = self.expect_name()?;
        let (key, value) = if self.test_punct(',') {
            self.next();
            (Some(first), self.expect_name()?)
        } else {
            (None, first)
        };
        if !self.test_op("in") {
            let token = self.current();
            let (kind, value) = token.describe();
            return Err(self.err(
                format!("Unexpected token \"{kind}\" of value \"{value}\" (\"operator\" expected with value \"in\")."),
                token.line,
            ));
        }
        self.next();
        let sequence = self.parse_expression(0)?;
        self.expect(TokenKind::BlockEnd)?;
        let (body, end) = self.subparse(&["else", "endfor"])?;
        let otherwise = if end.as_deref() == Some("else") {
            self.expect(TokenKind::BlockEnd)?;
            Some(self.subparse(&["endfor"])?.0)
        } else {
            None
        };
        self.expect(TokenKind::BlockEnd)?;
        Ok(Node::For {
            key,
            value,
            sequence,
            line,
            body,
            otherwise,
        })
    }

    /// `Parser::parseExpression()`.
    pub fn parse_expression(&mut self, precedence: u32) -> Result<Expr, Error> {
        let token = self.current().clone();
        let mut expr = match &token.kind {
            TokenKind::Operator(op) if op == "not" => {
                self.next();
                Expr::Unary(Unary::Not, Box::new(self.parse_expression(50)?))
            }
            TokenKind::Operator(op) if op == "-" || op == "+" => {
                self.next();
                let unary = if op == "-" { Unary::Neg } else { Unary::Pos };
                Expr::Unary(unary, Box::new(self.parse_expression(500)?))
            }
            TokenKind::Operator(op) if op == "(" => {
                self.next();
                let inner = self.parse_expression(0)?;
                self.expect_punct(')', "An opened parenthesis is not properly closed")?;
                inner
            }
            TokenKind::Operator(op) if op == "..." => {
                return Err(self.err(
                    "The Twig trial compiler does not support the spread operator.",
                    token.line,
                ));
            }
            _ => self.parse_literal()?,
        };
        loop {
            let token = self.current().clone();
            let TokenKind::Operator(op) = &token.kind else {
                break;
            };
            let Some((kind, op_precedence)) = infix(op) else {
                if refused_infix(op) && precedence == 0 {
                    return Err(self.err(
                        format!("The Twig trial compiler does not support the \"{op}\" operator."),
                        token.line,
                    ));
                }
                break;
            };
            if op_precedence < precedence {
                break;
            }
            self.next();
            expr = match kind {
                Infix::Binary(binary, p, right) => {
                    let right_expr = self.parse_expression(if right { p } else { p + 1 })?;
                    Expr::Binary(binary, Box::new(expr), Box::new(right_expr))
                }
                Infix::Ternary => {
                    let then = self.parse_expression(0)?;
                    let otherwise = if self.test_punct(':') {
                        self.next();
                        self.parse_expression(0)?
                    } else {
                        Expr::Constant(Constant::String(String::new()))
                    };
                    Expr::Ternary(Box::new(expr), Box::new(then), Box::new(otherwise))
                }
                Infix::Dot => self.parse_dot(expr)?,
                Infix::Filter => self.parse_filter(expr)?,
                Infix::Call => {
                    let Expr::Name(name) = expr else {
                        return Err(self.err("Function name must be an identifier.", token.line));
                    };
                    let arguments = self.parse_arguments_after_paren()?;
                    Expr::Function {
                        name,
                        arguments,
                        line: token.line,
                    }
                }
            };
        }
        Ok(expr)
    }

    fn parse_literal(&mut self) -> Result<Expr, Error> {
        let token = self.current().clone();
        Ok(match token.kind {
            TokenKind::Name(name) => {
                self.next();
                match name.as_str() {
                    "true" | "TRUE" => Expr::Constant(Constant::Bool(true)),
                    "false" | "FALSE" => Expr::Constant(Constant::Bool(false)),
                    "none" | "NONE" | "null" | "NULL" => Expr::Constant(Constant::Null),
                    _ => Expr::Name(name),
                }
            }
            TokenKind::Number(n) => {
                self.next();
                Expr::Constant(Constant::Number(n))
            }
            TokenKind::String(s) => {
                self.next();
                if matches!(self.current().kind, TokenKind::String(_)) {
                    return Err(self.unexpected());
                }
                Expr::Constant(Constant::String(s))
            }
            TokenKind::Punctuation('{') => self.parse_mapping()?,
            TokenKind::Operator(op) if op == "[" => self.parse_sequence()?,
            TokenKind::Operator(op) if op.bytes().all(|b| b.is_ascii_alphabetic()) => {
                // in this context, string operators are variable names
                self.next();
                Expr::Name(op)
            }
            _ => return Err(self.unexpected()),
        })
    }

    fn parse_sequence(&mut self) -> Result<Expr, Error> {
        self.next();
        let mut elements = Vec::new();
        while !self.test_punct(']') {
            if !elements.is_empty() {
                self.expect_punct(',', "A sequence element must be followed by a comma")?;
                if self.test_punct(']') {
                    break;
                }
            }
            elements.push((None, self.parse_expression(0)?));
        }
        self.expect_punct(']', "An opened sequence is not properly closed")?;
        Ok(Expr::Array(elements))
    }

    fn parse_mapping(&mut self) -> Result<Expr, Error> {
        self.next();
        let mut elements = Vec::new();
        while !self.test_punct('}') {
            if !elements.is_empty() {
                self.expect_punct(',', "A mapping value must be followed by a comma")?;
                if self.test_punct('}') {
                    break;
                }
            }
            let token = self.current().clone();
            let key = match token.kind {
                TokenKind::Name(name) => {
                    self.next();
                    if self.test_punct(',') || self.test_punct('}') {
                        // {a} is a shortcut for {a: a}
                        elements.push((
                            Some(Expr::Constant(Constant::String(name.clone()))),
                            Expr::Name(name),
                        ));
                        continue;
                    }
                    Expr::Constant(Constant::String(name))
                }
                TokenKind::String(s) => {
                    self.next();
                    Expr::Constant(Constant::String(s))
                }
                TokenKind::Number(n) => {
                    self.next();
                    Expr::Constant(Constant::Number(n))
                }
                TokenKind::Operator(op) if op == "(" => self.parse_expression(0)?,
                _ => {
                    let (kind, value) = token.describe();
                    return Err(self.err(
                        format!("A mapping key must be a quoted string, a number, a name, or an expression enclosed in parentheses (unexpected token \"{kind}\" of value \"{value}\"."),
                        token.line,
                    ));
                }
            };
            self.expect_punct(':', "A mapping key must be followed by a colon (:)")?;
            elements.push((Some(key), self.parse_expression(0)?));
        }
        self.expect_punct('}', "An opened mapping is not properly closed")?;
        Ok(Expr::Array(elements))
    }

    /// `DotExpressionParser::parse()`.
    fn parse_dot(&mut self, object: Expr) -> Result<Expr, Error> {
        let token = self.next();
        let attribute = match token.kind {
            TokenKind::Name(n) => n,
            TokenKind::Number(n) => n.to_string(),
            TokenKind::Operator(op) if op.bytes().all(|b| b.is_ascii_alphabetic()) => op,
            _ => {
                let (kind, value) = token.describe();
                return Err(self.err(
                    format!("Expected name or number, got value \"{value}\" of type \"{kind}\"."),
                    token.line,
                ));
            }
        };
        let (arguments, call) = if self.test_op("(") {
            self.next();
            (self.parse_arguments_after_paren()?, CallType::Method)
        } else {
            (Vec::new(), CallType::Any)
        };
        Ok(Expr::GetAttr {
            object: Box::new(object),
            attribute,
            arguments,
            call,
        })
    }

    /// `FilterExpressionParser::parse()`.
    fn parse_filter(&mut self, node: Expr) -> Result<Expr, Error> {
        let line = self.current().line;
        let name = self.expect_name()?;
        let arguments = if self.test_op("(") {
            self.next();
            self.parse_arguments_after_paren()?
        } else {
            Vec::new()
        };
        Ok(Expr::Filter {
            node: Box::new(node),
            name,
            arguments,
            line,
        })
    }

    /// `ArgumentsTrait::parseNamedArguments()`, positional arguments only.
    fn parse_arguments_after_paren(&mut self) -> Result<Vec<Expr>, Error> {
        let mut args = Vec::new();
        while !self.test_punct(')') {
            if !args.is_empty() {
                self.expect_punct(',', "Arguments must be separated by a comma")?;
                if self.test_punct(')') {
                    break;
                }
            }
            args.push(self.parse_expression(0)?);
            if self.test_op("=") || self.test_punct(':') {
                return Err(self.err(
                    "The Twig trial compiler does not support named arguments.",
                    self.current().line,
                ));
            }
        }
        self.expect_punct(')', "A list of arguments must be closed by a parenthesis")?;
        Ok(args)
    }
}
