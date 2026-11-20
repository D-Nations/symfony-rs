//! The Parser tests at symfony/css-selector v8.1.6, one module per
//! upstream test class; AbstractHandlerTestCase's tests and assertions
//! are the helpers below.
//!
//! Upstream: none
#![allow(non_snake_case)]

#[path = "../common/mod.rs"]
mod common;

mod class_parser_test;
mod comment_handler_test;
mod element_parser_test;
mod empty_string_parser_test;
mod hash_handler_test;
mod hash_parser_test;
mod identifier_handler_test;
mod number_handler_test;
mod parser_test;
mod reader_test;
mod string_handler_test;
mod token_stream_test;
mod whitespace_handler_test;

use common::{V, show, token};
use symfony_css_selector::parser::handler::HandlerInterface;
use symfony_css_selector::parser::{
    ParserInterface, Reader, TokenStream, TokenizerEscaping, TokenizerPatterns,
};

fn patterns() -> TokenizerPatterns {
    TokenizerPatterns::new()
}

fn escaping() -> TokenizerEscaping {
    TokenizerEscaping::new(patterns())
}

/// AbstractHandlerTestCase::testHandleValue.
fn handle_value(handler: &dyn HandlerInterface, rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        let mut reader = Reader::new(&args[0].bytes());
        let mut stream = TokenStream::new();
        assert_eq!(
            Ok(true),
            handler.handle(&mut reader, &mut stream),
            "row {name}"
        );
        assert_eq!(Ok(token(&args[1])), stream.get_next(), "row {name}");
        assert_remaining_content(&reader, &args[2].bytes(), name);
    }
}

/// CommentHandlerTest::testHandleValue.
fn handle_value_without_token(handler: &dyn HandlerInterface, rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        let mut reader = Reader::new(&args[0].bytes());
        let mut stream = TokenStream::new();
        assert_eq!(
            Ok(true),
            handler.handle(&mut reader, &mut stream),
            "row {name}"
        );
        assert!(stream.tokens().is_empty(), "row {name}");
        assert_remaining_content(&reader, &args[2].bytes(), name);
    }
}

/// AbstractHandlerTestCase::testDontHandleValue.
fn dont_handle_value(handler: &dyn HandlerInterface, rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        let value = args[0].bytes();
        let mut reader = Reader::new(&value);
        let mut stream = TokenStream::new();
        assert_eq!(
            Ok(false),
            handler.handle(&mut reader, &mut stream),
            "row {name}"
        );
        assert!(stream.tokens().is_empty(), "row {name}");
        assert_remaining_content(&reader, &value, name);
    }
}

fn assert_remaining_content(reader: &Reader, remaining_content: &[u8], row: &str) {
    if remaining_content.is_empty() {
        assert_eq!(0, reader.get_remaining_length(), "row {row}");
        assert!(reader.is_eof(), "row {row}");
    } else {
        assert_eq!(
            remaining_content.len() as i64,
            reader.get_remaining_length(),
            "row {row}"
        );
        assert_eq!(Some(0), reader.get_offset(remaining_content), "row {row}");
    }
}

/// The shortcut parser tests' testParse: one selector whose tree prints
/// as given.
fn shortcut_parse(parser: &dyn ParserInterface, rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        let selectors = parser.parse(&args[0].bytes()).expect("parse");
        assert_eq!(1, selectors.len(), "row {name}");
        assert_eq!(
            show(&args[1].bytes()),
            show(&selectors[0].get_tree().to_php_string()),
            "row {name}"
        );
    }
}
