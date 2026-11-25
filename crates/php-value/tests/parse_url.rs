//! `parse_url()` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/php_value_parse_url.php` (`scripts/record.sh
//! php-value`): 119 hand-picked URLs, 6,000 generated strings over the
//! characters it treats specially, and 6,000 generated
//! scheme/authority/path/query/fragment combinations.
//!
//! Upstream: none

use php_value::{
    Value,
    url::parse_url,
    vectors::{self, Error, base64},
};

#[test]
fn recorded_parse_url() -> Result<(), Error> {
    let data: serde_json::Value = serde_json::from_str(include_str!("data/parse_url.json"))?;
    let cases = data.as_array().ok_or("cases")?;
    assert_eq!(12119, cases.len());
    for case in cases {
        let url = base64(case[0].as_str().ok_or("url")?)?;
        let expected = vectors::decode(&case[1])?;
        let actual = parse_url(&url).map_or(Value::Bool(false), |u| Value::Array(u.to_array()));
        assert_eq!(expected, actual, "{:?}", String::from_utf8_lossy(&url));
    }
    Ok(())
}
