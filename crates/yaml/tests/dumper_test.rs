//! DumperTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//!
//! Each test has its upstream name and replays every call the upstream
//! test makes to the parser and the dumper, for each of its data-provider
//! rows, from tests/data/dumper_test.json (common/replay.rs says how).
//!
//! Upstream: symfony/yaml Tests/DumperTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::LazyLock;

use common::replay::{self, TestResult};
use serde_json::Value as Json;

fn recording() -> &'static Json {
    static DATA: LazyLock<Json> =
        LazyLock::new(|| replay::load(include_str!("data/dumper_test.json")));
    &DATA
}

fn replay(test: &str) -> TestResult {
    replay::replay(recording(), test)
}

const REPLAYED: &[&str] = &[
    "testBinaryDataIsDumpedBase64Encoded",
    "testCanForceQuotesOnValues",
    "testCarriageReturnFollowedByNewlineIsMaintainedWhenDumpingAsMultiLineLiteralBlock",
    "testCarriageReturnNotFollowedByNewlineIsPreservedWhenDumpingAsMultiLineLiteralBlock",
    "testDumpCompactNestedMapping",
    "testDumpDateTime",
    "testDumpDocumentMarkersAsTopLevelStrings",
    "testDumpEmptyArrayObjectInstanceAsMap",
    "testDumpEmptyStdClassInstanceAsMap",
    "testDumpIdeographicSpaces",
    "testDumpInlineNumericKeyAsString",
    "testDumpMultiLineStringAsScalarBlock",
    "testDumpMultiLineStringAsScalarBlockWhenFirstLineHasLeadingSpace",
    "testDumpMultiLineStringAsScalarBlockWhenFirstLineHasOnlySpaces",
    "testDumpMultiLineStringAsScalarBlockWhenFirstLineIsEmptyAndSecondLineHasLeadingSpace",
    "testDumpMultiLineStringWithLeadingSpacesOnlyLines",
    "testDumpNullAsEmptyAtRoot",
    "testDumpNullAsEmptyDumpsNestedMaps",
    "testDumpNullAsEmptyDumpsWhenInInlineMapping",
    "testDumpNullAsEmptyInExpandedMapping",
    "testDumpNullAsEmptyInExpandedSequence",
    "testDumpNullAsEmptyWhenInInlineSequence",
    "testDumpNullAsEmptyWithObject",
    "testDumpNullAsTilde",
    "testDumpObjectAsMap",
    "testDumpSimpleHashesInSequences",
    "testDumpSingleLineBreakAsMultiLineLiteralBlock",
    "testDumpTrailingNewlineInMultiLineLiteralBlocks",
    "testDumpTrailingNewlineInMultiLineLiteralBlocksForTaggedValues",
    "testDumpWithMultipleNullFlagsFormatsThrows",
    "testDumpedMergeKeyStringCanBeParsedBack",
    "testDumpingArrayObjectInstancesRespectsInlineLevel",
    "testDumpingArrayObjectInstancesWithNumericKeysInlined",
    "testDumpingArrayObjectInstancesWithNumericKeysRespectsInlineLevel",
    "testDumpingInlinedMultiLineIfRnBreakLineInTaggedValue",
    "testDumpingMultiLineStringAsScalarBlockTaggedValue",
    "testDumpingNotInlinedNullTaggedValue",
    "testDumpingNotInlinedScalarTaggedValue",
    "testDumpingStdClassInstancesRespectsInlineLevel",
    "testDumpingTaggedMultiLineInList",
    "testDumpingTaggedMultiLineTrailingNewlinesInList",
    "testDumpingTaggedMultiLineTrailingNewlinesInMap",
    "testDumpingTaggedValueMapRespectsInlineLevel",
    "testDumpingTaggedValueMapWithInlinedTagValues",
    "testDumpingTaggedValueSequenceRespectsInlineLevel",
    "testDumpingTaggedValueSequenceWithInlinedTagValues",
    "testDumpingTaggedValueSpecialCharsInTag",
    "testDumpingTaggedValueTopLevelAssoc",
    "testDumpingTaggedValueTopLevelAssocInline",
    "testDumpingTaggedValueTopLevelMultiLine",
    "testDumpingTaggedValueTopLevelScalar",
    "testDumpingTaggedValueWithCarriageReturnAsQuotedString",
    "testEscapedEscapeSequencesInQuotedScalar",
    "testIndentationInConstructor",
    "testInlineLevel",
    "testNegativeIndentationThrowsException",
    "testNoExtraTrailingNewlineWhenDumpingAsMultiLineLiteralBlock",
    "testNonUtf8DataIsDumpedBase64Encoded",
    "testObjectSupportDisabledButNoExceptions",
    "testObjectSupportDisabledWithExceptions",
    "testObjectSupportEnabled",
    "testSpecifications",
    "testTaggedMultiLineLiteralBlocksRoundTrip",
    "testTopLevelMultiLineStringLiteral",
    "testTopLevelTaggedMultiLineLiteralBlock",
    "testZeroIndentationThrowsException",
];

