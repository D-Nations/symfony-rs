//! ReaderTest.php at symfony/css-selector v8.1.6. Upstream sets the
//! position by reflection; `set_position` stands in. Patterns are
//! `regex::bytes` ones, with upstream's text.
//!
//! Upstream: symfony/css-selector Tests/Parser/ReaderTest.php

use regex::bytes::Regex;
use symfony_css_selector::parser::Reader;

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

#[test]
fn testIsEOF() {
    let reader = Reader::new(b"");
    assert!(reader.is_eof());

    let mut reader = Reader::new(b"hello");
    assert!(!reader.is_eof());

    reader.set_position(2);
    assert!(!reader.is_eof());

    reader.set_position(5);
    assert!(reader.is_eof());
}

#[test]
fn testGetRemainingLength() {
    let mut reader = Reader::new(b"hello");
    assert_eq!(5, reader.get_remaining_length());

    reader.set_position(2);
    assert_eq!(3, reader.get_remaining_length());

    reader.set_position(5);
    assert_eq!(0, reader.get_remaining_length());
}

#[test]
fn testGetSubstring() {
    let mut reader = Reader::new(b"hello");
    assert_eq!(b"he", reader.get_substring(2, 0));
    assert_eq!(b"el", reader.get_substring(2, 1));

    reader.set_position(2);
    assert_eq!(b"ll", reader.get_substring(2, 0));
    assert_eq!(b"lo", reader.get_substring(2, 1));
}

#[test]
fn testGetOffset() {
    let mut reader = Reader::new(b"hello");
    assert_eq!(Some(2), reader.get_offset(b"ll"));
    assert_eq!(None, reader.get_offset(b"w"));

    reader.set_position(2);
    assert_eq!(Some(0), reader.get_offset(b"ll"));
    assert_eq!(None, reader.get_offset(b"he"));
}

#[test]
fn testFindPattern() {
    let mut reader = Reader::new(b"hello");

    assert_eq!(None, reader.find_pattern(&re("world")));
    assert_eq!(
        Some(vec![b"hello".to_vec(), b"h".to_vec()]),
        reader.find_pattern(&re("^([a-z]).*"))
    );

    reader.set_position(2);
    assert_eq!(None, reader.find_pattern(&re("^h.*")));
    assert_eq!(
        Some(vec![b"llo".to_vec()]),
        reader.find_pattern(&re("^llo$"))
    );
}

#[test]
fn testMoveForward() {
    let mut reader = Reader::new(b"hello");
    assert_eq!(0, reader.get_position());

    reader.move_forward(2);
    assert_eq!(2, reader.get_position());
}

#[test]
fn testToEnd() {
    let mut reader = Reader::new(b"hello");
    reader.move_to_end();
    assert!(reader.is_eof());
}
