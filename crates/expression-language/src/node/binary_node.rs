//! Port of Symfony\Component\ExpressionLanguage\Node\BinaryNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/BinaryNode.php

use php_pcre::{Pcre, PcreError};
use php_value::{PhpArray, PhpString, Value};

use super::Node;
use crate::{
    compiler::Compiler,
    expression_function::Functions,
    php,
    syntax_error::{Class, Error},
};

/// The PHP code `matches` compiles to, before its two arguments.
pub const MATCHES_FUNCTION: &str = "(static function ($regexp, $str) { set_error_handler(static fn ($t, $m) => throw new \\Symfony\\Component\\ExpressionLanguage\\SyntaxError(sprintf('Regexp \"%s\" passed to \"matches\" is not valid', $regexp).substr($m, 12))); try { return preg_match($regexp, (string) $str); } finally { restore_error_handler(); } })(";

/// A binary operation.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryNode {
    pub operator: String,
    pub left: Box<Node>,
    pub right: Box<Node>,
}

/// `self::FUNCTIONS`: the operators that compile to a function call.
fn function(operator: &str) -> Option<&'static str> {
    Some(match operator {
        "**" => "pow",
        ".." => "range",
        "in" => "\\in_array",
        "not in" => "!\\in_array",
        "contains" => "str_contains",
        "starts with" => "str_starts_with",
        "ends with" => "str_ends_with",
        _ => return None,
    })
}

