//! Port of the symfony/expression-language component. See README.md for
//! the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod compiler;
pub mod expression;
pub mod expression_function;
pub mod expression_language;
pub mod lexer;
pub mod node;
pub mod object;
pub mod parsed_expression;
pub mod parser;
pub mod php;
pub mod syntax_error;
pub mod token;
pub mod token_stream;

pub use compiler::Compiler;
pub use expression::{Expression, ExpressionInput};
pub use expression_function::{
    ExpressionFunction, ExpressionFunctionProvider, Functions, PhpFunctions,
};
pub use expression_language::{ArrayCache, CacheItemPool, ExpressionLanguage};
pub use lexer::Lexer;
pub use node::Node;
pub use object::MethodObject;
pub use parsed_expression::ParsedExpression;
pub use parser::{IGNORE_UNKNOWN_FUNCTIONS, IGNORE_UNKNOWN_VARIABLES, Parser};
pub use syntax_error::{Class, Error};
pub use token::{Token, TokenType};
pub use token_stream::TokenStream;
