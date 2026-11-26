//! Port of Symfony\Component\ExpressionLanguage\Parser.php: a precedence
//! climbing parser from a token stream to a node tree.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Parser.php

use php_value::{Key, PhpArray, PhpString, Value, compare::loose_equals};

use crate::{
    expression_function::Functions,
    lexer::match_name,
    node::{
        ArrayNode, BinaryNode, ConditionalNode, ConstantNode, FunctionNode, GetAttrNode,
        GetAttrType, NameNode, Node, NullCoalesceNode, NullCoalescedNameNode, UnaryNode,
    },
    syntax_error::Error,
    token::TokenType,
    token_stream::TokenStream,
};

/// `Parser::IGNORE_UNKNOWN_VARIABLES`.
pub const IGNORE_UNKNOWN_VARIABLES: u32 = 1;
/// `Parser::IGNORE_UNKNOWN_FUNCTIONS`.
pub const IGNORE_UNKNOWN_FUNCTIONS: u32 = 2;

const MAX_NESTING_LEVEL: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Associativity {
    Left,
    Right,
}

fn unary_precedence(operator: &[u8]) -> Option<i32> {
    Some(match operator {
        b"not" | b"!" => 50,
        b"-" | b"+" | b"~" => 500,
        _ => return None,
    })
}

fn binary_operator(operator: &[u8]) -> Option<(i32, Associativity)> {
    use Associativity::{Left, Right};
    Some(match operator {
        b"or" | b"||" => (10, Left),
        b"xor" => (12, Left),
        b"and" | b"&&" => (15, Left),
        b"|" => (16, Left),
        b"^" => (17, Left),
        b"&" => (18, Left),
        b"==" | b"===" | b"!=" | b"!==" | b"<" | b">" | b">=" | b"<=" | b"not in" | b"in"
        | b"contains" | b"starts with" | b"ends with" | b"matches" => (20, Left),
        b".." | b"<<" | b">>" => (25, Left),
        b"+" | b"-" => (30, Left),
        b"~" => (40, Left),
        b"*" | b"/" | b"%" => (60, Left),
        b"**" => (200, Right),
        _ => return None,
    })
}

/// Parses token streams.
#[derive(Debug, Clone, Default)]
pub struct Parser {
    functions: Functions,
}

impl Parser {
    /// `new Parser($functions)`.
    pub fn new(functions: Functions) -> Self {
        Self { functions }
    }

    /// `parse($stream, $names, $flags)`. The names are the values of
    /// `names`; a string key is the name the compiled code uses
    /// (`['this' => 'container']`).
    pub fn parse(&self, stream: TokenStream, names: &PhpArray, flags: u32) -> Result<Node, Error> {
        self.do_parse(stream, names, flags)
    }

    /// `lint($stream, $names, $flags)`.
    pub fn lint(&self, stream: TokenStream, names: &PhpArray, flags: u32) -> Result<(), Error> {
        self.do_parse(stream, names, flags).map(|_| ())
    }

    fn do_parse(&self, stream: TokenStream, names: &PhpArray, flags: u32) -> Result<Node, Error> {
        let mut state = State {
            functions: &self.functions,
            stream,
            names,
            flags,
            nesting_level: 0,
        };
        let node = state.parse_expression(0)?;
        if !state.stream.is_eof() {
            let current = state.stream.current();
            return Err(unexpected_token(
                current.kind,
                &current.value_string(),
                current.cursor(),
                state.stream.expression(),
            ));
        }
        // a node tree is at most one level deeper than its number of tokens
        if MAX_NESTING_LEVEL <= state.stream.expression().len() {
            state.check_depth(&node)?;
        }
        Ok(node)
    }
}

fn unexpected_token(
    kind: TokenType,
    value: &PhpString,
    cursor: usize,
    expression: &PhpString,
) -> Error {
    let mut message = format!("Unexpected token \"{}\" of value \"", kind.as_str()).into_bytes();
    message.extend_from_slice(value.as_bytes());
    message.extend_from_slice(b"\".");
    Error::syntax(message, cursor, expression.as_bytes())
}

struct State<'a> {
    functions: &'a Functions,
    stream: TokenStream,
    names: &'a PhpArray,
    flags: u32,
    nesting_level: usize,
}

