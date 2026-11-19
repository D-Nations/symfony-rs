//! ParserTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//!
//! Each test has its upstream name and replays every call the upstream
//! test makes to `Parser::parse()` / `parseFile()` (through
//! `$this->parser`, `new Parser(...)` or `Yaml::parse()`), for each of its
//! data-provider rows: same limits, input and flags, and the Rust parser
//! must return exactly what PHP returned, or fail with PHP's message.
//! `oracle/recorders/yaml_test_calls.php` records the calls by running the
//! upstream test methods on the oracle with a recording subclass of the
//! real Parser, and keeps a recording only when the upstream test passes
//! there, so PHP's results are the values the upstream assertions accept.
//! On top, the exception a test expects (`expectException`,
//! `expectExceptionMessage`, `expectExceptionMessageMatches`) is checked
//! against the last call, as PHPUnit does.
//!
//! Inputs over 64 KiB are not recorded: the four tests that build them
//! with `str_repeat()` build them here the same way, and check them
//! against the recorded length. testParsingNotReadableFilesThrowsException
//! is skipped upstream when run as root or on Windows, as on the oracle.
//!
//! The fixtures are upstream's Tests/Fixtures (without the PHP enums) in
//! tests/Fixtures; `{tests}` in the recordings is this tests directory.
//!
//! Upstream: symfony/yaml Tests/ParserTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::{Arc, LazyLock};

use php_value::{
    PhpArray, Value,
    datetime::DateTimeImmutable,
    object::EnumCase,
    php_array,
    vectors::{base64, decode},
};
use serde_json::Value as Json;
use symfony_yaml::{Environment, Flags, Inline, ParseException, Parser};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const B: &str = r"Symfony\Component\Yaml\Tests\B";
const FOO_UNIT_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum";
const FOO_BACKED_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooBackedEnum";

/// The environment of the upstream tests: PHP's constants, the Yaml
/// class constants, class `B` of ParserTest.php (its constants, and
/// `unserialize()` may create it) and the two fixture enums.
fn environment() -> Arc<Environment> {
    static ENVIRONMENT: LazyLock<Arc<Environment>> = LazyLock::new(|| {
        let mut environment = Environment::new();
        for (name, value) in [("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")] {
            environment
                .constants
                .define(format!("{B}::{name}"), Value::from(value));
        }
        environment
            .classes
            .allow_with_defaults(B, php_array!["b" => "foo"]);
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

fn recordings() -> &'static Json {
    static DATA: LazyLock<Json> = LazyLock::new(|| {
        // the nesting-depth tests record values nested past serde's limit
        let mut reader = serde_json::Deserializer::from_str(include_str!("data/parser_test.json"));
        reader.disable_recursion_limit();
        match reader.into_iter::<Json>().next() {
            Some(Ok(data)) => data,
            other => panic!("tests/data/parser_test.json: {other:?}"),
        }
    });
    &DATA
}

/// This tests directory, as `{tests}` stands for it in the recordings.
fn tests_dir() -> String {
    format!("{}/tests", env!("CARGO_MANIFEST_DIR")).replace('\\', "/")
}

fn localize(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    if text.contains("{tests}") {
        return text.replace("{tests}", &tests_dir()).into_bytes();
    }
    bytes.to_vec()
}

/// A recorded value with `{tests}` in its strings replaced.
fn localize_value(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(localize(&s).as_slice().into()),
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|(k, v)| (k.clone(), localize_value(v.clone())))
                .collect(),
        ),
        other => other,
    }
}

/// The same value: types, order and contents, objects by class and
/// properties, dates by their formatted value and time zone.
fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(a), Value::Float(b)) => {
            a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
        }
        (Value::Array(a), Value::Array(b)) => same_array(a, b),
        (Value::Object(a), Value::Object(b)) => {
            if a.class() != b.class() {
                return false;
            }
            if let (Some(x), Some(y)) = (
                a.downcast_ref::<DateTimeImmutable>(),
                b.downcast_ref::<DateTimeImmutable>(),
            ) {
                let format = r"Y-m-d\TH:i:s.uP e";
                return x.format(format) == y.format(format);
            }
            same_array(&a.properties(), &b.properties())
        }
        (a, b) => a.identical(b),
    }
}

fn same_array(a: &PhpArray, b: &PhpArray) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
}

fn text(json: &Json) -> &str {
    json.as_str().unwrap_or_default()
}