#[test]
fn every_upstream_test_is_here() {
    assert_eq!(replay::names(recording()), REPLAYED);
}

#[test]
fn testBinaryDataIsDumpedBase64Encoded() -> TestResult {
    replay("testBinaryDataIsDumpedBase64Encoded")
}

#[test]
fn testCanForceQuotesOnValues() -> TestResult {
    replay("testCanForceQuotesOnValues")
}

#[test]
fn testCarriageReturnFollowedByNewlineIsMaintainedWhenDumpingAsMultiLineLiteralBlock() -> TestResult
{
    replay("testCarriageReturnFollowedByNewlineIsMaintainedWhenDumpingAsMultiLineLiteralBlock")
}

#[test]
fn testCarriageReturnNotFollowedByNewlineIsPreservedWhenDumpingAsMultiLineLiteralBlock()
-> TestResult {
    replay("testCarriageReturnNotFollowedByNewlineIsPreservedWhenDumpingAsMultiLineLiteralBlock")
}

#[test]
fn testDumpCompactNestedMapping() -> TestResult {
    replay("testDumpCompactNestedMapping")
}

#[test]
fn testDumpDateTime() -> TestResult {
    replay("testDumpDateTime")
}

#[test]
fn testDumpDocumentMarkersAsTopLevelStrings() -> TestResult {
    replay("testDumpDocumentMarkersAsTopLevelStrings")
}

#[test]
fn testDumpEmptyArrayObjectInstanceAsMap() -> TestResult {
    replay("testDumpEmptyArrayObjectInstanceAsMap")
}

#[test]
fn testDumpEmptyStdClassInstanceAsMap() -> TestResult {
    replay("testDumpEmptyStdClassInstanceAsMap")
}

#[test]
fn testDumpIdeographicSpaces() -> TestResult {
    replay("testDumpIdeographicSpaces")
}

#[test]
fn testDumpInlineNumericKeyAsString() -> TestResult {
    replay("testDumpInlineNumericKeyAsString")
}

#[test]
fn testDumpMultiLineStringAsScalarBlock() -> TestResult {
    replay("testDumpMultiLineStringAsScalarBlock")
}

#[test]
fn testDumpMultiLineStringAsScalarBlockWhenFirstLineHasLeadingSpace() -> TestResult {
    replay("testDumpMultiLineStringAsScalarBlockWhenFirstLineHasLeadingSpace")
}

#[test]
fn testDumpMultiLineStringAsScalarBlockWhenFirstLineHasOnlySpaces() -> TestResult {
    replay("testDumpMultiLineStringAsScalarBlockWhenFirstLineHasOnlySpaces")
}

#[test]
fn testDumpMultiLineStringAsScalarBlockWhenFirstLineIsEmptyAndSecondLineHasLeadingSpace()
-> TestResult {
    replay("testDumpMultiLineStringAsScalarBlockWhenFirstLineIsEmptyAndSecondLineHasLeadingSpace")
}

#[test]
fn testDumpMultiLineStringWithLeadingSpacesOnlyLines() -> TestResult {
    replay("testDumpMultiLineStringWithLeadingSpacesOnlyLines")
}

