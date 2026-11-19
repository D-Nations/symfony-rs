//! ParseExceptionTest.php at symfony/yaml
//! 6107ee26a25215c3df3af0a3315e56bb4fd0b7db. Names and assertions are
//! PHPUnit's. The tests after them pin the message rules PHP applies.
//!
//! Upstream: symfony/yaml Tests/ParseExceptionTest.php
#![allow(non_snake_case)]

use symfony_yaml::ParseException;

fn message(exception: &ParseException) -> String {
    exception.get_message().to_string()
}

#[test]
fn testGetMessage() {
    let exception = ParseException::new(
        "Error message",
        42,
        Some("foo: bar".into()),
        Some("/var/www/app/config.yml".to_owned()),
    );
    let expected = "Error message in \"/var/www/app/config.yml\" at line 42 (near \"foo: bar\")";

    assert_eq!(expected, message(&exception));
}

#[test]
fn testGetMessageWithUnicodeInFilename() {
    let exception = ParseException::new(
        "Error message",
        42,
        Some("foo: bar".into()),
        Some("äöü.yml".to_owned()),
    );
    let expected = "Error message in \"äöü.yml\" at line 42 (near \"foo: bar\")";

    assert_eq!(expected, message(&exception));
}

#[test]
fn the_final_dot_moves_after_the_details() {
    let exception = ParseException::new("Bad.", 3, Some("x".into()), None);
    assert_eq!("Bad at line 3 (near \"x\").", message(&exception));
}

#[test]
fn a_falsy_snippet_and_a_negative_line_are_left_out() {
    let exception = ParseException::new("Bad", -1, Some("0".into()), None);
    assert_eq!("Bad", message(&exception));
    let mut exception = ParseException::new("Bad", 0, Some("".into()), None);
    assert_eq!("Bad at line 0", message(&exception));
    exception.set_snippet("y".into());
    exception.set_parsed_file("a\"b.yml");
    assert_eq!(
        "Bad in \"a\\\"b.yml\" at line 0 (near \"y\")",
        message(&exception)
    );
}
