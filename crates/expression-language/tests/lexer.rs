//! LexerTest.php at symfony/expression-language v8.1.9. Names and
//! assertions are PHPUnit's; `assertEquals` on tokens compares numbers as
//! PHPUnit does (`phpunit_equals`).
//!
//! Upstream: symfony/expression-language Tests/LexerTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

use php_value::{Value, vectors::phpunit_equals};
use symfony_expression_language::{
    Class, Lexer, Token, TokenStream, TokenType, lexer::operator_regex,
};

fn token(kind: &str, value: impl Into<Value>, cursor: usize) -> Token {
    Token::new(
        TokenType::from_name(kind).expect("token type"),
        value,
        cursor,
    )
}

/// `assertEquals(new TokenStream($tokens, $expression), $actual)`:
/// numbers compare as PHPUnit compares them (1990 equals 1990.0).
fn assert_stream_equals(expected: &TokenStream, actual: &TokenStream) {
    assert_eq!(expected.expression(), actual.expression());
    assert_eq!(expected.tokens().len(), actual.tokens().len(), "{actual}");
    for (e, a) in expected.tokens().iter().zip(actual.tokens()) {
        assert_eq!(e.kind, a.kind, "{actual}");
        assert_eq!(e.cursor, a.cursor, "{actual}");
        assert!(
            phpunit_equals(&e.value, &a.value),
            "{:?} != {:?}",
            e.value,
            a.value
        );
    }
}

fn get_tokenize_data() -> Vec<(Vec<Token>, &'static str)> {
    vec![
        (vec![token("name", "a", 3)], "  a  "),
        (vec![token("name", "a", 1)], "a"),
        (vec![token("string", "foo", 1)], "\"foo\""),
        (
            vec![token("string", "foo\"bar\\baz", 1)],
            "\"foo\\\"bar\\\\baz\"",
        ),
        (vec![token("number", 3, 1)], "3"),
        (vec![token("operator", "+", 1)], "+"),
        (vec![token("punctuation", ".", 1)], "."),
        (
            vec![
                token("punctuation", "(", 1),
                token("number", 3, 2),
                token("operator", "+", 4),
                token("number", 5, 6),
                token("punctuation", ")", 7),
                token("operator", "~", 9),
                token("name", "foo", 11),
                token("punctuation", "(", 14),
                token("string", "bar", 15),
                token("punctuation", ")", 20),
                token("punctuation", ".", 21),
                token("name", "baz", 22),
                token("punctuation", "[", 25),
                token("number", 4, 26),
                token("punctuation", "]", 27),
                token("operator", "-", 29),
                token("number", 1990, 31),
                token("operator", "+", 39),
                token("operator", "~", 41),
                token("name", "qux", 42),
            ],
            "(3 + 5) ~ foo(\"bar\").baz[4] - 1.99E+3 + ~qux",
        ),
        (vec![token("operator", "..", 1)], ".."),
        (
            vec![
                token("number", 23, 1),
                token("operator", "..", 3),
                token("number", 26, 5),
            ],
            "23..26",
        ),
        (vec![token("string", "#foo", 1)], "'#foo'"),
        (vec![token("string", "#foo", 1)], "\"#foo\""),
        (
            vec![
                token("name", "foo", 1),
                token("punctuation", ".", 4),
                token("name", "not", 5),
                token("operator", "in", 9),
                token("punctuation", "[", 12),
                token("name", "bar", 13),
                token("punctuation", "]", 16),
            ],
            "foo.not in [bar]",
        ),
        (vec![token("number", 0.787, 1)], "0.787"),
        (vec![token("number", 0.1234, 1)], ".1234"),
        (vec![token("number", 188_165.117_8, 1)], "188_165.1_178"),
        (
            vec![
                token("operator", "-", 1),
                token("number", 7_189_000_000.0, 2),
            ],
            "-.7_189e+10",
        ),
        (vec![token("number", 65536, 1)], "65536 /* this is 2^16 */"),
        (
            vec![
                token("number", 2, 1),
                token("operator", "*", 21),
                token("number", 4, 23),
            ],
            "2 /* /* comment1 */ * 4",
        ),
        (
            vec![
                token("string", "/* this is", 1),
                token("operator", "~", 14),
                token("string", "not a comment */", 16),
            ],
            "\"/* this is\" ~ \"not a comment */\"",
        ),
        (
            vec![token("string", "/* this is not a comment */", 1)],
            "\"/* this is not a comment */\"",
        ),
        (
            vec![
                token("name", "foo", 1),
                token("operator", "xor", 5),
                token("name", "bar", 9),
            ],
            "foo xor bar",
        ),
    ]
}

