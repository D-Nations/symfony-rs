//! MergeOperationTest and its seven inherited AbstractOperationTestCase tests.
//! Symfony tests copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Tests/Catalogue/MergeOperationTest.php Tests/Catalogue/AbstractOperationTestCase.php
#![allow(non_snake_case)]
mod common;
use common::{TestResult, catalogue, messages, same_catalogue};
use php_value::{PhpArray, Value};
use symfony_translation::catalogue::{Batch, MergeOperation as Operation};
use symfony_translation::{Class, MessageCatalogue};
const MERGE: bool = true;
const ALL: &[(&str, &str)] = &[("a", "old_a"), ("b", "old_b"), ("c", "new_c")];
const OBSOLETE: &[(&str, &str)] = &[];
const METADATA_MESSAGES: &[(&str, &str)] = &[("a", "old_a"), ("b", "old_b"), ("c", "new_c")];

#[test]
fn testGetEmptyDomains() -> TestResult {
    assert_eq!(
        Operation::new(catalogue(&[]), catalogue(&[]))?.get_domains(),
        PhpArray::new()
    );
    Ok(())
}
#[test]
fn testGetMergedDomains() -> TestResult {
    assert_eq!(
        Operation::new(
            catalogue(&[("a", messages(&[])), ("b", messages(&[]))]),
            catalogue(&[("b", messages(&[])), ("c", messages(&[]))])
        )?
        .get_domains(),
        PhpArray::list(["a", "b", "c"].map(Value::string))
    );
    Ok(())
}
#[test]
fn testGetMessagesFromUnknownDomain() -> TestResult {
    let error = Operation::new(catalogue(&[]), catalogue(&[]))?
        .get_messages(b"domain")
        .err()
        .ok_or("expected error")?;
    assert!(error.class().is_invalid_argument());
    assert_eq!(error.get_message(), b"Invalid domain: \"domain\".");
    Ok(())
}
#[test]
fn testGetEmptyMessages() -> TestResult {
    assert_eq!(
        Operation::new(catalogue(&[("a", messages(&[]))]), catalogue(&[]))?.get_messages(b"a")?,
        messages(&[])
    );
    Ok(())
}
#[test]
fn testGetEmptyResult() -> TestResult {
    same_catalogue(
        &catalogue(&[]),
        &Operation::new(catalogue(&[]), catalogue(&[]))?.get_result(),
    );
    Ok(())
}
#[test]
fn testSourceAndTargetWithDifferentLocales() -> TestResult {
    let error = Operation::new(catalogue(&[]), MessageCatalogue::new("fr", PhpArray::new()))
        .err()
        .ok_or("expected error")?;
    assert_eq!(error.class(), &Class::Logic);
    assert_eq!(
        error.get_message(),
        b"Operated catalogues must belong to the same locale."
    );
    Ok(())
}
#[test]
fn testMovingMessagesToIntlDomainsKeepsCatalogueMetadata() -> TestResult {
    let target = catalogue(&[("messages", messages(&[("foo", "bar")]))]);
    target.set_catalogue_metadata(b"foo", Value::string("bar"), b"messages");
    let mut operation = Operation::new(catalogue(&[]), target)?;
    operation.move_messages_to_intl_domains_if_possible(Batch::All, true)?;
    assert_eq!(
        operation
            .get_result()
            .get_catalogue_metadata(b"", b"messages+intl-icu"),
        Value::Array(messages(&[("foo", "bar")]))
    );
    Ok(())
}
#[test]
fn testGetMessagesFromSingleDomain() -> TestResult {
    let mut operation = Operation::new(
        catalogue(&[("messages", messages(&[("a", "old_a"), ("b", "old_b")]))]),
        catalogue(&[("messages", messages(&[("a", "new_a"), ("c", "new_c")]))]),
    )?;
    assert_eq!(operation.get_messages(b"messages")?, messages(ALL));
    assert_eq!(
        operation.get_new_messages(b"messages")?,
        messages(&[("c", "new_c")])
    );
    assert_eq!(
        operation.get_obsolete_messages(b"messages")?,
        messages(OBSOLETE)
    );
    Ok(())
}
#[test]
fn testGetResultFromSingleDomain() -> TestResult {
    same_catalogue(
        &catalogue(&[("messages", messages(ALL))]),
        &Operation::new(
            catalogue(&[("messages", messages(&[("a", "old_a"), ("b", "old_b")]))]),
            catalogue(&[("messages", messages(&[("a", "new_a"), ("c", "new_c")]))]),
        )?
        .get_result(),
    );
    Ok(())
}
#[test]
fn testGetResultWithMetadata() -> TestResult {
    check_metadata("messages")
}
#[test]
fn testGetResultWithMetadataFromIntlDomain() -> TestResult {
    check_metadata("messages+intl-icu")
}
fn check_metadata(domain: &str) -> TestResult {
    let left = catalogue(&[(domain, messages(&[("a", "old_a"), ("b", "old_b")]))]);
    left.set_metadata(b"a", Value::string("foo"), domain.as_bytes());
    left.set_metadata(b"b", Value::string("bar"), domain.as_bytes());
    let right = catalogue(&[(domain, messages(&[("b", "new_b"), ("c", "new_c")]))]);
    right.set_metadata(b"b", Value::string("baz"), domain.as_bytes());
    right.set_metadata(b"c", Value::string("qux"), domain.as_bytes());
    let expected = catalogue(&[(domain, messages(METADATA_MESSAGES))]);
    if MERGE {
        expected.set_metadata(b"a", Value::string("foo"), domain.as_bytes());
    }
    expected.set_metadata(b"b", Value::string("bar"), domain.as_bytes());
    expected.set_metadata(b"c", Value::string("qux"), domain.as_bytes());
    same_catalogue(&expected, &Operation::new(left, right)?.get_result());
    Ok(())
}

#[test]
fn testGetResultFromIntlDomain() -> TestResult {
    same_catalogue(
        &catalogue(&[
            ("messages", messages(&[("b", "old_b")])),
            (
                "messages+intl-icu",
                messages(&[("d", "old_d"), ("c", "new_c"), ("a", "new_a")]),
            ),
        ]),
        &Operation::new(
            catalogue(&[
                ("messages", messages(&[("a", "old_a"), ("b", "old_b")])),
                ("messages+intl-icu", messages(&[("d", "old_d")])),
            ]),
            catalogue(&[(
                "messages+intl-icu",
                messages(&[("a", "new_a"), ("c", "new_c")]),
            )]),
        )?
        .get_result(),
    );
    Ok(())
}
