//! TokenStreamTest.php at symfony/css-selector v8.1.6. `assertSame` on
//! tokens becomes equality: the stream hands out copies, and the tokens
//! compared differ in value or position.
//!
//! Upstream: symfony/css-selector Tests/Parser/TokenStreamTest.php

use symfony_css_selector::Kind;
use symfony_css_selector::parser::{Token, TokenStream, TokenType};

fn t(type_: TokenType, value: &[u8], position: usize) -> Token {
    Token::new(Some(type_), Some(value), Some(position))
}

#[test]
fn testGetNext() {
    let mut stream = TokenStream::new();
    let t1 = t(TokenType::Identifier, b"h1", 0);
    let t2 = t(TokenType::Delimiter, b".", 2);
    let t3 = t(TokenType::Identifier, b"title", 3);
    stream.push(t1.clone()).push(t2.clone()).push(t3.clone());

    assert_eq!(Ok(t1), stream.get_next());
    assert_eq!(Ok(t2), stream.get_next());
    assert_eq!(Ok(t3), stream.get_next());
}

#[test]
fn testGetPeek() {
    let mut stream = TokenStream::new();
    let t1 = t(TokenType::Identifier, b"h1", 0);
    let t2 = t(TokenType::Delimiter, b".", 2);
    let t3 = t(TokenType::Identifier, b"title", 3);
    stream.push(t1.clone()).push(t2.clone()).push(t3);

    assert_eq!(Ok(t1.clone()), stream.get_peek());
    assert_eq!(Ok(t1), stream.get_next());
    assert_eq!(Ok(t2.clone()), stream.get_peek());
    assert_eq!(Ok(t2.clone()), stream.get_peek());
    assert_eq!(Ok(t2), stream.get_next());
}

#[test]
fn testGetNextIdentifier() {
    let mut stream = TokenStream::new();
    stream.push(t(TokenType::Identifier, b"h1", 0));

    assert_eq!(Ok(b"h1".to_vec()), stream.get_next_identifier());
}

#[test]
fn testFailToGetNextIdentifier() {
    let mut stream = TokenStream::new();
    stream.push(t(TokenType::Delimiter, b".", 2));

    let error = stream.get_next_identifier().unwrap_err();
    assert_eq!(Kind::SyntaxError, error.kind());
}

#[test]
fn testGetNextIdentifierOrStar() {
    let mut stream = TokenStream::new();

    stream.push(t(TokenType::Identifier, b"h1", 0));
    assert_eq!(
        Ok(Some(b"h1".to_vec())),
        stream.get_next_identifier_or_star()
    );

    stream.push(t(TokenType::Delimiter, b"*", 0));
    assert_eq!(Ok(None), stream.get_next_identifier_or_star());
}

#[test]
fn testFailToGetNextIdentifierOrStar() {
    let mut stream = TokenStream::new();
    stream.push(t(TokenType::Delimiter, b".", 2));

    let error = stream.get_next_identifier_or_star().unwrap_err();
    assert_eq!(Kind::SyntaxError, error.kind());
}

#[test]
fn testSkipWhitespace() {
    let mut stream = TokenStream::new();
    let t1 = t(TokenType::Identifier, b"h1", 0);
    let t2 = t(TokenType::Whitespace, b" ", 2);
    let t3 = t(TokenType::Identifier, b"h1", 3);
    stream.push(t1.clone()).push(t2).push(t3.clone());

    stream.skip_whitespace().unwrap();
    assert_eq!(Ok(t1), stream.get_next());

    stream.skip_whitespace().unwrap();
    assert_eq!(Ok(t3), stream.get_next());
}