#[test]
fn testDumpNullAsEmptyAtRoot() -> TestResult {
    replay("testDumpNullAsEmptyAtRoot")
}

#[test]
fn testDumpNullAsEmptyDumpsNestedMaps() -> TestResult {
    replay("testDumpNullAsEmptyDumpsNestedMaps")
}

#[test]
fn testDumpNullAsEmptyDumpsWhenInInlineMapping() -> TestResult {
    replay("testDumpNullAsEmptyDumpsWhenInInlineMapping")
}

#[test]
fn testDumpNullAsEmptyInExpandedMapping() -> TestResult {
    replay("testDumpNullAsEmptyInExpandedMapping")
}

#[test]
fn testDumpNullAsEmptyInExpandedSequence() -> TestResult {
    replay("testDumpNullAsEmptyInExpandedSequence")
}

#[test]
fn testDumpNullAsEmptyWhenInInlineSequence() -> TestResult {
    replay("testDumpNullAsEmptyWhenInInlineSequence")
}

#[test]
fn testDumpNullAsEmptyWithObject() -> TestResult {
    replay("testDumpNullAsEmptyWithObject")
}

#[test]
fn testDumpNullAsTilde() -> TestResult {
    replay("testDumpNullAsTilde")
}

#[test]
fn testDumpObjectAsMap() -> TestResult {
    replay("testDumpObjectAsMap")
}

#[test]
fn testDumpSimpleHashesInSequences() -> TestResult {
    replay("testDumpSimpleHashesInSequences")
}

#[test]
fn testDumpSingleLineBreakAsMultiLineLiteralBlock() -> TestResult {
    replay("testDumpSingleLineBreakAsMultiLineLiteralBlock")
}

#[test]
fn testDumpTrailingNewlineInMultiLineLiteralBlocks() -> TestResult {
    replay("testDumpTrailingNewlineInMultiLineLiteralBlocks")
}

#[test]
fn testDumpTrailingNewlineInMultiLineLiteralBlocksForTaggedValues() -> TestResult {
    replay("testDumpTrailingNewlineInMultiLineLiteralBlocksForTaggedValues")
}

#[test]
fn testDumpWithMultipleNullFlagsFormatsThrows() -> TestResult {
    replay("testDumpWithMultipleNullFlagsFormatsThrows")
}

#[test]
fn testDumpedMergeKeyStringCanBeParsedBack() -> TestResult {
    replay("testDumpedMergeKeyStringCanBeParsedBack")
}

#[test]
fn testDumpingArrayObjectInstancesRespectsInlineLevel() -> TestResult {
    replay("testDumpingArrayObjectInstancesRespectsInlineLevel")
}

#[test]
fn testDumpingArrayObjectInstancesWithNumericKeysInlined() -> TestResult {
    replay("testDumpingArrayObjectInstancesWithNumericKeysInlined")
}

#[test]
fn testDumpingArrayObjectInstancesWithNumericKeysRespectsInlineLevel() -> TestResult {
    replay("testDumpingArrayObjectInstancesWithNumericKeysRespectsInlineLevel")
}

#[test]
fn testDumpingInlinedMultiLineIfRnBreakLineInTaggedValue() -> TestResult {
    replay("testDumpingInlinedMultiLineIfRnBreakLineInTaggedValue")
}

#[test]
fn testDumpingMultiLineStringAsScalarBlockTaggedValue() -> TestResult {
    replay("testDumpingMultiLineStringAsScalarBlockTaggedValue")
}

#[test]
fn testDumpingNotInlinedNullTaggedValue() -> TestResult {
    replay("testDumpingNotInlinedNullTaggedValue")
}

#[test]
fn testDumpingNotInlinedScalarTaggedValue() -> TestResult {
    replay("testDumpingNotInlinedScalarTaggedValue")
}

#[test]
fn testDumpingStdClassInstancesRespectsInlineLevel() -> TestResult {
    replay("testDumpingStdClassInstancesRespectsInlineLevel")
}

#[test]
fn testDumpingTaggedMultiLineInList() -> TestResult {
    replay("testDumpingTaggedMultiLineInList")
}

