//! InlineTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//! Names and assertions are PHPUnit's. Data-provider tests read the
//! provider rows recorded from upstream (tests/data/inline_test.json,
//! `scripts/record.sh yaml`), so they run on upstream's own data.
//! `expectExceptionMessage` checks that the message contains the text;
//! `expectExceptionMessageMatches` uses the upstream pattern.
//!
//! Not here: testFlowAndBlockProduceSameOutputForAmpersandPrefixedItems
//! (needs `Yaml::parse()`, ported with Parser.php) and
//! testDumpNumericValueWithLocale (`n/a`: Rust has no process locale;
//! `dump()` never depends on one).
//!
//! Upstream: symfony/yaml Tests/InlineTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::{Arc, LazyLock};

use php_value::{
    Key, PhpArray, Value,
    datetime::{DateTimeImmutable, Fields, TimeZone},
    object::EnumCase,
    php_array,
    serialize::serialize,
    vectors::decode,
};
use serde_json::Value as Json;
use symfony_yaml::{
    Environment, Flags, Inline, ParseException, References, TaggedValue, inline::is_hash,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FOO_UNIT_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum";
const FOO_BACKED_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooBackedEnum";

/// The environment of the upstream tests: PHP's constants, the Yaml
/// class constants and the two fixture enums.
fn environment() -> Arc<Environment> {
    static ENVIRONMENT: LazyLock<Arc<Environment>> = LazyLock::new(|| {
        let mut environment = Environment::new();
        environment.constants.define_enum(
            FOO_UNIT_ENUM,
            vec![EnumCase::new(FOO_UNIT_ENUM, "BAR", None)],
        );
        environment.constants.define_enum(
            FOO_BACKED_ENUM,
            vec![EnumCase::new(FOO_BACKED_ENUM, "BAR", Some("bar".into()))],
        );
        Arc::new(environment)
    });
    ENVIRONMENT.clone()
}

/// `setUp()`: `Inline::initialize(0, 0)`.
fn inline() -> Inline {
    let mut inline = Inline::new(environment());
    inline.initialize(Flags::NONE, Some(0), None);
    inline
}

fn parse_with(
    yaml: &[u8],
    flags: Flags,
    references: &mut References,
) -> Result<Value, ParseException> {
    inline().parse(yaml, flags, references, None)
}

fn parse(yaml: &str, flags: Flags) -> Result<Value, ParseException> {
    parse_with(yaml.as_bytes(), flags, &mut References::new())
}

fn dump(value: &Value, flags: Flags) -> Result<String, Box<dyn std::error::Error>> {
    Ok(inline()
        .dump(value, flags, false)?
        .to_string_lossy()
        .into_owned())
}

fn message(error: &ParseException) -> String {
    error.get_message().to_string()
}

#[track_caller]
fn expect_message(result: Result<Value, ParseException>, contains: &str) {
    match result {
        Err(error) => assert!(
            message(&error).contains(contains),
            "{:?} does not contain {contains:?}",
            message(&error)
        ),
        Ok(value) => panic!("expected a ParseException containing {contains:?}, got {value:?}"),
    }
}

#[track_caller]
fn expect_matches(result: Result<Value, ParseException>, pattern: &str) {
    let regex = regex::Regex::new(pattern).unwrap_or_else(|e| panic!("pattern {pattern}: {e}"));
    match result {
        Err(error) => assert!(
            regex.is_match(&message(&error)),
            "{:?} does not match {pattern}",
            message(&error)
        ),
        Ok(value) => panic!("expected a ParseException matching {pattern}, got {value:?}"),
    }
}

#[track_caller]
fn expect_exception(result: Result<Value, ParseException>) {
    assert!(result.is_err(), "expected a ParseException, got {result:?}");
}

/// A PCRE pattern from the providers (`/…/` or `#…#`) as a Rust regex.
fn pcre(pattern: &str) -> String {
    let delimiter = pattern.chars().next().unwrap_or('/');
    let end = pattern.rfind(delimiter).unwrap_or(pattern.len());
    pattern[1..end].to_owned()
}

/// Recorded TaggedValue objects become real ones, so `dump()` sees them.
fn realize(value: Value) -> Value {
    match value {
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|(k, v)| (k.clone(), realize(v.clone())))
                .collect(),
        ),
        Value::Object(object) if object.class() == TaggedValue::CLASS => {
            let properties = object.properties();
            let tag = properties
                .get(&Key::from("tag"))
                .and_then(Value::as_string)
                .map(|s| s.to_string())
                .unwrap_or_default();
            let inner = properties
                .get(&Key::from("value"))
                .cloned()
                .unwrap_or_default();
            Value::Object(Arc::new(TaggedValue::new(tag, realize(inner))))
        }
        other => other,
    }
}

