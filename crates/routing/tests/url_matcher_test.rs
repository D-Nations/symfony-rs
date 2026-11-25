//! Port of Symfony\Component\Routing\Tests\Matcher\UrlMatcherTest.php on
//! [`UrlMatcher`]: the test bodies are in `url_matcher/mod.rs`, shared
//! with `RedirectableUrlMatcherTest`, which extends this class upstream.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Tests/Matcher/UrlMatcherTest.php
#![allow(non_snake_case)]

mod url_matcher;

use php_value::PhpArray;
use symfony_routing::{
    exception::Error, request_context::RequestContext, route_collection::RouteCollection,
    url_matcher::UrlMatcher,
};
use url_matcher::{Matcher, TestResult};

impl Matcher for UrlMatcher {
    fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Error> {
        UrlMatcher::match_path(self, pathinfo)
    }
}

/// `getUrlMatcher($routes, $context)`.
fn get_url_matcher(routes: RouteCollection, context: Option<RequestContext>) -> Box<dyn Matcher> {
    Box::new(UrlMatcher::new(routes, context.unwrap_or_default()))
}

#[test]
fn testZero() -> TestResult {
    url_matcher::testZero(get_url_matcher)
}

#[test]
fn testNoMethodSoAllowed() -> TestResult {
    url_matcher::testNoMethodSoAllowed(get_url_matcher)
}

#[test]
fn testMethodNotAllowed() -> TestResult {
    url_matcher::testMethodNotAllowed(get_url_matcher)
}

#[test]
fn testMethodNotAllowedOnRoot() -> TestResult {
    url_matcher::testMethodNotAllowedOnRoot(get_url_matcher)
}

#[test]
fn testHeadAllowedWhenRequirementContainsGet() -> TestResult {
    url_matcher::testHeadAllowedWhenRequirementContainsGet(get_url_matcher)
}

#[test]
fn testHeadAllowedWhenRequirementContainsHead() -> TestResult {
    url_matcher::testHeadAllowedWhenRequirementContainsHead(get_url_matcher)
}

#[test]
fn testMethodNotAllowedAggregatesAllowedMethods() -> TestResult {
    url_matcher::testMethodNotAllowedAggregatesAllowedMethods(get_url_matcher)
}

#[test]
fn testPatternMatchAndParameterReturn() -> TestResult {
    url_matcher::testPatternMatchAndParameterReturn(get_url_matcher)
}

#[test]
fn testDefaultsAreMerged() -> TestResult {
    url_matcher::testDefaultsAreMerged(get_url_matcher)
}

#[test]
fn testMethodIsIgnoredIfNoMethodGiven() -> TestResult {
    url_matcher::testMethodIsIgnoredIfNoMethodGiven(get_url_matcher)
}

#[test]
fn testRouteWithOptionalVariableAsFirstSegment() -> TestResult {
    url_matcher::testRouteWithOptionalVariableAsFirstSegment(get_url_matcher)
}

#[test]
fn testRouteWithOnlyOptionalVariables() -> TestResult {
    url_matcher::testRouteWithOnlyOptionalVariables(get_url_matcher)
}

#[test]
fn testMatchWithPrefixes() -> TestResult {
    url_matcher::testMatchWithPrefixes(get_url_matcher)
}

#[test]
fn testMatchWithDynamicPrefix() -> TestResult {
    url_matcher::testMatchWithDynamicPrefix(get_url_matcher)
}

#[test]
fn testMatchSpecialRouteName() -> TestResult {
    url_matcher::testMatchSpecialRouteName(get_url_matcher)
}

#[test]
fn testMatchImportantVariable() -> TestResult {
    url_matcher::testMatchImportantVariable(get_url_matcher)
}

#[test]
fn testShortPathDoesNotMatchImportantVariable() -> TestResult {
    url_matcher::testShortPathDoesNotMatchImportantVariable(get_url_matcher)
}

#[test]
fn testTrailingEncodedNewlineIsNotOverlooked() -> TestResult {
    url_matcher::testTrailingEncodedNewlineIsNotOverlooked(get_url_matcher)
}

#[test]
fn testMatchNonAlpha() -> TestResult {
    url_matcher::testMatchNonAlpha(get_url_matcher)
}

#[test]
fn testMatchWithDotMetacharacterInRequirements() -> TestResult {
    url_matcher::testMatchWithDotMetacharacterInRequirements(get_url_matcher)
}