/// Replays the recorded calls of `test`; `build` gives the inputs that
/// were too large to record (by call index within the row).
fn replay_with(test: &str, build: &dyn Fn(usize) -> Vec<u8>) -> TestResult {
    let rows = recordings()[test]
        .as_array()
        .ok_or_else(|| format!("no recording for {test}"))?;
    for row in rows {
        let label = match row["row"].as_str() {
            Some(name) => format!("{test} [{name}]"),
            None => test.to_owned(),
        };
        if row["status"] == "skipped" {
            continue;
        }
        assert_eq!(row["status"], "pass", "{label}: not passing upstream");
        let mut last: Option<Result<Value, ParseException>> = None;
        for (index, call) in row["calls"]
            .as_array()
            .ok_or("no calls")?
            .iter()
            .enumerate()
        {
            let limits = &call["limits"];
            let mut parser = Parser::with_environment(environment()).with_limits(
                limits[0].as_i64().ok_or("limits")?,
                limits[1].as_i64().ok_or("limits")?,
            )?;
            let input = if call["input"][0] == "omitted" {
                let built = build(index);
                assert_eq!(
                    Some(built.len() as u64),
                    call["input"][1].as_u64(),
                    "{label}: the built input differs from upstream's"
                );
                built
            } else {
                localize(&base64(text(&call["input"][1]))?)
            };
            let flags = Flags(call["flags"].as_u64().ok_or("flags")? as u32);
            let result = match text(&call["method"]) {
                "parse" => parser.parse(&input, flags),
                "parseFile" => parser.parse_file(&String::from_utf8_lossy(&input), flags),
                other => return Err(format!("unknown method {other}").into()),
            };
            let what = format!(
                "{label}, call {index}: {:?}",
                String::from_utf8_lossy(&input)
            );
            let recorded = &call["result"];
            match (recorded[0].as_str(), &result) {
                (Some("omitted"), _) => {}
                (Some("throws"), Err(error)) => {
                    assert_eq!(
                        text(&recorded[1]),
                        r"Symfony\Component\Yaml\Exception\ParseException",
                        "{what}"
                    );
                    assert_eq!(
                        error.get_message().to_string_lossy(),
                        String::from_utf8_lossy(&localize(text(&recorded[2]).as_bytes())),
                        "{what}"
                    );
                }
                (Some("throws"), Ok(value)) => panic!(
                    "{what}: expected {} {:?}, got {value:?}",
                    text(&recorded[1]),
                    text(&recorded[2])
                ),
                (_, Err(error)) => panic!("{what}: expected {recorded}, got {error}"),
                (_, Ok(value)) => {
                    let expected = localize_value(decode(recorded)?);
                    assert!(
                        same(&expected, value),
                        "{what}:\nexpected {expected:?}\n     got {value:?}"
                    );
                }
            }
            last = Some(result);
        }
        if let Some(expected) = row["expected"].as_object() {
            let Some(Err(error)) = &last else {
                panic!("{label}: expected {:?}, got {last:?}", expected["class"]);
            };
            let message = error.get_message().to_string_lossy().into_owned();
            if let Some(contains) = expected["message"].as_str() {
                let contains = String::from_utf8_lossy(&localize(contains.as_bytes())).into_owned();
                assert!(
                    message.contains(&contains),
                    "{label}: {message:?} does not contain {contains:?}"
                );
            }
            if let Some(pattern) = expected["matches"].as_str() {
                let regex = regex::Regex::new(&pcre(pattern))?;
                assert!(
                    regex.is_match(&message),
                    "{label}: {message:?} does not match {pattern}"
                );
            }
        }
    }
    Ok(())
}

fn replay(test: &str) -> TestResult {
    replay_with(test, &|index| {
        panic!("{test}: call {index} has no recorded input")
    })
}

/// A PCRE pattern (`#…#`) as a Rust regex.
fn pcre(pattern: &str) -> String {
    let delimiter = pattern.chars().next().unwrap_or('/');
    let end = pattern.rfind(delimiter).unwrap_or(pattern.len());
    pattern[1..end].to_owned()
}