/// The recorded rows of the provider of `test`: (name, arguments).
/// A provider row: its name and arguments.
type Row = (String, Vec<Value>);

fn provider(test: &str) -> Result<Vec<Row>, Box<dyn std::error::Error>> {
    static JSON: LazyLock<Json> = LazyLock::new(|| {
        serde_json::from_str(include_str!("data/inline_test.json")).unwrap_or(Json::Null)
    });
    let rows = JSON[test]["rows"]
        .as_array()
        .ok_or_else(|| format!("no provider rows for {test}"))?;
    let mut out = Vec::new();
    for row in rows {
        let name = row[0].as_str().unwrap_or_default().to_owned();
        let mut arguments = Vec::new();
        for argument in row[1].as_array().ok_or("no arguments")? {
            arguments.push(realize(decode(argument).map_err(|e| e.to_string())?));
        }
        out.push((name, arguments));
    }
    assert!(!out.is_empty(), "{test} has no rows");
    Ok(out)
}

fn text(value: &Value) -> String {
    value.as_string().map(|s| s.to_string()).unwrap_or_default()
}

fn int(value: &Value) -> i64 {
    match value {
        Value::Int(i) => *i,
        _ => 0,
    }
}

fn flags(value: Option<&Value>) -> Flags {
    Flags(value.map_or(0, int) as u32)
}

fn enum_case(class: &str, name: &str, value: Option<&str>) -> Value {
    Value::Object(EnumCase::new(class, name, value.map(Value::from)).object())
}

#[test]
fn testParse() -> TestResult {
    for (name, args) in provider("testParse")? {
        let yaml = text(&args[0]);
        let actual = parse(&yaml, flags(args.get(2)))?;
        assert_eq!(
            args[1], actual,
            "::parse() converts an inline YAML to a PHP structure ({yaml}) [{name}]"
        );
    }
    Ok(())
}

#[test]
fn testParseNan() -> TestResult {
    for (_, args) in provider("testParseNan")? {
        let actual = parse(&text(&args[0]), Flags::NONE)?;
        assert!(
            matches!(actual, Value::Float(f) if f.is_nan()),
            "{actual:?}"
        );
    }
    Ok(())
}

#[test]
fn testDumpNan() -> TestResult {
    assert_eq!(".NaN", dump(&Value::Float(f64::NAN), Flags::NONE)?);
    let parsed = parse(&dump(&Value::Float(f64::NAN), Flags::NONE)?, Flags::NONE)?;
    assert!(matches!(parsed, Value::Float(f) if f.is_nan()));
    Ok(())
}

#[test]
fn testParseWithMapObjects() -> TestResult {
    for (name, args) in provider("testParseWithMapObjects")? {
        let yaml = text(&args[0]);
        let flags = args
            .get(2)
            .map_or(Flags::PARSE_OBJECT_FOR_MAP, |f| flags(Some(f)));
        let actual = parse(&yaml, flags)?;
        assert_eq!(serialize(&args[1]), serialize(&actual), "{yaml} [{name}]");
    }
    Ok(())
}