#[test]
fn testMatchOverriddenRoute() -> TestResult {
    url_matcher::testMatchOverriddenRoute(get_url_matcher)
}

#[test]
fn testMatchRegression() -> TestResult {
    url_matcher::testMatchRegression(get_url_matcher)
}

#[test]
fn testMultipleParams() -> TestResult {
    url_matcher::testMultipleParams(get_url_matcher)
}

#[test]
fn testDefaultRequirementForOptionalVariables() -> TestResult {
    url_matcher::testDefaultRequirementForOptionalVariables(get_url_matcher)
}

#[test]
fn testMatchingIsEager() -> TestResult {
    url_matcher::testMatchingIsEager(get_url_matcher)
}

#[test]
fn testAdjacentVariables() -> TestResult {
    url_matcher::testAdjacentVariables(get_url_matcher)
}

#[test]
fn testOptionalVariableWithNoRealSeparator() -> TestResult {
    url_matcher::testOptionalVariableWithNoRealSeparator(get_url_matcher)
}

#[test]
fn testRequiredVariableWithNoRealSeparator() -> TestResult {
    url_matcher::testRequiredVariableWithNoRealSeparator(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariable() -> TestResult {
    url_matcher::testDefaultRequirementOfVariable(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariableDisallowsSlash() -> TestResult {
    url_matcher::testDefaultRequirementOfVariableDisallowsSlash(get_url_matcher)
}

#[test]
fn testDefaultRequirementOfVariableDisallowsNextSeparator() -> TestResult {
    url_matcher::testDefaultRequirementOfVariableDisallowsNextSeparator(get_url_matcher)
}

#[test]
fn testMissingTrailingSlash() -> TestResult {
    url_matcher::testMissingTrailingSlash(get_url_matcher)
}

#[test]
fn testExtraTrailingSlash() -> TestResult {
    url_matcher::testExtraTrailingSlash(get_url_matcher)
}

#[test]
fn testMissingTrailingSlashForNonSafeMethod() -> TestResult {
    url_matcher::testMissingTrailingSlashForNonSafeMethod(get_url_matcher)
}

#[test]
fn testExtraTrailingSlashForNonSafeMethod() -> TestResult {
    url_matcher::testExtraTrailingSlashForNonSafeMethod(get_url_matcher)
}

#[test]
fn testSchemeRequirement() -> TestResult {
    url_matcher::testSchemeRequirement(get_url_matcher)
}

#[test]
fn testSchemeRequirementForNonSafeMethod() -> TestResult {
    url_matcher::testSchemeRequirementForNonSafeMethod(get_url_matcher)
}

#[test]
fn testSamePathWithDifferentScheme() -> TestResult {
    url_matcher::testSamePathWithDifferentScheme(get_url_matcher)
}

#[test]
fn testDecodeOnce() -> TestResult {
    url_matcher::testDecodeOnce(get_url_matcher)
}

#[test]
fn testCannotRelyOnPrefix() -> TestResult {
    url_matcher::testCannotRelyOnPrefix(get_url_matcher)
}

#[test]
fn testWithHost() -> TestResult {
    url_matcher::testWithHost(get_url_matcher)
}

#[test]
fn testWithHostOnRouteCollection() -> TestResult {
    url_matcher::testWithHostOnRouteCollection(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHosts() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHosts(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsInReverse(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsAndVariable() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsAndVariable(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithHostsAndVariableInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithHostsAndVariableInReverse(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testVariationInTrailingSlashWithMethodsInReverse() -> TestResult {
    url_matcher::testVariationInTrailingSlashWithMethodsInReverse(get_url_matcher)
}

#[test]
fn testVariableVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testVariableVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testVariableVariationInTrailingSlashWithMethodsInReverse() -> TestResult {
    url_matcher::testVariableVariationInTrailingSlashWithMethodsInReverse(get_url_matcher)
}

#[test]
fn testMixOfStaticAndVariableVariationInTrailingSlashWithHosts() -> TestResult {
    url_matcher::testMixOfStaticAndVariableVariationInTrailingSlashWithHosts(get_url_matcher)
}

#[test]
fn testMixOfStaticAndVariableVariationInTrailingSlashWithMethods() -> TestResult {
    url_matcher::testMixOfStaticAndVariableVariationInTrailingSlashWithMethods(get_url_matcher)
}

#[test]
fn testWithOutHostHostDoesNotMatch() -> TestResult {
    url_matcher::testWithOutHostHostDoesNotMatch(get_url_matcher)
}

#[test]
fn testPathIsCaseSensitive() -> TestResult {
    url_matcher::testPathIsCaseSensitive(get_url_matcher)
}

#[test]
fn testHostIsCaseInsensitive() -> TestResult {
    url_matcher::testHostIsCaseInsensitive(get_url_matcher)
}

#[test]
fn testNoConfiguration() -> TestResult {
    url_matcher::testNoConfiguration(get_url_matcher)
}

#[test]
fn testNestedCollections() -> TestResult {
    url_matcher::testNestedCollections(get_url_matcher)
}

#[test]
fn testSchemeAndMethodMismatch() -> TestResult {
    url_matcher::testSchemeAndMethodMismatch(get_url_matcher)
}

#[test]
fn testUnicodeRoute() -> TestResult {
    url_matcher::testUnicodeRoute(get_url_matcher)
}

#[test]
fn testRequirementWithCapturingGroup() -> TestResult {
    url_matcher::testRequirementWithCapturingGroup(get_url_matcher)
}

#[test]
fn testDotAllWithCatchAll() -> TestResult {
    url_matcher::testDotAllWithCatchAll(get_url_matcher)
}

#[test]
fn testHostPattern() -> TestResult {
    url_matcher::testHostPattern(get_url_matcher)
}

#[test]
fn testUtf8Prefix() -> TestResult {
    url_matcher::testUtf8Prefix(get_url_matcher)
}

#[test]
fn testUtf8AndMethodMatching() -> TestResult {
    url_matcher::testUtf8AndMethodMatching(get_url_matcher)
}

#[test]
fn testHostWithDot() -> TestResult {
    url_matcher::testHostWithDot(get_url_matcher)
}

#[test]
fn testHostVariableDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testHostVariableWithPermissiveRequirementDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableWithPermissiveRequirementDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testHostVariableWithPermissiveRequirementMatchesTheHostOnly() -> TestResult {
    url_matcher::testHostVariableWithPermissiveRequirementMatchesTheHostOnly(get_url_matcher)
}

#[test]
fn testHostVariableWithNamedGroupInRequirementDoesNotMatchThePath() -> TestResult {
    url_matcher::testHostVariableWithNamedGroupInRequirementDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testStaticRouteWithHostVariableDoesNotMatchThePath() -> TestResult {
    url_matcher::testStaticRouteWithHostVariableDoesNotMatchThePath(get_url_matcher)
}

#[test]
fn testSlashVariant() -> TestResult {
    url_matcher::testSlashVariant(get_url_matcher)
}

#[test]
fn testSlashVariant2() -> TestResult {
    url_matcher::testSlashVariant2(get_url_matcher)
}

#[test]
fn testSlashWithVerb() -> TestResult {
    url_matcher::testSlashWithVerb(get_url_matcher)
}

#[test]
fn testSlashAndVerbPrecedence() -> TestResult {
    url_matcher::testSlashAndVerbPrecedence(get_url_matcher)
}

#[test]
fn testGreedyTrailingRequirement() -> TestResult {
    url_matcher::testGreedyTrailingRequirement(get_url_matcher)
}

#[test]
fn testTrailingRequirementWithDefault() -> TestResult {
    url_matcher::testTrailingRequirementWithDefault(get_url_matcher)
}

#[test]
fn testTrailingRequirementWithDefaultA() -> TestResult {
    url_matcher::testTrailingRequirementWithDefaultA(get_url_matcher)
}

#[test]
fn testTrailingRequirementWithDefaultB() -> TestResult {
    url_matcher::testTrailingRequirementWithDefaultB(get_url_matcher)
}

#[test]
fn testRestrictiveTrailingRequirementWithStaticRouteAfter() -> TestResult {
    url_matcher::testRestrictiveTrailingRequirementWithStaticRouteAfter(get_url_matcher)
}

#[test]
fn testUtf8VarName() -> TestResult {
    url_matcher::testUtf8VarName(get_url_matcher)
}

#[test]
fn testParameterWithRequirementWithDefault() -> TestResult {
    url_matcher::testParameterWithRequirementWithDefault(get_url_matcher)
}

#[test]
fn testMapping() -> TestResult {
    url_matcher::testMapping(get_url_matcher)
}

#[test]
fn testMappingwithAlias() -> TestResult {
    url_matcher::testMappingwithAlias(get_url_matcher)
}
