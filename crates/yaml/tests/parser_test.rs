//! ParserTest.php at symfony/yaml 6107ee26a25215c3df3af0a3315e56bb4fd0b7db.
//!
//! Each test has its upstream name and replays every call the upstream
//! test makes to the parser (through `$this->parser`, `new Parser(...)`,
//! `Yaml::parse()`) and the dumper, for each of its data-provider rows,
//! from `tests/data/parser_test.json` (common/replay.rs says how).
//!
//! Inputs over 64 KiB are not recorded: the four tests that build them
//! with `str_repeat()` build them here the same way, and check them
//! against the recorded length. The unreadable-file case is exercised
//! live on Unix against an unprivileged PHP oracle recording; its old
//! root-oracle recording was skipped. scripts/linux-test.sh runs non-root.
//!
//! Upstream: symfony/yaml Tests/ParserTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::LazyLock;

use common::replay::{self, TestResult, same};
use php_value::{Value, php_array};
use serde_json::Value as Json;
use symfony_yaml::{Flags, Parser};

fn recording() -> &'static Json {
    static DATA: LazyLock<Json> =
        LazyLock::new(|| replay::load(include_str!("data/parser_test.json")));
    &DATA
}

fn replay(test: &str) -> TestResult {
    replay::replay(recording(), test)
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

#[cfg(unix)]
#[test]
fn testParsingNotReadableFilesThrowsException() -> TestResult {
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "symfony-yaml-parser-{}-{nonce}",
        std::process::id()
    ));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let directory = Directory(directory);
    fs::create_dir(directory.0.join("Fixtures"))?;
    let file = directory.0.join("Fixtures/not_readable.yml");
    fs::write(&file, include_bytes!("Fixtures/not_readable.yml"))?;
    let filename = String::from_utf8(php_value::path::from_path(&file))?;
    let root = String::from_utf8(php_value::path::from_path(&directory.0))?;
    let recorded: Json = serde_json::from_str(include_str!("data/unreadable_file.json"))?;
    let mut parser = Parser::new();

    let readable = parser.parse_file(&filename, Flags::NONE)?;
    assert!(same(
        &php_value::vectors::decode(&recorded["readable"])?,
        &readable
    ));
    fs::set_permissions(&file, fs::Permissions::from_mode(0o200))?;
    assert_eq!(
        fs::File::open(&file).err().map(|error| error.kind()),
        Some(std::io::ErrorKind::PermissionDenied),
        "Run this permission-dependent case as an unprivileged user."
    );

    // The Result error is ParseException, as upstream expects.
    let error = parser
        .parse_file(&filename, Flags::NONE)
        .err()
        .ok_or("the unreadable file unexpectedly parsed")?;
    assert_eq!(recorded["unreadable"][0], "throws");
    assert_eq!(
        recorded["unreadable"][1],
        r"Symfony\Component\Yaml\Exception\ParseException"
    );
    let expected = recorded["unreadable"][2]
        .as_str()
        .ok_or("missing recorded exception message")?
        .replace("{tmp}", &root);
    assert_eq!(error.get_message().as_bytes(), expected.as_bytes());
    // ParserTest.php's expectExceptionMessageMatches(), unchanged.
    assert!(
        regex::Regex::new(r#"^File ".+/Fixtures/not_readable.yml" cannot be read\.$"#)?
            .is_match(&error.get_message().to_string())
    );

    fs::set_permissions(&file, fs::Permissions::from_mode(0o644))?;
    let restored = parser.parse_file(&filename, Flags::NONE)?;
    assert!(same(
        &php_value::vectors::decode(&recorded["restored"])?,
        &restored
    ));
    Ok(())
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

/// Tests with constructed large inputs or a live Unix filesystem precondition.
const BUILT: &[&str] = &[
    "testCanParseVeryLongValue",
    "testParseHandlesLargeDocumentMarkers",
    "testParseHandlesLargeLeadingComments",
    "testParseHandlesLargeYamlHeaders",
    "testParsingNotReadableFilesThrowsException",
];

#[test]
fn every_upstream_test_is_here() {
    let mut here: Vec<&str> = REPLAYED.iter().chain(BUILT).copied().collect();
    here.sort_unstable();
    assert_eq!(replay::names(recording()), here);
}

#[test]
fn testCanParseVeryLongValue() -> TestResult {
    let long_string_with_spaces = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ".repeat(20000);
    let tricky_val = Value::Array(php_array!["x" => long_string_with_spaces.as_str()]);

    let yaml_string = symfony_yaml::dump(&tricky_val, 2, 4, Flags::NONE)?;
    let array_from_yaml = Parser::new().parse(&yaml_string, Flags::NONE)?;

    assert!(same(&tricky_val, &array_from_yaml));
    // the calls: Yaml::dump($trickyVal), then parse($yamlString)
    replay::replay_with(
        recording(),
        "testCanParseVeryLongValue",
        &|index| match index {
            0 => tricky_val.clone(),
            _ => Value::String(yaml_string.clone()),
        },
    )
}

#[test]
fn testParseHandlesLargeYamlHeaders() -> TestResult {
    let yaml = format!("%YAML:{}\nfoo: bar\n", "1".repeat(100_000));
    replay::replay_with(recording(), "testParseHandlesLargeYamlHeaders", &|_| {
        Value::from(yaml.as_str())
    })
}

#[test]
fn testParseHandlesLargeLeadingComments() -> TestResult {
    let yaml = format!("#{}\nfoo: bar\n", "comment".repeat(20000));
    replay::replay_with(recording(), "testParseHandlesLargeLeadingComments", &|_| {
        Value::from(yaml.as_str())
    })
}

#[test]
fn testParseHandlesLargeDocumentMarkers() -> TestResult {
    let yaml = format!("--- # {}\nfoo: bar\n...   ", "header".repeat(20000));
    replay::replay_with(recording(), "testParseHandlesLargeDocumentMarkers", &|_| {
        Value::from(yaml.as_str())
    })
}
