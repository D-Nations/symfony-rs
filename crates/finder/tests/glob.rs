//! GlobTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; `preg_match` is pcre.rs.
//!
//! Upstream: symfony/finder Tests/GlobTest.php
#![allow(non_snake_case)]

mod common;

use common::TestResult;
use symfony_finder::{Glob, pcre::Pcre};

fn to_regex(glob: &str) -> String {
    to_regex_with(glob, true, true, Some(b'#'))
}

fn to_regex_with(glob: &str, dot: bool, slash: bool, delimiter: Option<u8>) -> String {
    String::from_utf8_lossy(&Glob::to_regex(glob.as_bytes(), dot, slash, delimiter)).into_owned()
}

fn preg_match(regex: &str, subject: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    Ok(i32::from(
        Pcre::new(regex.as_bytes())?.is_match(subject.as_bytes())?,
    ))
}

#[test]
fn testGlobToRegexDelimiters() {
    assert_eq!(r"#^(?=[^\.])\#$#", to_regex("#"));
    assert_eq!(r"#^\.[^/]*$#", to_regex(".*"));
    assert_eq!(r"^\.[^/]*$", to_regex_with(".*", true, true, None));
    assert_eq!(r"/^\.[^/]*$/", to_regex_with(".*", true, true, Some(b'/')));
}

#[test]
fn testGlobToRegexBracketExpressions() -> TestResult {
    let regex = to_regex("a[*?]");
    assert_eq!(1, preg_match(&regex, "a*")?);
    assert_eq!(1, preg_match(&regex, "a?")?);
    assert_eq!(0, preg_match(&regex, "ab")?);
    assert_eq!(0, preg_match(&regex, "a]")?);

    let regex = to_regex(r"[a\-c]");
    assert_eq!(1, preg_match(&regex, "-")?);
    assert_eq!(1, preg_match(&regex, "c")?);
    assert_eq!(0, preg_match(&regex, "b")?);

    assert_eq!(r"#^(?=[^\.])file[0-9]\.txt$#", to_regex("file[0-9].txt"));
    assert_eq!(r"#^(?=[^\.])[[:alpha:]]$#", to_regex("[[:alpha:]]"));
    Ok(())
}

#[test]
fn testGlobToRegexDoubleStarMatchesRootFiles() -> TestResult {
    let regex = to_regex("**/*.txt");
    assert_eq!(1, preg_match(&regex, "file.txt")?);
    assert_eq!(1, preg_match(&regex, "foo/file.txt")?);
    assert_eq!(1, preg_match(&regex, "foo/bar/baz.txt")?);
    assert_eq!(1, preg_match(&regex, "/foo/bar.txt")?);
    assert_eq!(0, preg_match(&regex, "./foo/bar.txt")?);
    assert_eq!(0, preg_match(&regex, "foo/.bar/bar.txt")?);
    assert_eq!(0, preg_match(&regex, ".file.txt")?);
    assert_eq!(0, preg_match(&regex, "foo/bar/baz.php")?);

    let regex = to_regex_with("**/*.txt", false, true, Some(b'#'));
    assert_eq!(1, preg_match(&regex, "./foo/bar.txt")?);
    assert_eq!(1, preg_match(&regex, "foo/.bar/bar.txt")?);
    assert_eq!(1, preg_match(&regex, ".file.txt")?);
    Ok(())
}

#[test]
fn testGlobToRegexMergesConsecutiveStarsWhenWildcardsCrossSlashes() {
    assert_eq!(
        r"#^(?=[^\.])/?(?=[^\.]).*/(?=[^\.])controllers\.json$#",
        to_regex_with("**/controllers.json", true, false, Some(b'#'))
    );
    assert_eq!(
        r"#^(?=[^\.])foo/(?=[^\.]).*/(?=[^\.])bar$#",
        to_regex_with("foo/**/bar", true, false, Some(b'#'))
    );
    assert_eq!(
        r"#^(?=[^\.]).*$#",
        to_regex_with("**", true, false, Some(b'#'))
    );
    assert_eq!(
        r"#^(?=[^\.])a\*.*$#",
        to_regex_with(r"a\**", true, false, Some(b'#'))
    );
}