#[test]
fn testDumpingTaggedMultiLineTrailingNewlinesInList() -> TestResult {
    replay("testDumpingTaggedMultiLineTrailingNewlinesInList")
}

#[test]
fn testDumpingTaggedMultiLineTrailingNewlinesInMap() -> TestResult {
    replay("testDumpingTaggedMultiLineTrailingNewlinesInMap")
}

#[test]
fn testDumpingTaggedValueMapRespectsInlineLevel() -> TestResult {
    replay("testDumpingTaggedValueMapRespectsInlineLevel")
}

#[test]
fn testDumpingTaggedValueMapWithInlinedTagValues() -> TestResult {
    replay("testDumpingTaggedValueMapWithInlinedTagValues")
}

#[test]
fn testDumpingTaggedValueSequenceRespectsInlineLevel() -> TestResult {
    replay("testDumpingTaggedValueSequenceRespectsInlineLevel")
}

#[test]
fn testDumpingTaggedValueSequenceWithInlinedTagValues() -> TestResult {
    replay("testDumpingTaggedValueSequenceWithInlinedTagValues")
}

#[test]
fn testDumpingTaggedValueSpecialCharsInTag() -> TestResult {
    replay("testDumpingTaggedValueSpecialCharsInTag")
}

#[test]
fn testDumpingTaggedValueTopLevelAssoc() -> TestResult {
    replay("testDumpingTaggedValueTopLevelAssoc")
}

#[test]
fn testDumpingTaggedValueTopLevelAssocInline() -> TestResult {
    replay("testDumpingTaggedValueTopLevelAssocInline")
}

#[test]
fn testDumpingTaggedValueTopLevelMultiLine() -> TestResult {
    replay("testDumpingTaggedValueTopLevelMultiLine")
}

#[test]
fn testDumpingTaggedValueTopLevelScalar() -> TestResult {
    replay("testDumpingTaggedValueTopLevelScalar")
}

#[test]
fn testDumpingTaggedValueWithCarriageReturnAsQuotedString() -> TestResult {
    replay("testDumpingTaggedValueWithCarriageReturnAsQuotedString")
}

#[test]
fn testEscapedEscapeSequencesInQuotedScalar() -> TestResult {
    replay("testEscapedEscapeSequencesInQuotedScalar")
}

#[test]
fn testIndentationInConstructor() -> TestResult {
    replay("testIndentationInConstructor")
}

#[test]
fn testInlineLevel() -> TestResult {
    replay("testInlineLevel")
}

#[test]
fn testNegativeIndentationThrowsException() -> TestResult {
    replay("testNegativeIndentationThrowsException")
}

#[test]
fn testNoExtraTrailingNewlineWhenDumpingAsMultiLineLiteralBlock() -> TestResult {
    replay("testNoExtraTrailingNewlineWhenDumpingAsMultiLineLiteralBlock")
}

#[test]
fn testNonUtf8DataIsDumpedBase64Encoded() -> TestResult {
    replay("testNonUtf8DataIsDumpedBase64Encoded")
}

#[test]
fn testObjectSupportDisabledButNoExceptions() -> TestResult {
    replay("testObjectSupportDisabledButNoExceptions")
}

#[test]
fn testObjectSupportDisabledWithExceptions() -> TestResult {
    replay("testObjectSupportDisabledWithExceptions")
}

#[test]
fn testObjectSupportEnabled() -> TestResult {
    replay("testObjectSupportEnabled")
}

#[test]
fn testSpecifications() -> TestResult {
    replay("testSpecifications")
}

#[test]
fn testTaggedMultiLineLiteralBlocksRoundTrip() -> TestResult {
    replay("testTaggedMultiLineLiteralBlocksRoundTrip")
}

#[test]
fn testTopLevelMultiLineStringLiteral() -> TestResult {
    replay("testTopLevelMultiLineStringLiteral")
}

#[test]
fn testTopLevelTaggedMultiLineLiteralBlock() -> TestResult {
    replay("testTopLevelTaggedMultiLineLiteralBlock")
}

#[test]
fn testZeroIndentationThrowsException() -> TestResult {
    replay("testZeroIndentationThrowsException")
}