/// The tests that replay their recording as it is.
const REPLAYED: &[&str] = &[
    "testAdditionallyIndentedLinesAreParsedAsNewLinesInFoldedBlocks",
    "testAnchorAfterTag",
    "testAnchorOnTheRootNode",
    "testBackslashInQuotedMultiLineString",
    "testBackslashInSingleQuotedString",
    "testBlankLinesAreParsedAsNewLinesInFoldedBlocks",
    "testBlankLinesInQuotedMultiLineString",
    "testBlockChomping",
    "testBlockLiteralWithLeadingNewlines",
    "testBlockMappingOnTheDocumentStartMarkerLine",
    "testBlockScalarArray",
    "testBlockScalarKeepsTrailingNewlineWhenNestedInMergeKey",
    "testBlockScalarKeepsTrailingNewlineWhenNestedInSequenceItem",
    "testBlockScalarsInCompactNestedCollections",
    "testBlockScalarsWithoutContent",
    "testBlockSequenceOnTheDocumentStartMarkerLine",
    "testBooleanKeys",
    "testBuiltInTagsWithoutValue",
    "testCanParseContentWithTrailingSpaces",
    "testColonInMappingValueException",
    "testColonInMappingValueExceptionNotTriggeredByColonInComment",
    "testCommentAtTheRootIndent",
    "testCommentCharactersInMultiLineQuotedStrings",
    "testCommentLikeStringsAreNotStrippedInBlockScalars",
    "testComplexMappingNestedInMappingThrowsParseException",
    "testComplexMappingNestedInSequenceThrowsParseException",
    "testComplexMappingThrowsParseException",
    "testContentOnTheDocumentStartMarkerLine",
    "testCoreTagsOnBlockScalars",
    "testCustomTagSupport",
    "testCustomTagsDisabled",
    "testDetectCircularReferences",
    "testDocumentEndMarkerFollowedByAnotherDocument",
    "testDocumentEndMarkerIsAWholeLine",
    "testDuplicateYamlDirective",
    "testEmptyLinesBeforeTheDocumentStartMarker",
    "testEmptyMappingKeyWithCustomTag",
    "testEmptyMappingKeyWithTag",
    "testEmptyValue",
    "testEmptyValueInExpandedMappingIsSupported",
    "testEmptyValueInExpandedSequenceIsSupported",
    "testEndOfTheDocumentMarker",
    "testEscapedQuoteInQuotedMultiLineString",
    "testEvalRefException",
    "testExceptionWhenUsingUnsupportedBuiltInTags",
    "testExplicitStringCasting",
    "testFilenamesAreParsedAsStringsWithoutFlag",
    "testFloatKeys",
    "testFoldedStringBlockWithComments",
    "testHashLedLinesAfterMoreIndentedEmptyLinesAreBlockScalarContent",
    "testIndentedDocumentMarkersAreContent",
    "testInlineMappingFollowedByMoreContentIsInvalid",
    "testInlineNotationSpanningMultipleLines",
    "testInlineSequenceFollowedByMoreContentIsInvalid",
    "testInvalidInlineSequenceContainingStringWithEscapedQuotationCharacter",
    "testKeyStartingWithNulByteThrowsParseExceptionWhenMappingsAreParsedAsObjects",
    "testLeadingCommentBlockIsIgnored",
    "testLineBreaksAroundMoreIndentedLinesInFoldedBlocks",
    "testLineNumberInException",
    "testMappingDuplicateKeyBlock",
    "testMappingDuplicateKeyFlow",
    "testMappingInASequence",
    "testMappingKeyInMultiLineStringThrowsException",
    "testMergeKeysInFlowMappingsWhenMappingsAreParsedAsObjects",
    "testMergeKeysWhenMappingsAreParsedAsObjects",
    "testMultiLineComment",
    "testMultiLineQuotedStringWithTrailingBackslash",
    "testMultiLineStringLastResortParsing",
    "testMultipleDocumentsNotSupportedException",
    "testNestedFoldedStringBlockWithComments",
    "testNonSpecificTagSupport",
    "testNonStringFollowedByCommentEmbeddedInMapping",
    "testNonUtf8Exception",
    "testNullAsDuplicatedData",
    "testObjectForMap",
    "testObjectSupportDisabledButNoExceptions",
    "testObjectSupportEnabled",
    "testObjectsSupportDisabledWithExceptions",
    "testParseAcceptsInlineSequencesAtTheNestingDepth",
    "testParseAcceptsLargeCollectionAliasedOnce",
    "testParseAcceptsScalarAliasesWithoutCountingThem",
    "testParseAliasFollowedByAComment",
    "testParseBinaryData",
    "testParseCountsTaggedValueCollectionAliases",
    "testParseDateAsMappingValue",
    "testParseDateWithSubseconds",
    "testParseEmptyBinaryData",
    "testParseExceptionNotAffectedByMultiLineStringLastResortParsing",
    "testParseExceptionOnDuplicate",
    "testParseFile",
    "testParseHandlesTrailingNewlinesAfterDocumentEndMarker",
    "testParseIdeographicSpaces",
    "testParseIndentedMappings",
    "testParseInlineMappingWithAnchoredQuotedValue",
    "testParseInlineMappingWithAnchoredQuotedValueContainingBraces",
    "testParseInvalidBinaryData",
    "testParseMergeKeyAliasFollowedByAComment",
    "testParseMergeKeyFollowedByATab",
    "testParseMultiLineMappingValue",
    "testParseMultiLineQuotedString",
    "testParseMultiLineString",
    "testParseMultiLineUnquotedString",
    "testParseMultiLineUnquotedStringWithTrailingComment",
    "testParseQuotedMergeKeyAsRegularKey",
    "testParseQuotedStringContainingEscapedQuotationCharacters",
    "testParseReferencesOnMergeKeys",
    "testParseReferencesOnMergeKeysWithMappingsParsedAsObjects",
    "testParseRejectsBlockAliasesWhenDisabled",
    "testParseRejectsDocumentsThatExceedTheConfiguredCollectionAliasLimit",
    "testParseRejectsDocumentsThatExceedTheDefaultNestingDepth",
    "testParseRejectsInlineAliasesWhenDisabled",
    "testParseRejectsUnterminatedInlineMappingsThatExceedTheNestingDepth",
    "testParseRejectsUnterminatedInlineSequencesThatExceedTheNestingDepth",
    "testParseValueWithModifiers",
    "testParseValueWithNegativeModifiers",
    "testParserCleansUpReferencesBetweenRuns",
    "testParserIsStateless",
    "testParserThrowsExceptionWithCorrectLineNumber",
    "testParsingIniThrowsException",
    "testParsingMultipleDocuments",
    "testParsingNonExistentFilesThrowsException",
    "testParsingNotReadableFilesThrowsException",
    "testPhpConstantTagMappingAsScalarKey",
    "testPhpConstantTagMappingKey",
    "testReferenceResolvingInInlineStrings",
    "testRootLevelInlineMappingFollowedByMoreContentIsInvalid",
    "testScalarInSequence",
    "testSequenceFollowedByCommentEmbeddedInMapping",
    "testSequenceInAMapping",
    "testSequenceInMappingStartedBySingleDashLine",
    "testShortcutKeyUnindentedCollectionException",
    "testSkipBlankLines",
    "testSpacesOnlyLinesInBlockScalarsWithIndentationIndicator",
    "testSpecifications",
    "testStringBlockWithComments",
    "testTabLedLinesAreMoreIndentedInFoldedBlocks",
    "testTabsAsIndentationInYaml",
    "testTabsInBlockScalarHeaders",
    "testTagMappingAsScalarKey",
    "testTaggedBlockScalarInNestedList",
    "testTaggedBlockScalarsAsListItems",
    "testTaggedInlineMapping",
    "testTaggedTextAsListItem",
    "testTaggedValueTopLevelAssoc",
    "testTaggedValueTopLevelAssocInline",
    "testTaggedValueTopLevelList",
    "testTaggedValueTopLevelNull",
    "testTaggedValueTopLevelNumber",
    "testTaggedValueTopLevelString",
    "testTaggedValueWithoutValue",
    "testThrowExceptionIfInvalidAdditionalClosingTagOccurs",
    "testTopLevelNull",
    "testTopLevelNumber",
    "testTrailingBackslashInSingleQuotedMultiLineString",
    "testTrailingBackslashesBeforeNewlineInQuotedMultiLineString",
    "testUnindentedCollectionException",
    "testUnquotedMultilineScalarHandlesCommentsAndBlanks",
    "testUnquotedMultilineScalarThrowsOnOrphanedLineAfterComment",
    "testUnquotedMultilineScalarWithBlankLines",
    "testUnsupportedBuiltInTagWithScalar",
    "testUnsupportedTagWithScalar",
    "testUppercaseHexadecimalPrefixKeys",
    "testValidTokenSeparation",
    "testWhiteSpaceIsFoldedInMultiLineQuotedString",
    "testWhitespaceAtEndOfLine",
    "testWrappedUnquotedStringWithMultipleSpacesInValue",
    "testWrongPhpConstantSyntax",
    "testYamlDirective",
];

