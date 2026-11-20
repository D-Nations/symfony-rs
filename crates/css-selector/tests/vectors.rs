//! Cases upstream's tests leave open, recorded from PHP 8.4.27 with
//! symfony/css-selector v8.1.6 (`scripts/record.sh css-selector`):
//! `CssSelectorConverter::toXPath()` with and without the HTML extension
//! and with an empty prefix, or the exception each throws. They pin PHP's
//! truthiness (`"0"`), byte-wise patterns, unicode escapes, series
//! arithmetic, messages and token positions.
//!
//! Upstream: none

mod common;

use common::{assert_outcome, data, decode, show};
use symfony_css_selector::CssSelectorConverter;

#[test]
fn recorded_cases() {
    let cases = data()["cases"].as_array().expect("cases");
    assert!(cases.len() > 200);
    for case in cases {
        let css = decode(&case[0]).bytes();
        let context = show(&css);
        let html = CssSelectorConverter::new(true);
        let xml = CssSelectorConverter::new(false);
        assert_outcome(
            &decode(&case[1]),
            html.to_xpath(&css),
            &format!("html {context}"),
        );
        assert_outcome(
            &decode(&case[2]),
            xml.to_xpath(&css),
            &format!("xml {context}"),
        );
        assert_outcome(
            &decode(&case[3]),
            html.to_xpath_with_prefix(&css, b""),
            &format!("no prefix {context}"),
        );
    }
}