#[test]
fn testParsePhpConstants() -> TestResult {
    for (name, args) in provider("testParsePhpConstants")? {
        let actual = parse(&text(&args[0]), Flags::PARSE_CONSTANT)?;
        assert_eq!(args[1], actual, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParsePhpConstantThrowsExceptionWhenUndefined() {
    expect_message(
        parse("!php/const WRONG_CONSTANT", Flags::PARSE_CONSTANT),
        "The constant \"WRONG_CONSTANT\" is not defined",
    );
}

#[test]
fn testParsePhpEnumThrowsExceptionWhenUndefined() {
    expect_message(
        parse("!php/enum SomeEnum", Flags::PARSE_CONSTANT),
        "The enum \"SomeEnum\" is not defined",
    );
}

#[test]
fn testParsePhpEnumThrowsExceptionWhenNameUndefined() {
    expect_message(
        parse(
            r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::Foo",
            Flags::PARSE_CONSTANT,
        ),
        r#"The string "Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::Foo" is not the name of a valid enum"#,
    );
}

#[test]
fn testParsePhpEnumThrowsExceptionWhenNotAnEnum() {
    expect_message(
        parse("!php/enum PHP_INT_MAX", Flags::PARSE_CONSTANT),
        "The enum \"PHP_INT_MAX\" is not defined",
    );
}

#[test]
fn testParsePhpEnumThrowsExceptionWhenNotBacked() {
    expect_message(
        parse(
            r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::BAR->value",
            Flags::PARSE_CONSTANT,
        ),
        r#"The enum "Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::BAR" defines no value next to its name"#,
    );
}

#[test]
fn testParsePhpConstantThrowsExceptionOnInvalidType() -> TestResult {
    assert_eq!(Value::Null, parse("!php/const PHP_INT_MAX", Flags::NONE)?);
    expect_matches(
        parse(
            "!php/const PHP_INT_MAX",
            Flags::PARSE_EXCEPTION_ON_INVALID_TYPE,
        ),
        r#"The string "!php/const PHP_INT_MAX" could not be parsed as a constant.*"#,
    );
    Ok(())
}

#[test]
fn testParsePhpEnumThrowsExceptionOnInvalidType() -> TestResult {
    assert_eq!(Value::Null, parse("!php/enum SomeEnum::Foo", Flags::NONE)?);
    expect_matches(
        parse(
            "!php/enum SomeEnum::Foo",
            Flags::PARSE_EXCEPTION_ON_INVALID_TYPE,
        ),
        r#"The string "!php/enum SomeEnum::Foo" could not be parsed as an enum.*"#,
    );
    Ok(())
}

#[test]
fn testParsePhpObjectThrowsExceptionOnNonStringScalar() {
    expect_message(
        parse(
            r"!php/object !php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum",
            Flags::PARSE_OBJECT | Flags::PARSE_CONSTANT,
        ),
        "The \"!php/object\" tag only supports a string value, got \"array\"",
    );
}

#[test]
fn testDump() -> TestResult {
    for (name, args) in provider("testDump")? {
        let yaml = text(&args[0]);
        let dumped = dump(&args[1], Flags::NONE)?;
        assert_eq!(
            yaml, dumped,
            "::dump() converts a PHP structure to an inline YAML ({yaml}) [{name}]"
        );
        let parsed = parse(&dumped, flags(args.get(2)))?;
        assert_eq!(args[1], parsed, "check consistency [{name}]");
    }
    Ok(())
}

#[test]
fn testHashStringsResemblingExponentialNumericsShouldNotBeChangedToINF() -> TestResult {
    let value = Value::from("686e444");
    assert_eq!(value, parse(&dump(&value, Flags::NONE)?, Flags::NONE)?);
    Ok(())
}

#[test]
fn testParseScalarWithNonEscapedBlackslashShouldThrowException() {
    expect_message(
        parse(r#""Foo\Var""#, Flags::NONE),
        r#"Found unknown escape character "\V"."#,
    );
}

#[test]
fn testParseScalarWithNonEscapedBlackslashAtTheEndShouldThrowException() {
    expect_exception(parse(r#""Foo\""#, Flags::NONE));
}

#[test]
fn testParseScalarWithIncorrectlyQuotedStringShouldThrowException() {
    expect_exception(parse("'don't do somthin' like that'", Flags::NONE));
}

#[test]
fn testParseScalarWithIncorrectlyDoubleQuotedStringShouldThrowException() {
    expect_exception(parse(r#""don"t do somthin" like that""#, Flags::NONE));
}

#[test]
fn testParseInvalidMappingKeyShouldThrowException() {
    expect_exception(parse(r#"{ "foo " bar": "bar" }"#, Flags::NONE));
}

#[test]
fn testParseMappingKeyWithColonNotFollowedBySpace() {
    expect_message(
        parse(r#"{foo:""}"#, Flags::NONE),
        r#"Colons must be followed by a space or an indication character (i.e. " ", ",", "[", "]", "{", "}")"#,
    );
}

#[test]
fn testParseInvalidMappingShouldThrowException() {
    expect_exception(parse("[foo] bar", Flags::NONE));
}

#[test]
fn testParseInvalidSequenceShouldThrowException() {
    expect_exception(parse("{ foo: bar } bar", Flags::NONE));
}

#[test]
fn testParseEmbeddedMappingWithDuplicateKeysShouldThrowException() {
    expect_matches(
        parse("[ foo: { bar: 1, bar: 2 } ]", Flags::NONE),
        r#"Duplicate key "bar" detected"#,
    );
}

#[test]
fn testParseInvalidTaggedSequenceShouldThrowException() {
    expect_exception(parse("!foo { bar: baz } qux", Flags::PARSE_CUSTOM_TAGS));
}

#[test]
fn testParseScalarWithCorrectlyQuotedStringShouldReturnString() -> TestResult {
    let value = "'don''t do somthin'' like that'";
    let expect = Value::from("don't do somthin' like that");
    assert_eq!(expect, inline().parse_scalar_value(value.as_bytes())?);
    Ok(())
}

#[test]
fn testParseReferences() -> TestResult {
    for (name, args) in provider("testParseReferences")? {
        let mut references = References::new();
        references.insert(Key::from("var"), Value::from("var-value"));
        let actual = parse_with(text(&args[0]).as_bytes(), Flags::NONE, &mut references)?;
        assert_eq!(args[1], actual, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseMapReferenceInSequence() -> TestResult {
    let foo = php_array!["a" => "Steve", "b" => "Clark", "c" => "Brian"];
    let mut references = References::new();
    references.insert(Key::from("foo"), Value::Array(foo.clone()));
    let expected = Value::Array(PhpArray::list([Value::Array(foo)]));
    assert_eq!(
        expected,
        parse_with(b"[*foo]", Flags::NONE, &mut references)?
    );
    Ok(())
}

#[test]
fn testParseUnquotedAsterisk() {
    expect_message(
        parse("{ foo: * }", Flags::NONE),
        "A reference must contain at least one character at line 1.",
    );
}

#[test]
fn testParseUnquotedAsteriskFollowedByAComment() {
    expect_message(
        parse("{ foo: * #foo }", Flags::NONE),
        "A reference must contain at least one character at line 1.",
    );
}

#[test]
fn testParseUnquotedScalarStartingWithReservedIndicator() -> TestResult {
    for (_, args) in provider("testParseUnquotedScalarStartingWithReservedIndicator")? {
        let indicator = text(&args[0]);
        expect_message(
            parse(&format!("{{ foo: {indicator}foo }}"), Flags::NONE),
            &format!(
                "cannot start a plain scalar; you need to quote the scalar at line 1 (near \"{indicator}foo\")."
            ),
        );
    }
    Ok(())
}

#[test]
fn testParseUnquotedScalarStartingWithScalarIndicator() -> TestResult {
    for (_, args) in provider("testParseUnquotedScalarStartingWithScalarIndicator")? {
        let indicator = text(&args[0]);
        expect_message(
            parse(&format!("{{ foo: {indicator}foo }}"), Flags::NONE),
            &format!(
                "cannot start a plain scalar; you need to quote the scalar at line 1 (near \"{indicator}foo\")."
            ),
        );
    }
    Ok(())
}

#[test]
fn testIsHash() -> TestResult {
    for (name, args) in provider("testIsHash")? {
        assert_eq!(args[1], Value::Bool(is_hash(&args[0])), "[{name}]");
    }
    Ok(())
}

/// A provider row of getTimestampTests: the instant from its fields.
fn timestamp_expected(args: &[Value], zone: TimeZone) -> DateTimeImmutable {
    let fields = Fields {
        year: int(&args[1]),
        month: int(&args[2]),
        day: int(&args[3]),
        hour: int(&args[4]),
        minute: int(&args[5]),
        second: int(&args[6]),
        microsecond: int(&args[7]) as u32,
    };
    DateTimeImmutable::from_fields(fields, zone)
}

#[test]
fn testParseTimestampAsUnixTimestampByDefault() -> TestResult {
    for (name, args) in provider("testParseTimestampAsUnixTimestampByDefault")? {
        let yaml = text(&args[0]);
        // The provider's fields are UTC, as DateTimeImmutable($yaml) gives them.
        let local = timestamp_expected(&args, TimeZone::Utc);
        let microsecond = int(&args[7]);
        let expected = if microsecond != 0 {
            Value::Float(format!("{}.{microsecond}", local.timestamp()).parse()?)
        } else {
            Value::Int(local.timestamp())
        };
        assert_eq!(expected, parse(&yaml, Flags::NONE)?, "{yaml} [{name}]");
    }
    Ok(())
}

#[test]
fn testParseTimestampAsDateTimeObject() -> TestResult {
    for (name, args) in provider("testParseTimestampAsDateTimeObject")? {
        let yaml = text(&args[0]);
        let expected = timestamp_expected(&args, TimeZone::Utc);
        let date = parse(&yaml, Flags::PARSE_DATETIME)?;
        let date = date
            .as_object()
            .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
            .ok_or("not a date")?;
        assert_eq!(
            (expected.timestamp(), expected.microsecond()),
            (date.timestamp(), date.microsecond()),
            "[{name}]"
        );
        assert_eq!(text(&args[8]), date.format("O"), "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseTimestampBeforeUnixEpochAsUnixTimestamp() -> TestResult {
    assert_eq!(
        Value::Float(-1.5),
        parse("1969-12-31T23:59:58.5Z", Flags::NONE)?
    );
    assert_eq!(Value::Int(-2), parse("1969-12-31T23:59:58Z", Flags::NONE)?);
    Ok(())
}

#[test]
fn testParseNestedTimestampListAsDateTimeObject() -> TestResult {
    for (name, args) in provider("testParseNestedTimestampListAsDateTimeObject")? {
        let yaml = text(&args[0]);
        let expected = timestamp_expected(&args, TimeZone::Utc);
        let parsed = parse(&format!("{{nested: [{yaml}]}}"), Flags::PARSE_DATETIME)?;
        let nested = parsed
            .as_array()
            .and_then(|a| a.get(&Key::from("nested")))
            .and_then(Value::as_array)
            .and_then(|a| a.get(&Key::Int(0)))
            .and_then(Value::as_object)
            .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
            .ok_or("not ['nested' => [date]]")?;
        assert_eq!(parsed.as_array().map(PhpArray::len), Some(1));
        assert_eq!(
            (expected.timestamp(), expected.microsecond()),
            (nested.timestamp(), nested.microsecond()),
            "[{name}]"
        );
    }
    Ok(())
}

#[test]
fn testParseInvalidDate() {
    expect_matches(
        parse("2024-50-50", Flags::PARSE_DATETIME),
        r#"^The date "2024-50-50" could not be parsed as it is an invalid date.*"#,
    );
}

#[test]
fn testDumpDateTime() -> TestResult {
    for (name, args) in provider("testDumpDateTime")? {
        assert_eq!(text(&args[1]), dump(&args[0], Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testDumpNumericKeyAsString() -> TestResult {
    for (name, args) in provider("testDumpNumericKeyAsString")? {
        assert_eq!(
            text(&args[2]),
            dump(&args[0], flags(Some(&args[1])))?,
            "[{name}]"
        );
    }
    Ok(())
}

#[test]
fn testDumpUnitEnum() -> TestResult {
    assert_eq!(
        r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::BAR",
        dump(&enum_case(FOO_UNIT_ENUM, "BAR", None), Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseUnitEnumCases() -> TestResult {
    let expected = Value::Array(PhpArray::list([enum_case(FOO_UNIT_ENUM, "BAR", None)]));
    assert_eq!(
        expected,
        parse(
            r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum",
            Flags::PARSE_CONSTANT
        )?
    );
    Ok(())
}

#[test]
fn testParseUnitEnum() -> TestResult {
    assert_eq!(
        enum_case(FOO_UNIT_ENUM, "BAR", None),
        parse(
            r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum::BAR",
            Flags::PARSE_CONSTANT
        )?
    );
    Ok(())
}

#[test]
fn testParseBackedEnumValue() -> TestResult {
    assert_eq!(
        Value::from("bar"),
        parse(
            r"!php/enum Symfony\Component\Yaml\Tests\Fixtures\FooBackedEnum::BAR->value",
            Flags::PARSE_CONSTANT
        )?
    );
    Ok(())
}

#[test]
fn testParseBinaryData() -> TestResult {
    for (name, args) in provider("testParseBinaryData")? {
        assert_eq!(
            Value::from("Hello world"),
            parse(&text(&args[0]), Flags::NONE)?,
            "[{name}]"
        );
    }
    Ok(())
}

#[test]
fn testParseInvalidBinaryData() -> TestResult {
    for (_, args) in provider("testParseInvalidBinaryData")? {
        expect_matches(parse(&text(&args[0]), Flags::NONE), &pcre(&text(&args[1])));
    }
    Ok(())
}

#[test]
fn testNotSupportedMissingValue() {
    expect_message(
        parse("{this, is not, supported}", Flags::NONE),
        r#"Malformed inline YAML string: "{this, is not, supported}" at line 1."#,
    );
}

#[test]
fn testUnterminatedSequenceEndingWithTag() {
    expect_message(
        parse("[!foo", Flags::PARSE_CUSTOM_TAGS),
        r#"Malformed inline YAML string: "[!foo""#,
    );
}

#[test]
fn testUnterminatedMappingEndingWithTag() {
    expect_message(
        parse("{foo: !bar", Flags::PARSE_CUSTOM_TAGS),
        r#"Malformed inline YAML string: "{foo: !bar""#,
    );
}

#[test]
fn testVeryLongQuotedStrings() -> TestResult {
    let long_string_with_quotes = "x\r\n\\\"x\"x".repeat(1000);
    let yaml_string = dump(
        &Value::Array(php_array!["longStringWithQuotes" => long_string_with_quotes.as_str()]),
        Flags::NONE,
    )?;
    let array_from_yaml = parse(&yaml_string, Flags::NONE)?;
    let parsed = array_from_yaml
        .as_array()
        .and_then(|a| a.get(&Key::from("longStringWithQuotes")))
        .cloned();
    assert_eq!(Some(Value::from(long_string_with_quotes.as_str())), parsed);
    Ok(())
}

#[test]
fn testMappingKeysCannotBeOmitted() {
    expect_message(parse("{: foo}", Flags::NONE), "Missing mapping key");
}

#[test]
fn testParseMissingMappingValueAsNull() -> TestResult {
    for (name, args) in provider("testParseMissingMappingValueAsNull")? {
        assert_eq!(args[1], parse(&text(&args[0]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testTheEmptyStringIsAValidMappingKey() -> TestResult {
    assert_eq!(
        Value::Array(php_array!["" => "foo"]),
        parse(r#"{ "": foo }"#, Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testImplicitStringCastingOfMappingKeysThrowsException() -> TestResult {
    for (_, args) in provider("testImplicitStringCastingOfMappingKeysThrowsException")? {
        expect_message(
            parse(&text(&args[0]), Flags::NONE),
            "Implicit casting of incompatible mapping keys to strings is not supported. Quote your evaluable mapping keys instead",
        );
    }
    Ok(())
}

#[test]
fn testBuiltInTagWithoutValueAsMappingKey() {
    expect_message(
        parse("{!!str : foo}", Flags::NONE),
        "Implicit casting of incompatible mapping keys to strings is not supported. Quote your evaluable mapping keys instead",
    );
}

fn tagged(value: &Value) -> Option<&TaggedValue> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<TaggedValue>())
}

fn at(value: &Value, key: impl Into<Key>) -> Option<&Value> {
    value.as_array().and_then(|a| a.get(&key.into()))
}

#[test]
fn testTagWithoutValueInSequence() -> TestResult {
    let value = parse("[!foo]", Flags::PARSE_CUSTOM_TAGS)?;
    let tag = at(&value, 0).and_then(tagged).ok_or("not a TaggedValue")?;
    assert_eq!("foo", tag.get_tag());
    assert_eq!(&Value::from(""), tag.get_value());
    Ok(())
}

#[test]
fn testTagWithEmptyValueInSequence() -> TestResult {
    let value = parse(r#"[!foo ""]"#, Flags::PARSE_CUSTOM_TAGS)?;
    let tag = at(&value, 0).and_then(tagged).ok_or("not a TaggedValue")?;
    assert_eq!("foo", tag.get_tag());
    assert_eq!(&Value::from(""), tag.get_value());
    Ok(())
}

#[test]
fn testTagWithoutValueInMapping() -> TestResult {
    let value = parse("{foo: !bar}", Flags::PARSE_CUSTOM_TAGS)?;
    let tag = at(&value, "foo")
        .and_then(tagged)
        .ok_or("not a TaggedValue")?;
    assert_eq!("bar", tag.get_tag());
    assert_eq!(&Value::from(""), tag.get_value());
    Ok(())
}

#[test]
fn testTagWithEmptyValueInMapping() -> TestResult {
    let value = parse(r#"{foo: !bar ""}"#, Flags::PARSE_CUSTOM_TAGS)?;
    let tag = at(&value, "foo")
        .and_then(tagged)
        .ok_or("not a TaggedValue")?;
    assert_eq!("bar", tag.get_tag());
    assert_eq!(&Value::from(""), tag.get_value());
    Ok(())
}

#[test]
fn testTagWithQuotedInteger() -> TestResult {
    let value = parse(r#"!number "5""#, Flags::PARSE_CUSTOM_TAGS)?;
    let tag = tagged(&value).ok_or("not a TaggedValue")?;
    assert_eq!("number", tag.get_tag());
    assert_eq!(&Value::from("5"), tag.get_value());
    Ok(())
}

#[test]
fn testTagWithUnquotedInteger() -> TestResult {
    let value = parse("!number 5", Flags::PARSE_CUSTOM_TAGS)?;
    let tag = tagged(&value).ok_or("not a TaggedValue")?;
    assert_eq!("number", tag.get_tag());
    assert_eq!(&Value::Int(5), tag.get_value());
    Ok(())
}

#[test]
fn testUnfinishedInlineMap() {
    expect_message(
        parse("{abc: 'def'", Flags::NONE),
        "Unexpected end of line, expected one of \",}\n\" at line 1 (near \"{abc: 'def'\").",
    );
}

#[test]
fn testParseOctalNumbers() -> TestResult {
    for (name, args) in provider("testParseOctalNumbers")? {
        assert_eq!(args[0], parse(&text(&args[1]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseOctalNumbersYaml11Notation() -> TestResult {
    for (name, args) in provider("testParseOctalNumbersYaml11Notation")? {
        assert_eq!(args[0], parse(&text(&args[1]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testPhpObjectWithEmptyValue() -> TestResult {
    for (_, args) in provider("testPhpObjectWithEmptyValue")? {
        expect_message(
            parse(&text(&args[0]), Flags::PARSE_OBJECT),
            r#"Missing value for tag "!php/object" at line 1 (near "!php/object")."#,
        );
    }
    Ok(())
}

#[test]
fn testPhpConstTagWithEmptyValue() -> TestResult {
    for (_, args) in provider("testPhpConstTagWithEmptyValue")? {
        expect_message(
            parse(&text(&args[0]), Flags::PARSE_CONSTANT),
            r#"Missing value for tag "!php/const" at line 1 (near "!php/const")."#,
        );
    }
    Ok(())
}

#[test]
fn testPhpEnumTagWithEmptyValue() -> TestResult {
    for (_, args) in provider("testPhpEnumTagWithEmptyValue")? {
        let value = text(&args[0]).replace("!php/const", "!php/enum");
        expect_message(
            parse(&value, Flags::PARSE_CONSTANT),
            r#"Missing value for tag "!php/enum" at line 1 (near "!php/enum")."#,
        );
    }
    Ok(())
}

#[test]
fn testParseCommentNotPrefixedBySpaces() -> TestResult {
    assert_eq!(
        Value::from("foo"),
        parse(r##""foo"#comment"##, Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseUnquotedStringContainingHashTagNotPrefixedBySpace() -> TestResult {
    assert_eq!(
        Value::from("foo#nocomment"),
        parse("foo#nocomment", Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testUnquotedExclamationMarkThrows() -> TestResult {
    for (_, args) in provider("testUnquotedExclamationMarkThrows")? {
        expect_matches(
            parse(&text(&args[0]), Flags::NONE),
            r#"^Using the unquoted scalar value "!" is not supported\. You must quote it at line 1 \(near ""#,
        );
    }
    Ok(())
}

#[test]
fn testQuotedExclamationMark() -> TestResult {
    for (name, args) in provider("testQuotedExclamationMark")? {
        assert_eq!(args[0], parse(&text(&args[1]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseIdeographicSpace() -> TestResult {
    for (name, args) in provider("testParseIdeographicSpace")? {
        assert_eq!(args[1], parse(&text(&args[0]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseSingleQuotedTaggedString() -> TestResult {
    assert_eq!(Value::from("foo"), parse("!!str 'foo'", Flags::NONE)?);
    Ok(())
}

#[test]
fn testParseDoubleQuotedTaggedString() -> TestResult {
    assert_eq!(Value::from("foo"), parse(r#"!!str "foo""#, Flags::NONE)?);
    Ok(())
}

#[test]
fn testParseQuotedReferenceLikeStringsInMapping() -> TestResult {
    let yaml = r#"{foo: '&foo', bar: "&bar", baz: !!str '&baz'}"#;
    assert_eq!(
        Value::Array(php_array!["foo" => "&foo", "bar" => "&bar", "baz" => "&baz"]),
        parse(yaml, Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseQuotedReferenceLikeStringsInSequence() -> TestResult {
    let yaml = r#"['&foo', "&bar", !!str '&baz']"#;
    assert_eq!(
        Value::Array(php_array!["&foo", "&bar", "&baz"]),
        parse(yaml, Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseSequenceWithEmptyElement() -> TestResult {
    let list = |values: Vec<Value>| Value::Array(PhpArray::list(values));
    assert_eq!(
        list(vec!["foo".into(), Value::Null, "bar".into()]),
        parse("[foo, , bar]", Flags::NONE)?
    );
    assert_eq!(
        list(vec![Value::Null, "foo".into(), "bar".into()]),
        parse("[, foo, bar]", Flags::NONE)?
    );
    assert_eq!(
        list(vec!["foo".into(), "bar".into()]),
        parse("[foo, bar, ]", Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseAnchoredInlineValues() -> TestResult {
    for (name, args) in provider("testParseAnchoredInlineValues")? {
        assert_eq!(args[1], parse(&text(&args[0]), Flags::NONE)?, "[{name}]");
    }
    Ok(())
}

#[test]
fn testParseAnchoredMergeKey() -> TestResult {
    assert_eq!(
        Value::Array(php_array!["k" => "v", "bar" => 2]),
        parse("{ <<: &a { k: v }, bar: 2 }", Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseQuotedMergeKeyAsRegularKey() -> TestResult {
    assert_eq!(
        Value::Array(php_array!["<<" => "foo", "bar" => 2]),
        parse("{ '<<': foo, bar: 2 }", Flags::NONE)?
    );
    assert_eq!(
        Value::Array(php_array!["<<" => Value::Array(php_array!["k" => "v"])]),
        parse(r#"{ "<<": { k: v } }"#, Flags::NONE)?
    );
    assert_eq!(
        Value::Array(php_array!["<<" => Value::Array(php_array!["foo"])]),
        parse("{ '<<': [foo] }", Flags::NONE)?
    );
    Ok(())
}

#[test]
fn testParseMergeKeyWithScalarValueThrowsParseException() {
    expect_message(
        parse("{ <<: foo }", Flags::NONE),
        "YAML merge keys used with a scalar value instead of an array",
    );
}

#[test]
fn testParseMergeKeyWithSequenceOfScalarsThrowsParseException() {
    expect_message(
        parse("{ <<: [foo] }", Flags::NONE),
        "Merge items must be arrays",
    );
}

#[test]
fn testParseKeyStartingWithNulByteWithMapObjectsThrowsParseException() {
    expect_message(
        parse("{ foo: 1, \"\\0\": 1 }", Flags::PARSE_OBJECT_FOR_MAP),
        "Mapping keys starting with a NUL byte are not supported with the \"Yaml::PARSE_OBJECT_FOR_MAP\" flag",
    );
}