#[test]
fn testGlobToRegexEscapedBrackets() -> TestResult {
    let regex = to_regex(r"file\[1\].txt");
    assert_eq!(r"#^(?=[^\.])file\[1\]\.txt$#", regex);
    assert_eq!(1, preg_match(&regex, "file[1].txt")?);
    assert_eq!(0, preg_match(&regex, "file1.txt")?);
    assert_eq!(1, preg_match(&to_regex(r"a\[b"), "a[b")?);
    assert_eq!(1, preg_match(&to_regex("file[0-9].txt"), "file1.txt")?);

    let regex = to_regex(r"[a\]]");
    assert_eq!(1, preg_match(&regex, "]")?);
    assert_eq!(0, preg_match(&regex, "a]")?);
    Ok(())
}

#[test]
fn testGlobToRegexUnclosedBrackets() -> TestResult {
    assert_eq!(r"#^(?=[^\.])a\[b$#", to_regex("a[b"));
    assert_eq!(r"#^(?=[^\.])\[]$#", to_regex("[]"));
    assert_eq!(r"#^(?=[^\.])\[a\]$#", to_regex(r"[a\]"));
    assert_eq!(r"#^(?=[^\.])\[a\[b$#", to_regex("[a[b"));
    assert_eq!(r"#^(?=[^\.])\[[^/]*\.txt$#", to_regex("[*.txt"));
    assert_eq!(1, preg_match(&to_regex("a[b"), "a[b")?);
    assert_eq!(1, preg_match(&to_regex("[]"), "[]")?);
    assert_eq!(1, preg_match(&to_regex("[*.txt"), "[foo.txt")?);
    assert_eq!(0, preg_match(&to_regex("[*.txt"), "^.txt")?);
    assert_eq!(r"#^(?=[^\.])[]]$#", to_regex("[]]"));
    assert_eq!(r"#^(?=[^\.])[a[]$#", to_regex("[a[]"));
    Ok(())
}

#[test]
fn testGlobToRegexBracketsAfterAnUnclosedOne() -> TestResult {
    assert_eq!(r"#^(?=[^\.])\[\[:alpha:]$#", to_regex("[[:alpha:]"));
    assert_eq!(1, preg_match(&to_regex("[[:alpha:]"), "[[:alpha:]")?);
    Ok(())
}

#[test]
fn testGlobToRegexUnclosedCurlies() -> TestResult {
    assert_eq!(r"#^(?=[^\.])a\{b$#", to_regex("a{b"));
    assert_eq!(r"#^(?=[^\.])\{a,b$#", to_regex("{a,b"));
    assert_eq!(r"#^(?=[^\.])\{a,(b|c)$#", to_regex("{a,{b,c}"));
    assert_eq!(r"#^(?=[^\.])\{a(b|c)$#", to_regex("{a{b,c}"));
    assert_eq!(r"#^(?=[^\.])\{(a)$#", to_regex("{{a}"));
    assert_eq!(r"#^(?=[^\.])\{[^/]*$#", to_regex("{*"));
    assert_eq!(1, preg_match(&to_regex("a{b"), "a{b")?);
    assert_eq!(1, preg_match(&to_regex("{a,{b,c}"), "{a,c")?);
    assert_eq!(r"#^(?=[^\.])(a|b)}$#", to_regex("{a,b}}"));
    assert_eq!(r"#^(?=[^\.])a,b$#", to_regex("a,b"));
    Ok(())
}

#[test]
fn testGlobToRegexCurliesInsideBrackets() -> TestResult {
    assert_eq!(r"#^(?=[^\.])[{]$#", to_regex("[{]"));
    assert_eq!(r"#^(?=[^\.])[{}]$#", to_regex("[{}]"));
    assert_eq!(r"#^(?=[^\.])(a|[{])$#", to_regex("{a,[{]}"));
    assert_eq!(r"#^(?=[^\.])([a{]a)$#", to_regex("{[a{]a}"));
    assert_eq!(r"#^(?=[^\.])(a|[,])$#", to_regex("{a,[,]}"));
    assert_eq!(r"#^(?=[^\.])(a|\[b)$#", to_regex("{a,[b}"));
    assert_eq!(1, preg_match(&to_regex("[{}]"), "}")?);
    assert_eq!(0, preg_match(&to_regex("[{}]"), "(")?);
    assert_eq!(1, preg_match(&to_regex("[{-}]"), "|")?);
    Ok(())
}