#[test]
fn testAdditionallyIndentedLinesAreParsedAsNewLinesInFoldedBlocks() -> TestResult {
    replay("testAdditionallyIndentedLinesAreParsedAsNewLinesInFoldedBlocks")
}

#[test]
fn testAnchorAfterTag() -> TestResult {
    replay("testAnchorAfterTag")
}

#[test]
fn testAnchorOnTheRootNode() -> TestResult {
    replay("testAnchorOnTheRootNode")
}

#[test]
fn testBackslashInQuotedMultiLineString() -> TestResult {
    replay("testBackslashInQuotedMultiLineString")
}

#[test]
fn testBackslashInSingleQuotedString() -> TestResult {
    replay("testBackslashInSingleQuotedString")
}

#[test]
fn testBlankLinesAreParsedAsNewLinesInFoldedBlocks() -> TestResult {
    replay("testBlankLinesAreParsedAsNewLinesInFoldedBlocks")
}

#[test]
fn testBlankLinesInQuotedMultiLineString() -> TestResult {
    replay("testBlankLinesInQuotedMultiLineString")
}

#[test]
fn testBlockChomping() -> TestResult {
    replay("testBlockChomping")
}

#[test]
fn testBlockLiteralWithLeadingNewlines() -> TestResult {
    replay("testBlockLiteralWithLeadingNewlines")
}

#[test]
fn testBlockMappingOnTheDocumentStartMarkerLine() -> TestResult {
    replay("testBlockMappingOnTheDocumentStartMarkerLine")
}

#[test]
fn testBlockScalarArray() -> TestResult {
    replay("testBlockScalarArray")
}

#[test]
fn testBlockScalarKeepsTrailingNewlineWhenNestedInMergeKey() -> TestResult {
    replay("testBlockScalarKeepsTrailingNewlineWhenNestedInMergeKey")
}

#[test]
fn testBlockScalarKeepsTrailingNewlineWhenNestedInSequenceItem() -> TestResult {
    replay("testBlockScalarKeepsTrailingNewlineWhenNestedInSequenceItem")
}

#[test]
fn testBlockScalarsInCompactNestedCollections() -> TestResult {
    replay("testBlockScalarsInCompactNestedCollections")
}

#[test]
fn testBlockScalarsWithoutContent() -> TestResult {
    replay("testBlockScalarsWithoutContent")
}

#[test]
fn testBlockSequenceOnTheDocumentStartMarkerLine() -> TestResult {
    replay("testBlockSequenceOnTheDocumentStartMarkerLine")
}

#[test]
fn testBooleanKeys() -> TestResult {
    replay("testBooleanKeys")
}

#[test]
fn testBuiltInTagsWithoutValue() -> TestResult {
    replay("testBuiltInTagsWithoutValue")
}

#[test]
fn testCanParseContentWithTrailingSpaces() -> TestResult {
    replay("testCanParseContentWithTrailingSpaces")
}

#[test]
fn testColonInMappingValueException() -> TestResult {
    replay("testColonInMappingValueException")
}

#[test]
fn testColonInMappingValueExceptionNotTriggeredByColonInComment() -> TestResult {
    replay("testColonInMappingValueExceptionNotTriggeredByColonInComment")
}

#[test]
fn testCommentAtTheRootIndent() -> TestResult {
    replay("testCommentAtTheRootIndent")
}

#[test]
fn testCommentCharactersInMultiLineQuotedStrings() -> TestResult {
    replay("testCommentCharactersInMultiLineQuotedStrings")
}

#[test]
fn testCommentLikeStringsAreNotStrippedInBlockScalars() -> TestResult {
    replay("testCommentLikeStringsAreNotStrippedInBlockScalars")
}

#[test]
fn testComplexMappingNestedInMappingThrowsParseException() -> TestResult {
    replay("testComplexMappingNestedInMappingThrowsParseException")
}

#[test]
fn testComplexMappingNestedInSequenceThrowsParseException() -> TestResult {
    replay("testComplexMappingNestedInSequenceThrowsParseException")
}

#[test]
fn testComplexMappingThrowsParseException() -> TestResult {
    replay("testComplexMappingThrowsParseException")
}

#[test]
fn testContentOnTheDocumentStartMarkerLine() -> TestResult {
    replay("testContentOnTheDocumentStartMarkerLine")
}

#[test]
fn testCoreTagsOnBlockScalars() -> TestResult {
    replay("testCoreTagsOnBlockScalars")
}

#[test]
fn testCustomTagSupport() -> TestResult {
    replay("testCustomTagSupport")
}

#[test]
fn testCustomTagsDisabled() -> TestResult {
    replay("testCustomTagsDisabled")
}

#[test]
fn testDetectCircularReferences() -> TestResult {
    replay("testDetectCircularReferences")
}

#[test]
fn testDocumentEndMarkerFollowedByAnotherDocument() -> TestResult {
    replay("testDocumentEndMarkerFollowedByAnotherDocument")
}