impl BinaryNode {
    pub fn new(operator: impl Into<String>, left: impl Into<Node>, right: impl Into<Node>) -> Self {
        Self {
            operator: operator.into(),
            left: Box::new(left.into()),
            right: Box::new(right.into()),
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        let operator = self.operator.as_str();
        if operator == "matches" {
            match &*self.right {
                Node::Constant(c) => {
                    evaluate_matches(&c.value, &Value::from(""))?;
                }
                Node::Binary(b) if b.operator != "~" => {
                    return Err(Error::syntax(
                        "The regex passed to \"matches\" must be a string.",
                        0,
                        b"",
                    ));
                }
                _ => {}
            }
            compiler
                .raw(MATCHES_FUNCTION)
                .compile(&self.right)?
                .raw(", ")
                .compile(&self.left)?
                .raw(")");
            return Ok(());
        }

        if let Some(function) = function(operator) {
            compiler
                .raw(function)
                .raw("(")
                .compile(&self.left)?
                .raw(", ")
                .compile(&self.right)?;
            if operator == "in" || operator == "not in" {
                compiler.raw(", true");
            }
            compiler.raw(")");
            return Ok(());
        }

        let operator = match operator {
            "~" => ".",
            "and" => "&&",
            "or" => "||",
            other => other,
        };
        compiler
            .raw("(")
            .compile(&self.left)?
            .raw(" ")
            .raw(operator)
            .raw(" ")
            .compile(&self.right)?
            .raw(")");
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let operator = self.operator.as_str();
        let left = self.left.evaluate(functions, values)?;

        if function(operator).is_some() {
            let right = self.right.evaluate(functions, values)?;
            return match operator {
                "in" => Ok(Value::Bool(php::in_array_strict(&left, &right)?)),
                "not in" => Ok(Value::Bool(!php::in_array_strict(&left, &right)?)),
                "**" => php::pow(&left, &right),
                ".." => php::range(&left, &right),
                "contains" => php::string_test("str_contains", &left, &right),
                "starts with" => php::string_test("str_starts_with", &left, &right),
                _ => php::string_test("str_ends_with", &left, &right),
            };
        }

        match operator {
            "or" | "||" => {
                return Ok(Value::Bool(
                    left.is_truthy() || self.right.evaluate(functions, values)?.is_truthy(),
                ));
            }
            "xor" => {
                return Ok(Value::Bool(
                    left.is_truthy() ^ self.right.evaluate(functions, values)?.is_truthy(),
                ));
            }
            "and" | "&&" => {
                return Ok(Value::Bool(
                    left.is_truthy() && self.right.evaluate(functions, values)?.is_truthy(),
                ));
            }
            _ => {}
        }

        let right = self.right.evaluate(functions, values)?;
        match operator {
            "|" | "^" | "&" => php::bitwise(&left, operator, &right),
            "<<" | ">>" => php::shift(&left, operator, &right),
            "==" => Ok(Value::Bool(php::equals(&left, &right))),
            "===" => Ok(Value::Bool(left.identical(&right))),
            "!=" => Ok(Value::Bool(!php::equals(&left, &right))),
            "!==" => Ok(Value::Bool(!left.identical(&right))),
            "<" => Ok(Value::Bool(php::less(&left, &right)?)),
            ">" => Ok(Value::Bool(php::greater(&left, &right)?)),
            ">=" => Ok(Value::Bool(php::greater_or_equal(&left, &right)?)),
            "<=" => Ok(Value::Bool(php::less_or_equal(&left, &right)?)),
            "+" => php::add(&left, &right),
            "-" => php::sub(&left, &right),
            "~" => php::concat(&left, &right),
            "*" => php::mul(&left, &right),
            "/" => {
                if php::equals(&Value::Int(0), &right) {
                    return Err(Error::new(Class::DivisionByZeroError, "Division by zero."));
                }
                php::div(&left, &right)
            }
            "%" => {
                if php::equals(&Value::Int(0), &right) {
                    return Err(Error::new(Class::DivisionByZeroError, "Modulo by zero."));
                }
                php::modulo(&left, &right)
            }
            "matches" => evaluate_matches(&right, &left),
            _ => Err(Error::new(
                Class::LogicException,
                format!(
                    "\"Symfony\\Component\\ExpressionLanguage\\Node\\BinaryNode\" does not support the \"{operator}\" operator."
                ),
            )),
        }
    }
}

/// A value passed to `evaluateMatches()`'s `string` (or `?string`)
/// parameter, in PHP's coercive mode for a user function.
fn matches_argument(
    value: &Value,
    position: usize,
    name: &str,
    nullable: bool,
) -> Result<PhpString, Error> {
    let error = || {
        Error::type_error(format!(
            "Symfony\\Component\\ExpressionLanguage\\Node\\BinaryNode::evaluateMatches(): Argument #{position} (${name}) must be of type {}string, {} given",
            if nullable { "?" } else { "" },
            crate::php::value_name(value)
        ))
    };
    match value {
        Value::Null if nullable => Ok(PhpString::default()),
        Value::Null | Value::Array(_) => Err(error()),
        Value::Object(o) => o.to_php_string().ok_or_else(error),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// `evaluateMatches($regexp, $str)`: `preg_match()`, with a pattern
/// PHP rejects turned into a `SyntaxError`.
pub(crate) fn evaluate_matches(regexp: &Value, str: &Value) -> Result<Value, Error> {
    let regexp = matches_argument(regexp, 1, "regexp", false)?;
    let subject = matches_argument(str, 2, "str", true)?;
    let pcre = match Pcre::new(regexp.as_bytes()) {
        Ok(pcre) => pcre,
        Err(PcreError::Compile(message)) => {
            let mut text = b"Regexp \"".to_vec();
            text.extend_from_slice(regexp.as_bytes());
            text.extend_from_slice(b"\" passed to \"matches\" is not valid: ");
            text.extend_from_slice(message.as_bytes());
            return Err(Error::syntax(text, 0, b""));
        }
        Err(PcreError::Unsupported(message)) => {
            return Err(Error::new(Class::Unsupported, message));
        }
    };
    // preg_match() returns false on a matching error, which the int
    // return type turns into 0
    Ok(Value::Int(i64::from(
        pcre.is_match(subject.as_bytes()).unwrap_or(false),
    )))
}