#[test]
fn testTokenize() {
    for (mut tokens, expression) in get_tokenize_data() {
        tokens.push(Token {
            kind: TokenType::Eof,
            value: Value::Null,
            cursor: Some(expression.len() + 1),
        });
        let actual = Lexer::new().tokenize(expression).expect(expression);
        assert_stream_equals(&TokenStream::new(tokens, expression), &actual);
    }
}

#[test]
fn testTokenizeMultilineComment() {
    let expression = "/**\n * This is 2^16, even if we could have\n * used the hexadecimal representation 0x10000. Just a\n * matter of taste! \u{1f642}\n */\n65536 and\n/*\n This time, only one star to start the comment and that's it.\n It is valid too!\n */\n65535 /* this is 2^16 - 1 */";
    let tokens = vec![
        token("number", 65536, 128),
        token("operator", "and", 134),
        token("number", 65535, 225),
        Token {
            kind: TokenType::Eof,
            value: Value::Null,
            cursor: Some(expression.len() + 1),
        },
    ];
    let actual = Lexer::new().tokenize(expression).unwrap();
    assert_stream_equals(
        &TokenStream::new(tokens, expression.replace('\n', " ")),
        &actual,
    );
}

fn assert_syntax_error(expression: &str, message: &str) {
    let error = Lexer::new().tokenize(expression).unwrap_err();
    assert_eq!(error.class(), &Class::SyntaxError);
    assert!(
        error
            .message()
            .as_bytes()
            .windows(message.len())
            .any(|w| w == message.as_bytes()),
        "{error}"
    );
}

#[test]
fn testTokenizeThrowsErrorWithMessage() {
    assert_syntax_error(
        "service(faulty.expression.example').dummyMethod()",
        "Unexpected character \"'\" around position 33 for expression `service(faulty.expression.example').dummyMethod()`.",
    );
}

#[test]
fn testTokenizeThrowsErrorOnUnclosedBrace() {
    assert_syntax_error(
        "service(unclosed.expression.dummyMethod()",
        "Unclosed \"(\" around position 7 for expression `service(unclosed.expression.dummyMethod()`.",
    );
}

#[test]
fn testTokenizeOnNotOpenedBracket() {
    assert_syntax_error(
        "service)not.opened.expression.dummyMethod()",
        "Unexpected \")\" around position 7 for expression `service)not.opened.expression.dummyMethod()`.",
    );
}

#[test]
fn testTokenizeUnterminatedStringIsLinear() {
    let expression = format!("\"{}\\", "\\a".repeat(30));
    let start = std::time::Instant::now();
    let error = Lexer::new().tokenize(&expression).unwrap_err();
    assert_eq!(error.class(), &Class::SyntaxError);
    let elapsed = start.elapsed().as_secs_f64();
    assert!(
        elapsed < 0.5,
        "Tokenizing an unterminated string took {elapsed:.3}s, suggesting a ReDoS regression."
    );
}

/// The regex `Resources/bin/generate_operator_regex.php` prints is in
/// upstream's Lexer.php; the lexer here matches the same alternation.
#[test]
fn testOperatorRegexWasGeneratedWithScript() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../upstream/symfony/expression-language/Lexer.php"
    );
    let Ok(source) = std::fs::read_to_string(path) else {
        eprintln!("upstream/ not fetched: skipping testOperatorRegexWasGeneratedWithScript");
        return;
    };
    let regex = operator_regex();
    assert!(
        source.contains(&regex),
        "You need to run \"generate_operator_regex.php\" to generate the operator regex: {regex}"
    );
}