#[test]
fn testDocumentEndMarkerIsAWholeLine() -> TestResult {
    replay("testDocumentEndMarkerIsAWholeLine")
}

#[test]
fn testDuplicateYamlDirective() -> TestResult {
    replay("testDuplicateYamlDirective")
}

#[test]
fn testEmptyLinesBeforeTheDocumentStartMarker() -> TestResult {
    replay("testEmptyLinesBeforeTheDocumentStartMarker")
}

#[test]
fn testEmptyMappingKeyWithCustomTag() -> TestResult {
    replay("testEmptyMappingKeyWithCustomTag")
}

#[test]
fn testEmptyMappingKeyWithTag() -> TestResult {
    replay("testEmptyMappingKeyWithTag")
}

#[test]
fn testEmptyValue() -> TestResult {
    replay("testEmptyValue")
}

#[test]
fn testEmptyValueInExpandedMappingIsSupported() -> TestResult {
    replay("testEmptyValueInExpandedMappingIsSupported")
}

#[test]
fn testEmptyValueInExpandedSequenceIsSupported() -> TestResult {
    replay("testEmptyValueInExpandedSequenceIsSupported")
}

#[test]
fn testEndOfTheDocumentMarker() -> TestResult {
    replay("testEndOfTheDocumentMarker")
}

#[test]
fn testEscapedQuoteInQuotedMultiLineString() -> TestResult {
    replay("testEscapedQuoteInQuotedMultiLineString")
}

#[test]
fn testEvalRefException() -> TestResult {
    replay("testEvalRefException")
}

#[test]
fn testExceptionWhenUsingUnsupportedBuiltInTags() -> TestResult {
    replay("testExceptionWhenUsingUnsupportedBuiltInTags")
}

#[test]
fn testExplicitStringCasting() -> TestResult {
    replay("testExplicitStringCasting")
}

#[test]
fn testFilenamesAreParsedAsStringsWithoutFlag() -> TestResult {
    replay("testFilenamesAreParsedAsStringsWithoutFlag")
}

#[test]
fn testFloatKeys() -> TestResult {
    replay("testFloatKeys")
}

#[test]
fn testFoldedStringBlockWithComments() -> TestResult {
    replay("testFoldedStringBlockWithComments")
}

#[test]
fn testHashLedLinesAfterMoreIndentedEmptyLinesAreBlockScalarContent() -> TestResult {
    replay("testHashLedLinesAfterMoreIndentedEmptyLinesAreBlockScalarContent")
}

#[test]
fn testIndentedDocumentMarkersAreContent() -> TestResult {
    replay("testIndentedDocumentMarkersAreContent")
}

#[test]
fn testInlineMappingFollowedByMoreContentIsInvalid() -> TestResult {
    replay("testInlineMappingFollowedByMoreContentIsInvalid")
}

#[test]
fn testInlineNotationSpanningMultipleLines() -> TestResult {
    replay("testInlineNotationSpanningMultipleLines")
}

#[test]
fn testInlineSequenceFollowedByMoreContentIsInvalid() -> TestResult {
    replay("testInlineSequenceFollowedByMoreContentIsInvalid")
}

#[test]
fn testInvalidInlineSequenceContainingStringWithEscapedQuotationCharacter() -> TestResult {
    replay("testInvalidInlineSequenceContainingStringWithEscapedQuotationCharacter")
}

#[test]
fn testKeyStartingWithNulByteThrowsParseExceptionWhenMappingsAreParsedAsObjects() -> TestResult {
    replay("testKeyStartingWithNulByteThrowsParseExceptionWhenMappingsAreParsedAsObjects")
}

#[test]
fn testLeadingCommentBlockIsIgnored() -> TestResult {
    replay("testLeadingCommentBlockIsIgnored")
}

#[test]
fn testLineBreaksAroundMoreIndentedLinesInFoldedBlocks() -> TestResult {
    replay("testLineBreaksAroundMoreIndentedLinesInFoldedBlocks")
}

#[test]
fn testLineNumberInException() -> TestResult {
    replay("testLineNumberInException")
}

#[test]
fn testMappingDuplicateKeyBlock() -> TestResult {
    replay("testMappingDuplicateKeyBlock")
}

#[test]
fn testMappingDuplicateKeyFlow() -> TestResult {
    replay("testMappingDuplicateKeyFlow")
}

#[test]
fn testMappingInASequence() -> TestResult {
    replay("testMappingInASequence")
}

#[test]
fn testMappingKeyInMultiLineStringThrowsException() -> TestResult {
    replay("testMappingKeyInMultiLineStringThrowsException")
}

#[test]
fn testMergeKeysInFlowMappingsWhenMappingsAreParsedAsObjects() -> TestResult {
    replay("testMergeKeysInFlowMappingsWhenMappingsAreParsedAsObjects")
}

#[test]
fn testMergeKeysWhenMappingsAreParsedAsObjects() -> TestResult {
    replay("testMergeKeysWhenMappingsAreParsedAsObjects")
}

#[test]
fn testMultiLineComment() -> TestResult {
    replay("testMultiLineComment")
}

#[test]
fn testMultiLineQuotedStringWithTrailingBackslash() -> TestResult {
    replay("testMultiLineQuotedStringWithTrailingBackslash")
}

#[test]
fn testMultiLineStringLastResortParsing() -> TestResult {
    replay("testMultiLineStringLastResortParsing")
}

#[test]
fn testMultipleDocumentsNotSupportedException() -> TestResult {
    replay("testMultipleDocumentsNotSupportedException")
}

