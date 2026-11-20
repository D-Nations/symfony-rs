//! GitignoreTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; the rows of `provider` and `providerExtended` (both feed
//! testToRegex) and of `provideNegatedPatternsCases` are replayed from
//! tests/data/providers.json. `assertMatchesRegularExpression` is
//! `preg_match`, here pcre.rs.
//!
//! Upstream: symfony/finder Tests/GitignoreTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, bytes, provider};
use php_value::Value;
use symfony_finder::{Gitignore, pcre::Pcre};

fn list(v: &Value) -> Vec<Vec<u8>> {
    match v {
        Value::Array(a) => a.values().map(bytes).collect(),
        other => panic!("not a list: {other:?}"),
    }
}

fn implode(glue: &[u8], lines: &[Vec<u8>]) -> Vec<u8> {
    lines.join(glue)
}

fn check(
    to_regex: fn(&[u8]) -> Result<Vec<u8>, symfony_finder::Error>,
    rows: Vec<(String, Vec<Value>)>,
) -> TestResult {
    for (name, args) in rows {
        let lines = list(&args[0]);
        let patterns = implode(b"\n", &lines);
        let regex = to_regex(&patterns)?;
        assert_eq!(regex, to_regex(&implode(b"\r\n", &lines))?, "row {name}");
        assert_eq!(regex, to_regex(&implode(b"\r", &lines))?, "row {name}");
        let compiled = Pcre::new(&regex)?;
        for case in list(&args[1]) {
            assert!(
                compiled.is_match(&case)?,
                "Failed asserting path:\n{}\nmatches gitignore patterns:\n{}",
                String::from_utf8_lossy(&case),
                String::from_utf8_lossy(&patterns)
            );
        }
        for case in list(&args[2]) {
            assert!(
                !compiled.is_match(&case)?,
                "Failed asserting path:\n{}\nNOT matching gitignore patterns:\n{}",
                String::from_utf8_lossy(&case),
                String::from_utf8_lossy(&patterns)
            );
        }
    }
    Ok(())
}

#[test]
fn testToRegex() -> TestResult {
    let rows = provider("GitignoreTest", "testToRegex");
    assert_eq!(rows.len(), 476);
    check(Gitignore::to_regex, rows)
}

#[test]
fn testToRegexWithMorePatternsThanTheRegexNestingLimit() -> TestResult {
    let lines: Vec<String> = (1..=300).map(|i| format!("dir{i}/")).collect();

    let regex = Pcre::new(&Gitignore::to_regex(lines.join("\n").as_bytes())?)?;
    assert!(regex.is_match(b"dir300/file.txt")?);
    assert!(!regex.is_match(b"other/file.txt")?);

    let negated: Vec<String> = lines.iter().map(|l| format!("!{l}")).collect();
    let negated_regex = Pcre::new(&Gitignore::to_regex_matching_negated_patterns(
        negated.join("\n").as_bytes(),
    )?)?;
    assert!(negated_regex.is_match(b"dir300/file.txt")?);
    assert!(!negated_regex.is_match(b"other/file.txt")?);
    Ok(())
}

#[test]
fn testToRegexMatchingNegatedPatterns() -> TestResult {
    let rows = provider("GitignoreTest", "testToRegexMatchingNegatedPatterns");
    assert_eq!(rows.len(), 59);
    check(Gitignore::to_regex_matching_negated_patterns, rows)
}
