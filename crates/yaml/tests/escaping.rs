//! Symfony Yaml's Escaper and Unescaper on PHP 8.4.27, recorded by
//! oracle/recorders/yaml_escaping.php (`scripts/record.sh yaml`).
//!
//! Upstream: none

mod common;

use common::{TestResult, base64, load, rows};
use symfony_yaml::{escaper, unescaper};

#[test]
fn escaper_matches_symfony() -> TestResult {
    let json = load(include_str!("data/escaping.json"))?;
    let cases = rows(&json, "escaping")?;
    for row in cases {
        let input = base64(&row[0])?;
        let shown = String::from_utf8_lossy(&input).into_owned();
        assert_eq!(
            Some(escaper::requires_double_quoting(&input)),
            row[1].as_bool(),
            "double? {shown:?}"
        );
        assert_eq!(
            escaper::escape_with_double_quotes(&input),
            base64(&row[2])?,
            "double {shown:?}"
        );
        assert_eq!(
            Some(escaper::requires_single_quoting(&input)),
            row[3].as_bool(),
            "single? {shown:?}"
        );
        assert_eq!(
            escaper::escape_with_single_quotes(&input),
            base64(&row[4])?,
            "single {shown:?}"
        );
    }
    assert!(cases.len() > 300);
    Ok(())
}

#[test]
fn unescaper_matches_symfony() -> TestResult {
    let json = load(include_str!("data/escaping.json"))?;
    let cases = rows(&json, "unescaping")?;
    for row in cases {
        let input = base64(&row[0])?;
        let text = String::from_utf8(input.clone())?;
        let expected = row[1].as_str().ok_or("no result")?;
        match unescaper::unescape_double_quoted_string(&text) {
            Ok(bytes) => assert_eq!(bytes, base64(&row[1])?, "double {text:?}"),
            Err(error) => assert_eq!(
                format!("!{}", error.get_message()),
                expected,
                "double {text:?}"
            ),
        }
        assert_eq!(
            unescaper::unescape_single_quoted_string(&input),
            base64(&row[2])?,
            "single {text:?}"
        );
    }
    assert!(cases.len() > 300);
    Ok(())
}