#[test]
fn testNestedFoldedStringBlockWithComments() -> TestResult {
    replay("testNestedFoldedStringBlockWithComments")
}

#[test]
fn testNonSpecificTagSupport() -> TestResult {
    replay("testNonSpecificTagSupport")
}

#[test]
fn testNonStringFollowedByCommentEmbeddedInMapping() -> TestResult {
    replay("testNonStringFollowedByCommentEmbeddedInMapping")
}

#[test]
fn testNonUtf8Exception() -> TestResult {
    replay("testNonUtf8Exception")
}

#[test]
fn testNullAsDuplicatedData() -> TestResult {
    replay("testNullAsDuplicatedData")
}

#[test]
fn testObjectForMap() -> TestResult {
    replay("testObjectForMap")
}

#[test]
fn testObjectSupportDisabledButNoExceptions() -> TestResult {
    replay("testObjectSupportDisabledButNoExceptions")
}

#[test]
fn testObjectSupportEnabled() -> TestResult {
    replay("testObjectSupportEnabled")
}

#[test]
fn testObjectsSupportDisabledWithExceptions() -> TestResult {
    replay("testObjectsSupportDisabledWithExceptions")
}

#[test]
fn testParseAcceptsInlineSequencesAtTheNestingDepth() -> TestResult {
    replay("testParseAcceptsInlineSequencesAtTheNestingDepth")
}

#[test]
fn testParseAcceptsLargeCollectionAliasedOnce() -> TestResult {
    replay("testParseAcceptsLargeCollectionAliasedOnce")
}

#[test]
fn testParseAcceptsScalarAliasesWithoutCountingThem() -> TestResult {
    replay("testParseAcceptsScalarAliasesWithoutCountingThem")
}

#[test]
fn testParseAliasFollowedByAComment() -> TestResult {
    replay("testParseAliasFollowedByAComment")
}

#[test]
fn testParseBinaryData() -> TestResult {
    replay("testParseBinaryData")
}

#[test]
fn testParseCountsTaggedValueCollectionAliases() -> TestResult {
    replay("testParseCountsTaggedValueCollectionAliases")
}

#[test]
fn testParseDateAsMappingValue() -> TestResult {
    replay("testParseDateAsMappingValue")
}

#[test]
fn testParseDateWithSubseconds() -> TestResult {
    replay("testParseDateWithSubseconds")
}

#[test]
fn testParseEmptyBinaryData() -> TestResult {
    replay("testParseEmptyBinaryData")
}

#[test]
fn testParseExceptionNotAffectedByMultiLineStringLastResortParsing() -> TestResult {
    replay("testParseExceptionNotAffectedByMultiLineStringLastResortParsing")
}

#[test]
fn testParseExceptionOnDuplicate() -> TestResult {
    replay("testParseExceptionOnDuplicate")
}

#[test]
fn testParseFile() -> TestResult {
    replay("testParseFile")
}

#[test]
fn testParseHandlesTrailingNewlinesAfterDocumentEndMarker() -> TestResult {
    replay("testParseHandlesTrailingNewlinesAfterDocumentEndMarker")
}

#[test]
fn testParseIdeographicSpaces() -> TestResult {
    replay("testParseIdeographicSpaces")
}

#[test]
fn testParseIndentedMappings() -> TestResult {
    replay("testParseIndentedMappings")
}

#[test]
fn testParseInlineMappingWithAnchoredQuotedValue() -> TestResult {
    replay("testParseInlineMappingWithAnchoredQuotedValue")
}

#[test]
fn testParseInlineMappingWithAnchoredQuotedValueContainingBraces() -> TestResult {
    replay("testParseInlineMappingWithAnchoredQuotedValueContainingBraces")
}

#[test]
fn testParseInvalidBinaryData() -> TestResult {
    replay("testParseInvalidBinaryData")
}

#[test]
fn testParseMergeKeyAliasFollowedByAComment() -> TestResult {
    replay("testParseMergeKeyAliasFollowedByAComment")
}

#[test]
fn testParseMergeKeyFollowedByATab() -> TestResult {
    replay("testParseMergeKeyFollowedByATab")
}

#[test]
fn testParseMultiLineMappingValue() -> TestResult {
    replay("testParseMultiLineMappingValue")
}

#[test]
fn testParseMultiLineQuotedString() -> TestResult {
    replay("testParseMultiLineQuotedString")
}

#[test]
fn testParseMultiLineString() -> TestResult {
    replay("testParseMultiLineString")
}

#[test]
fn testParseMultiLineUnquotedString() -> TestResult {
    replay("testParseMultiLineUnquotedString")
}

#[test]
fn testParseMultiLineUnquotedStringWithTrailingComment() -> TestResult {
    replay("testParseMultiLineUnquotedStringWithTrailingComment")
}

#[test]
fn testParseQuotedMergeKeyAsRegularKey() -> TestResult {
    replay("testParseQuotedMergeKeyAsRegularKey")
}

#[test]
fn testParseQuotedStringContainingEscapedQuotationCharacters() -> TestResult {
    replay("testParseQuotedStringContainingEscapedQuotationCharacters")
}

#[test]
fn testParseReferencesOnMergeKeys() -> TestResult {
    replay("testParseReferencesOnMergeKeys")
}

#[test]
fn testParseReferencesOnMergeKeysWithMappingsParsedAsObjects() -> TestResult {
    replay("testParseReferencesOnMergeKeysWithMappingsParsedAsObjects")
}

#[test]
fn testParseRejectsBlockAliasesWhenDisabled() -> TestResult {
    replay("testParseRejectsBlockAliasesWhenDisabled")
}

