//! CssSelectorConverterTest.php at symfony/css-selector v8.1.6, on
//! upstream's provider rows (tests/data/php.json). Upstream reads and
//! resets the static cache by reflection; `cache_keys` and `clear_cache`
//! stand in (the cache is per thread here, so each test sees its own).
//!
//! Upstream: symfony/css-selector Tests/CssSelectorConverterTest.php
#![allow(non_snake_case)]

mod common;

use common::{provider, show};
use symfony_css_selector::CssSelectorConverter;

fn xpath(converter: &CssSelectorConverter, css: &[u8]) -> String {
    show(&converter.to_xpath(css).unwrap())
}

#[test]
fn testCssToXPath() {
    let converter = CssSelectorConverter::new(true);

    assert_eq!("descendant-or-self::*", xpath(&converter, b""));
    assert_eq!("descendant-or-self::h1", xpath(&converter, b"h1"));
    assert_eq!(
        "descendant-or-self::h1[@id = 'foo']",
        xpath(&converter, b"h1#foo")
    );
    assert_eq!(
        "descendant-or-self::h1[@class and contains(concat(' ', normalize-space(@class), ' '), ' foo ')]",
        xpath(&converter, b"h1.foo")
    );
    assert_eq!("descendant-or-self::foo:h1", xpath(&converter, b"foo|h1"));
    assert_eq!("descendant-or-self::h1", xpath(&converter, b"H1"));

    // Test the cache layer
    let converter = CssSelectorConverter::new(true);
    assert_eq!("descendant-or-self::h1", xpath(&converter, b"H1"));
}

#[test]
fn testCssToXPathXml() {
    let converter = CssSelectorConverter::new(false);

    assert_eq!("descendant-or-self::H1", xpath(&converter, b"H1"));

    let converter = CssSelectorConverter::new(false);
    // Test the cache layer
    assert_eq!("descendant-or-self::H1", xpath(&converter, b"H1"));
}

#[test]
fn testParseExceptions() {
    let error = CssSelectorConverter::new(true)
        .to_xpath(b"h1:")
        .unwrap_err();
    assert!(error.is_parse_exception());
    assert_eq!(
        b"Expected identifier, but <eof at 3> found.",
        error.message()
    );
}

#[test]
fn testLruCacheMovesRecentlyUsedToEnd() {
    CssSelectorConverter::set_max_cached_items(5);
    CssSelectorConverter::clear_cache(true);

    let converter = CssSelectorConverter::new(true);

    // Fill cache with 5 entries (h0-h4)
    for i in 0..5 {
        converter.to_xpath(format!("h{i}").as_bytes()).unwrap();
    }

    // Access h0 to move it to end (most recently used)
    converter.to_xpath(b"h0").unwrap();

    // Trigger eviction
    converter.to_xpath(b"h5").unwrap();

    let cache = CssSelectorConverter::cache_keys(true);

    // h0 was accessed recently (moved to end), so it survives eviction
    assert!(cache.contains(&b"descendant-or-self::\0h0".to_vec()));
    // h5 is the newest entry
    assert!(cache.contains(&b"descendant-or-self::\0h5".to_vec()));
    // h1 was the oldest untouched entry, should be evicted
    assert!(!cache.contains(&b"descendant-or-self::\0h1".to_vec()));

    CssSelectorConverter::set_max_cached_items(1024);
}

#[test]
fn testCssToXPathWithoutPrefix() {
    for (name, args) in provider("CssSelectorConverterTest", "testCssToXPathWithoutPrefix") {
        let converter = CssSelectorConverter::new(true);
        assert_eq!(
            show(&args[1].bytes()),
            show(
                &converter
                    .to_xpath_with_prefix(&args[0].bytes(), b"")
                    .unwrap()
            ),
            "->parse() parses an input string and returns a node (row {name})"
        );
    }
}
