//! UnicodeStringTest's tests of the methods ported here (`testLength`,
//! `testSlice`, `testChunk`, `testWidth`, `testWordwrap`, inherited from
//! AbstractAsciiTestCase), on upstream's own provider rows as the oracle
//! returns them (tests/data/unicode_string_test.json, `scripts/record.sh
//! string`). Names and assertions are upstream's.
//!
//! Upstream: symfony/string Tests/UnicodeStringTest.php Tests/AbstractUnicodeTestCase.php Tests/AbstractAsciiTestCase.php
#![allow(non_snake_case)]

use php_value::{Value, vectors::decode};
use serde_json::Value as Json;
use symfony_string::UnicodeString;

fn provider(test: &str) -> Vec<(String, Vec<Value>)> {
    let json: Json = serde_json::from_str(include_str!("data/unicode_string_test.json"))
        .unwrap_or_else(|e| panic!("unicode_string_test.json: {e}"));
    json[test]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no provider for {test}"))
        .iter()
        .map(|row| {
            let name = row[0].as_str().unwrap_or_default().to_owned();
            let args = row[1]
                .as_array()
                .unwrap_or_else(|| panic!("{test}: arguments"))
                .iter()
                .map(|a| decode(a).unwrap_or_else(|e| panic!("{test}: {e:?}")))
                .collect();
            (name, args)
        })
        .collect()
}

/// A string, or a symfony/string object's text (recorded as its `string`
/// property).
fn text(value: &Value) -> String {
    if let Some(object) = value.as_object() {
        let property = object
            .properties()
            .get(&"string".into())
            .cloned()
            .unwrap_or_default();
        return text(&property);
    }
    let bytes = value
        .as_string()
        .unwrap_or_else(|| panic!("not a string: {value:?}"));
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| panic!("not UTF-8: {e}"))
}

fn int(value: &Value) -> i64 {
    match value {
        Value::Int(i) => *i,
        other => panic!("not an int: {other:?}"),
    }
}

/// `static::createFromString($string)`.
fn create(value: &Value) -> UnicodeString {
    UnicodeString::new(&text(value))
}

#[test]
fn testLength() {
    let rows = provider("testLength");
    assert_eq!(rows.len(), 10);
    for (name, args) in rows {
        // Upstream skips this row where grapheme_strlen('च्छे') is not 2
        // (ICU-21661); unicode-segmentation joins Indic conjuncts (GB9c).
        if UnicodeString::new("च्छे").length() != 2 && text(&args[1]) == "अनुच्छेद"
        {
            continue;
        }
        assert_eq!(create(&args[1]).length() as i64, int(&args[0]), "{name}");
    }
}

#[test]
fn testSlice() {
    let rows = provider("testSlice");
    assert_eq!(rows.len(), 25);
    for (name, args) in rows {
        let length = match args.get(3) {
            None | Some(Value::Null) => None,
            Some(v) => Some(int(v)),
        };
        assert_eq!(
            create(&args[0]),
            create(&args[1]).slice(int(&args[2]), length),
            "{name}"
        );
    }
}

#[test]
fn testChunk() {
    let rows = provider("testChunk");
    assert_eq!(rows.len(), 10);
    for (name, args) in rows {
        let expected: Vec<UnicodeString> = args[1]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: chunks"))
            .values()
            .map(create)
            .collect();
        let length = usize::try_from(int(&args[2])).unwrap_or_else(|e| panic!("{e}"));
        let actual = create(&args[0])
            .chunk(length)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(expected, actual, "{name}");
    }
}

#[test]
fn testWidth() {
    let rows = provider("testWidth");
    assert_eq!(rows.len(), 57);
    for (name, args) in rows {
        let ignore = !matches!(args.get(2), Some(Value::Bool(false)));
        assert_eq!(
            create(&args[1]).width(ignore) as i64,
            int(&args[0]),
            "{name}: {:?}",
            text(&args[1])
        );
    }
}

#[test]
fn testWordwrap() {
    let rows = provider("testWordwrap");
    assert_eq!(rows.len(), 9);
    for (name, args) in rows {
        let cut = matches!(args.get(4), Some(Value::Bool(true)));
        let width = usize::try_from(int(&args[2])).unwrap_or_else(|e| panic!("{e}"));
        let actual = create(&args[1])
            .wordwrap(width, &text(&args[3]), cut)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(create(&args[0]), actual, "{name}");
    }
}