#[test]
fn testParseRejectsDocumentsThatExceedTheConfiguredCollectionAliasLimit() -> TestResult {
    replay("testParseRejectsDocumentsThatExceedTheConfiguredCollectionAliasLimit")
}

#[test]
fn testParseRejectsDocumentsThatExceedTheDefaultNestingDepth() -> TestResult {
    replay("testParseRejectsDocumentsThatExceedTheDefaultNestingDepth")
}

#[test]
fn testParseRejectsInlineAliasesWhenDisabled() -> TestResult {
    replay("testParseRejectsInlineAliasesWhenDisabled")
}

#[test]
fn testParseRejectsUnterminatedInlineMappingsThatExceedTheNestingDepth() -> TestResult {
    replay("testParseRejectsUnterminatedInlineMappingsThatExceedTheNestingDepth")
}

#[test]
fn testParseRejectsUnterminatedInlineSequencesThatExceedTheNestingDepth() -> TestResult {
    replay("testParseRejectsUnterminatedInlineSequencesThatExceedTheNestingDepth")
}

#[test]
fn testParseValueWithModifiers() -> TestResult {
    replay("testParseValueWithModifiers")
}

#[test]
fn testParseValueWithNegativeModifiers() -> TestResult {
    replay("testParseValueWithNegativeModifiers")
}

#[test]
fn testParserCleansUpReferencesBetweenRuns() -> TestResult {
    replay("testParserCleansUpReferencesBetweenRuns")
}

#[test]
fn testParserIsStateless() -> TestResult {
    replay("testParserIsStateless")
}

#[test]
fn testParserThrowsExceptionWithCorrectLineNumber() -> TestResult {
    replay("testParserThrowsExceptionWithCorrectLineNumber")
}

#[test]
fn testParsingIniThrowsException() -> TestResult {
    replay("testParsingIniThrowsException")
}

#[test]
fn testParsingMultipleDocuments() -> TestResult {
    replay("testParsingMultipleDocuments")
}

#[test]
fn testParsingNonExistentFilesThrowsException() -> TestResult {
    replay("testParsingNonExistentFilesThrowsException")
}

#[test]
fn testParsingNotReadableFilesThrowsException() -> TestResult {
    replay("testParsingNotReadableFilesThrowsException")
}

#[test]
fn testPhpConstantTagMappingAsScalarKey() -> TestResult {
    replay("testPhpConstantTagMappingAsScalarKey")
}

#[test]
fn testPhpConstantTagMappingKey() -> TestResult {
    replay("testPhpConstantTagMappingKey")
}

#[test]
fn testReferenceResolvingInInlineStrings() -> TestResult {
    replay("testReferenceResolvingInInlineStrings")
}

#[test]
fn testRootLevelInlineMappingFollowedByMoreContentIsInvalid() -> TestResult {
    replay("testRootLevelInlineMappingFollowedByMoreContentIsInvalid")
}

#[test]
fn testScalarInSequence() -> TestResult {
    replay("testScalarInSequence")
}

#[test]
fn testSequenceFollowedByCommentEmbeddedInMapping() -> TestResult {
    replay("testSequenceFollowedByCommentEmbeddedInMapping")
}

#[test]
fn testSequenceInAMapping() -> TestResult {
    replay("testSequenceInAMapping")
}

#[test]
fn testSequenceInMappingStartedBySingleDashLine() -> TestResult {
    replay("testSequenceInMappingStartedBySingleDashLine")
}

#[test]
fn testShortcutKeyUnindentedCollectionException() -> TestResult {
    replay("testShortcutKeyUnindentedCollectionException")
}

#[test]
fn testSkipBlankLines() -> TestResult {
    replay("testSkipBlankLines")
}

#[test]
fn testSpacesOnlyLinesInBlockScalarsWithIndentationIndicator() -> TestResult {
    replay("testSpacesOnlyLinesInBlockScalarsWithIndentationIndicator")
}

#[test]
fn testSpecifications() -> TestResult {
    replay("testSpecifications")
}

#[test]
fn testStringBlockWithComments() -> TestResult {
    replay("testStringBlockWithComments")
}

#[test]
fn testTabLedLinesAreMoreIndentedInFoldedBlocks() -> TestResult {
    replay("testTabLedLinesAreMoreIndentedInFoldedBlocks")
}

#[test]
fn testTabsAsIndentationInYaml() -> TestResult {
    replay("testTabsAsIndentationInYaml")
}

#[test]
fn testTabsInBlockScalarHeaders() -> TestResult {
    replay("testTabsInBlockScalarHeaders")
}

#[test]
fn testTagMappingAsScalarKey() -> TestResult {
    replay("testTagMappingAsScalarKey")
}

#[test]
fn testTaggedBlockScalarInNestedList() -> TestResult {
    replay("testTaggedBlockScalarInNestedList")
}

#[test]
fn testTaggedBlockScalarsAsListItems() -> TestResult {
    replay("testTaggedBlockScalarsAsListItems")
}

#[test]
fn testTaggedInlineMapping() -> TestResult {
    replay("testTaggedInlineMapping")
}

#[test]
fn testTaggedTextAsListItem() -> TestResult {
    replay("testTaggedTextAsListItem")
}

#[test]
fn testTaggedValueTopLevelAssoc() -> TestResult {
    replay("testTaggedValueTopLevelAssoc")
}

#[test]
fn testTaggedValueTopLevelAssocInline() -> TestResult {
    replay("testTaggedValueTopLevelAssocInline")
}

#[test]
fn testTaggedValueTopLevelList() -> TestResult {
    replay("testTaggedValueTopLevelList")
}

#[test]
fn testTaggedValueTopLevelNull() -> TestResult {
    replay("testTaggedValueTopLevelNull")
}