impl State<'_> {
    fn expression(&self) -> &[u8] {
        self.stream.expression().as_bytes()
    }

    fn nested_too_deeply(&self) -> Error {
        Error::syntax(
            format!(
                "Expression is nested too deeply, the maximum nesting level is {MAX_NESTING_LEVEL}"
            ),
            self.stream.current().cursor(),
            self.expression(),
        )
    }

    /// `enterNestingLevel()`.
    fn enter_nesting_level(&mut self) -> Result<(), Error> {
        self.nesting_level += 1;
        if MAX_NESTING_LEVEL < self.nesting_level {
            return Err(self.nested_too_deeply());
        }
        Ok(())
    }

    /// `checkDepth($node)`.
    fn check_depth(&self, node: &Node) -> Result<(), Error> {
        let mut nodes = vec![(node, 1usize)];
        while let Some((node, depth)) = nodes.pop() {
            if MAX_NESTING_LEVEL < depth {
                return Err(self.nested_too_deeply());
            }
            for child in node.children() {
                nodes.push((child, depth + 1));
            }
        }
        Ok(())
    }

    fn parse_expression(&mut self, precedence: i32) -> Result<Node, Error> {
        let nesting_level = self.nesting_level;
        let result = self.parse_expression_inner(precedence, nesting_level);
        self.nesting_level = nesting_level;
        result
    }

    fn parse_expression_inner(
        &mut self,
        precedence: i32,
        nesting_level: usize,
    ) -> Result<Node, Error> {
        self.enter_nesting_level()?;
        let mut expr = self.get_primary()?;
        loop {
            let token = self.stream.current();
            if token.kind != TokenType::Operator {
                break;
            }
            let Some((op_precedence, associativity)) = binary_operator(token.value_bytes()) else {
                break;
            };
            if op_precedence < precedence {
                break;
            }
            let operator = String::from_utf8_lossy(token.value_bytes()).into_owned();
            self.enter_nesting_level()?;
            self.stream.next()?;
            let expr1 = self.parse_expression(if associativity == Associativity::Left {
                op_precedence + 1
            } else {
                op_precedence
            })?;
            expr = Node::Binary(BinaryNode::new(operator, expr, expr1));
        }

        if precedence == 0 {
            self.nesting_level = nesting_level + 1;
            return self.parse_conditional_expression(expr);
        }
        Ok(expr)
    }

    fn get_primary(&mut self) -> Result<Node, Error> {
        let token = self.stream.current();
        if token.kind == TokenType::Operator
            && let Some(precedence) = unary_precedence(token.value_bytes())
        {
            let operator = String::from_utf8_lossy(token.value_bytes()).into_owned();
            self.stream.next()?;
            let expr = self.parse_expression(precedence)?;
            return self.parse_postfix_expression(Node::Unary(UnaryNode::new(operator, expr)));
        }

        if token.test(TokenType::Punctuation, Some("(")) {
            self.stream.next()?;
            let expr = self.parse_expression(0)?;
            self.stream.expect(
                TokenType::Punctuation,
                Some(")"),
                Some("An opened parenthesis is not properly closed"),
            )?;
            return self.parse_postfix_expression(expr);
        }

        self.parse_primary_expression()
    }

    fn parse_conditional_expression(&mut self, mut expr: Node) -> Result<Node, Error> {
        while self
            .stream
            .current()
            .test(TokenType::Punctuation, Some("??"))
        {
            self.enter_nesting_level()?;
            self.stream.next()?;
            let expr2 = self.parse_expression(0)?;
            expr = Node::NullCoalesce(NullCoalesceNode::new(expr, expr2));
        }

        while self
            .stream
            .current()
            .test(TokenType::Punctuation, Some("?"))
        {
            self.enter_nesting_level()?;
            self.stream.next()?;
            let (expr2, expr3);
            if self
                .stream
                .current()
                .test(TokenType::Punctuation, Some(":"))
            {
                self.stream.next()?;
                expr3 = self.parse_expression(0)?;
                expr2 = None;
            } else {
                let e2 = self.parse_expression(0)?;
                if self
                    .stream
                    .current()
                    .test(TokenType::Punctuation, Some(":"))
                {
                    self.stream.next()?;
                    expr3 = self.parse_expression(0)?;
                } else {
                    expr3 = Node::Constant(ConstantNode::new(Value::Null));
                }
                expr2 = Some(e2);
            }
            let expr2 = expr2.unwrap_or_else(|| expr.clone());
            expr = Node::Conditional(ConditionalNode::new(expr, expr2, expr3));
        }

        Ok(expr)
    }

    fn parse_primary_expression(&mut self) -> Result<Node, Error> {
        let token = self.stream.current().clone();
        let node = match token.kind {
            TokenType::Name => {
                let (node, postfix) = self.parse_name(&token)?;
                if !postfix {
                    return Ok(node);
                }
                node
            }
            TokenType::Number | TokenType::String => {
                self.stream.next()?;
                return Ok(Node::Constant(ConstantNode::new(token.value)));
            }
            _ => {
                if token.test(TokenType::Punctuation, Some("[")) {
                    Node::Array(self.parse_array_expression()?)
                } else if token.test(TokenType::Punctuation, Some("{")) {
                    Node::Array(self.parse_hash_expression()?)
                } else {
                    return Err(unexpected_token(
                        token.kind,
                        &token.value_string(),
                        token.cursor(),
                        self.stream.expression(),
                    ));
                }
            }
        };
        self.parse_postfix_expression(node)
    }

    /// The name branch of `parsePrimaryExpression()`: a constant, a
    /// function call or a variable. The flag says if postfix operators
    /// follow. It is a separate function so that the frame of the
    /// recursive path (arrays, hashes) stays small.
    #[inline(never)]
    fn parse_name(&mut self, token: &crate::token::Token) -> Result<(Node, bool), Error> {
        self.stream.next()?;
        let name = token.value_bytes();
        match name {
            b"true" | b"TRUE" => return Ok((Node::Constant(ConstantNode::new(true)), false)),
            b"false" | b"FALSE" => return Ok((Node::Constant(ConstantNode::new(false)), false)),
            b"null" | b"NULL" => {
                return Ok((Node::Constant(ConstantNode::new(Value::Null)), false));
            }
            _ => {}
        }
        let node = if self.stream.current().value.identical(&Value::from("(")) {
            if self.flags & IGNORE_UNKNOWN_FUNCTIONS == 0 && !self.functions.contains(name) {
                let mut message = b"The function \"".to_vec();
                message.extend_from_slice(name);
                message.extend_from_slice(b"\" does not exist.");
                return Err(Error::syntax_with_proposals(
                    message,
                    token.cursor(),
                    self.expression(),
                    Some((name, &self.functions.names())),
                ));
            }
            let arguments = self.parse_arguments()?;
            Node::Function(FunctionNode::new(PhpString::from(name), arguments))
        } else {
            let compiled_name = if self.flags & IGNORE_UNKNOWN_VARIABLES == 0 {
                let value = &token.value;
                if !self.names.values().any(|n| n.identical(value)) {
                    if self
                        .stream
                        .current()
                        .test(TokenType::Punctuation, Some("??"))
                    {
                        return Ok((
                            Node::NullCoalescedName(NullCoalescedNameNode::new(PhpString::from(
                                name,
                            ))),
                            false,
                        ));
                    }
                    let mut message = b"Variable \"".to_vec();
                    message.extend_from_slice(name);
                    message.extend_from_slice(b"\" is not valid.");
                    let proposals: Vec<PhpString> = self
                        .names
                        .values()
                        .filter_map(Value::to_php_string)
                        .collect();
                    return Err(Error::syntax_with_proposals(
                        message,
                        token.cursor(),
                        self.expression(),
                        Some((name, &proposals)),
                    ));
                }
                // is the name used in the compiled code different
                // from the name used in the expression?
                match self
                    .names
                    .iter()
                    .find(|(_, n)| loose_equals(n, value))
                    .map(|(k, _)| k)
                {
                    Some(Key::Str(key)) => key.clone(),
                    _ => PhpString::from(name),
                }
            } else {
                PhpString::from(name)
            };
            Node::Name(NameNode::new(compiled_name))
        };
        Ok((node, true))
    }

    fn parse_array_expression(&mut self) -> Result<ArrayNode, Error> {
        self.stream.expect(
            TokenType::Punctuation,
            Some("["),
            Some("An array element was expected"),
        )?;
        let mut node = ArrayNode::new();
        let mut first = true;
        while !self
            .stream
            .current()
            .test(TokenType::Punctuation, Some("]"))
        {
            if !first {
                self.stream.expect(
                    TokenType::Punctuation,
                    Some(","),
                    Some("An array element must be followed by a comma"),
                )?;
                // trailing ,?
                if self
                    .stream
                    .current()
                    .test(TokenType::Punctuation, Some("]"))
                {
                    break;
                }
            }
            first = false;
            let element = self.parse_expression(0)?;
            node.add_element(element, None);
        }
        self.stream.expect(
            TokenType::Punctuation,
            Some("]"),
            Some("An opened array is not properly closed"),
        )?;
        Ok(node)
    }

    fn parse_hash_expression(&mut self) -> Result<ArrayNode, Error> {
        self.stream.expect(
            TokenType::Punctuation,
            Some("{"),
            Some("A hash element was expected"),
        )?;
        let mut node = ArrayNode::new();
        let mut first = true;
        while !self
            .stream
            .current()
            .test(TokenType::Punctuation, Some("}"))
        {
            if !first {
                self.stream.expect(
                    TokenType::Punctuation,
                    Some(","),
                    Some("A hash value must be followed by a comma"),
                )?;
                // trailing ,?
                if self
                    .stream
                    .current()
                    .test(TokenType::Punctuation, Some("}"))
                {
                    break;
                }
            }
            first = false;

            // a hash key can be:
            //
            //  * a number -- 12
            //  * a string -- 'a'
            //  * a name, which is equivalent to a string -- a
            //  * an expression, which must be enclosed in parentheses -- (1 + 2)
            let current = self.stream.current();
            let key = if current.test(TokenType::String, None)
                || current.test(TokenType::Name, None)
                || current.test(TokenType::Number, None)
            {
                let key = Node::Constant(ConstantNode::new(current.value.clone()));
                self.stream.next()?;
                key
            } else if current.test(TokenType::Punctuation, Some("(")) {
                self.parse_expression(0)?
            } else {
                let mut message = format!(
                    "A hash key must be a quoted string, a number, a name, or an expression enclosed in parentheses (unexpected token \"{}\" of value \"",
                    current.kind.as_str()
                )
                .into_bytes();
                message.extend_from_slice(current.value_string().as_bytes());
                message.extend_from_slice(b"\".");
                return Err(Error::syntax(message, current.cursor(), self.expression()));
            };

            self.stream.expect(
                TokenType::Punctuation,
                Some(":"),
                Some("A hash key must be followed by a colon (:)"),
            )?;
            let value = self.parse_expression(0)?;
            node.add_element(value, Some(key));
        }
        self.stream.expect(
            TokenType::Punctuation,
            Some("}"),
            Some("An opened hash is not properly closed"),
        )?;
        Ok(node)
    }

    fn parse_postfix_expression(&mut self, mut node: Node) -> Result<Node, Error> {
        loop {
            let token = self.stream.current().clone();
            if token.kind != TokenType::Punctuation {
                break;
            }
            let value = token.value_bytes();
            if value == b"." || value == b"?." {
                self.enter_nesting_level()?;
                let is_null_safe = value == b"?.";
                self.stream.next()?;
                let token = self.stream.current().clone();
                if token.test(TokenType::Punctuation, Some("[")) {
                    if !is_null_safe {
                        return Err(Error::syntax(
                            "Expected name.",
                            token.cursor(),
                            self.expression(),
                        ));
                    }
                    self.stream.next()?;
                    let arg = self.parse_expression(0)?;
                    self.stream
                        .expect(TokenType::Punctuation, Some("]"), None)?;
                    node = Node::GetAttr(GetAttrNode::with_null_safe(
                        node,
                        arg,
                        ArrayNode::arguments(),
                        GetAttrType::Array,
                        true,
                    ));
                } else {
                    self.stream.next()?;
                    // Operators like "not" and "matches" are valid method or
                    // property names: the lexer reads them as operators
                    // first. Other types (strings, numbers) are not names.
                    if token.kind != TokenType::Name
                        && (token.kind != TokenType::Operator
                            || match_name(token.value_bytes(), 0).is_none())
                    {
                        return Err(Error::syntax(
                            "Expected name.",
                            token.cursor(),
                            self.expression(),
                        ));
                    }
                    let arg = ConstantNode::with(token.value.clone(), true, is_null_safe);
                    let mut arguments = ArrayNode::arguments();
                    let kind = if self
                        .stream
                        .current()
                        .test(TokenType::Punctuation, Some("("))
                    {
                        if let Node::Node(nodes) = self.parse_arguments()? {
                            for n in nodes {
                                arguments.add_element(n, None);
                            }
                        }
                        GetAttrType::Method
                    } else {
                        GetAttrType::Property
                    };
                    node = Node::GetAttr(GetAttrNode::new(node, arg, arguments, kind));
                }
            } else if value == b"[" {
                self.enter_nesting_level()?;
                self.stream.next()?;
                let arg = self.parse_expression(0)?;
                self.stream
                    .expect(TokenType::Punctuation, Some("]"), None)?;
                node = Node::GetAttr(GetAttrNode::new(
                    node,
                    arg,
                    ArrayNode::arguments(),
                    GetAttrType::Array,
                ));
            } else {
                break;
            }
        }
        Ok(node)
    }

    /// `parseArguments()`: a `Node` holding the arguments.
    fn parse_arguments(&mut self) -> Result<Node, Error> {
        let mut args = Vec::new();
        self.stream.expect(
            TokenType::Punctuation,
            Some("("),
            Some("A list of arguments must begin with an opening parenthesis"),
        )?;
        while !self
            .stream
            .current()
            .test(TokenType::Punctuation, Some(")"))
        {
            if !args.is_empty() {
                self.stream.expect(
                    TokenType::Punctuation,
                    Some(","),
                    Some("Arguments must be separated by a comma"),
                )?;
            }
            args.push(self.parse_expression(0)?);
        }
        self.stream.expect(
            TokenType::Punctuation,
            Some(")"),
            Some("A list of arguments must be closed by a parenthesis"),
        )?;
        Ok(Node::Node(args))
    }
}