#[test]
fn testTaggedValueTopLevelNumber() -> TestResult {
    replay("testTaggedValueTopLevelNumber")
}

#[test]
fn testTaggedValueTopLevelString() -> TestResult {
    replay("testTaggedValueTopLevelString")
}

#[test]
fn testTaggedValueWithoutValue() -> TestResult {
    replay("testTaggedValueWithoutValue")
}

#[test]
fn testThrowExceptionIfInvalidAdditionalClosingTagOccurs() -> TestResult {
    replay("testThrowExceptionIfInvalidAdditionalClosingTagOccurs")
}

#[test]
fn testTopLevelNull() -> TestResult {
    replay("testTopLevelNull")
}

#[test]
fn testTopLevelNumber() -> TestResult {
    replay("testTopLevelNumber")
}

#[test]
fn testTrailingBackslashInSingleQuotedMultiLineString() -> TestResult {
    replay("testTrailingBackslashInSingleQuotedMultiLineString")
}

#[test]
fn testTrailingBackslashesBeforeNewlineInQuotedMultiLineString() -> TestResult {
    replay("testTrailingBackslashesBeforeNewlineInQuotedMultiLineString")
}

#[test]
fn testUnindentedCollectionException() -> TestResult {
    replay("testUnindentedCollectionException")
}

#[test]
fn testUnquotedMultilineScalarHandlesCommentsAndBlanks() -> TestResult {
    replay("testUnquotedMultilineScalarHandlesCommentsAndBlanks")
}

#[test]
fn testUnquotedMultilineScalarThrowsOnOrphanedLineAfterComment() -> TestResult {
    replay("testUnquotedMultilineScalarThrowsOnOrphanedLineAfterComment")
}

#[test]
fn testUnquotedMultilineScalarWithBlankLines() -> TestResult {
    replay("testUnquotedMultilineScalarWithBlankLines")
}

#[test]
fn testUnsupportedBuiltInTagWithScalar() -> TestResult {
    replay("testUnsupportedBuiltInTagWithScalar")
}

#[test]
fn testUnsupportedTagWithScalar() -> TestResult {
    replay("testUnsupportedTagWithScalar")
}

#[test]
fn testUppercaseHexadecimalPrefixKeys() -> TestResult {
    replay("testUppercaseHexadecimalPrefixKeys")
}

#[test]
fn testValidTokenSeparation() -> TestResult {
    replay("testValidTokenSeparation")
}

#[test]
fn testWhiteSpaceIsFoldedInMultiLineQuotedString() -> TestResult {
    replay("testWhiteSpaceIsFoldedInMultiLineQuotedString")
}

#[test]
fn testWhitespaceAtEndOfLine() -> TestResult {
    replay("testWhitespaceAtEndOfLine")
}

#[test]
fn testWrappedUnquotedStringWithMultipleSpacesInValue() -> TestResult {
    replay("testWrappedUnquotedStringWithMultipleSpacesInValue")
}

#[test]
fn testWrongPhpConstantSyntax() -> TestResult {
    replay("testWrongPhpConstantSyntax")
}

#[test]
fn testYamlDirective() -> TestResult {
    replay("testYamlDirective")
}

/// The tests written out below, as they build inputs too large to record.
const BUILT: &[&str] = &[
    "testCanParseVeryLongValue",
    "testParseHandlesLargeDocumentMarkers",
    "testParseHandlesLargeLeadingComments",
    "testParseHandlesLargeYamlHeaders",
];

#[test]
fn every_upstream_test_is_here() {
    let recorded: Vec<&String> = recordings()
        .as_object()
        .map(|o| o.keys().collect())
        .unwrap_or_default();
    let mut here: Vec<&str> = REPLAYED.iter().chain(BUILT).copied().collect();
    here.sort_unstable();
    assert_eq!(recorded, here);
}

#[test]
fn testCanParseVeryLongValue() -> TestResult {
    let long_string_with_spaces = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ".repeat(20000);
    let tricky_val = php_array!["x" => long_string_with_spaces.as_str()];
    // Yaml::dump($trickyVal): one mapping line, the value inline
    let inline = Inline::new(environment());
    let value = inline.dump(
        &Value::from(long_string_with_spaces.as_str()),
        Flags::NONE,
        false,
    )?;
    let yaml_string = format!("x: {}\n", value.to_string_lossy());
    replay_with("testCanParseVeryLongValue", &|_| {
        yaml_string.clone().into_bytes()
    })?;

    let array_from_yaml =
        Parser::with_environment(environment()).parse(yaml_string.as_bytes(), Flags::NONE)?;
    assert!(same(&Value::Array(tricky_val), &array_from_yaml));
    Ok(())
}

#[test]
fn testParseHandlesLargeYamlHeaders() -> TestResult {
    let yaml = format!("%YAML:{}\nfoo: bar\n", "1".repeat(100000));
    replay_with("testParseHandlesLargeYamlHeaders", &|_| {
        yaml.clone().into_bytes()
    })
}

#[test]
fn testParseHandlesLargeLeadingComments() -> TestResult {
    let yaml = format!("#{}\nfoo: bar\n", "comment".repeat(20000));
    replay_with("testParseHandlesLargeLeadingComments", &|_| {
        yaml.clone().into_bytes()
    })
}

#[test]
fn testParseHandlesLargeDocumentMarkers() -> TestResult {
    let yaml = format!("--- # {}\nfoo: bar\n...   ", "header".repeat(20000));
    replay_with("testParseHandlesLargeDocumentMarkers", &|_| {
        yaml.clone().into_bytes()
    })
}
