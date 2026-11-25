//! Tests/Argument/TaggedIteratorArgumentTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The data-provider rows with a default method use the
//! deprecated constructor shape ([`IteratorArgument::tagged_legacy`]), as
//! upstream's do through their argument types.
//!
//! Upstream: symfony/dependency-injection Tests/Argument/TaggedIteratorArgumentTest.php
#![allow(non_snake_case)]

use symfony_deprecation_contracts::collect;
use symfony_dependency_injection::argument::IteratorArgument;

#[test]
fn testWithTagOnly() {
    let a = IteratorArgument::tagged("foo", None, false, Vec::new(), true);
    assert_eq!(Some("foo".to_string()), a.tag());
    assert_eq!(None, a.index_attribute());
    assert_eq!(None, a.default_index_method(false));
    assert!(!a.needs_indexes());
    assert_eq!(None, a.default_priority_method(false));
}

#[test]
fn testOnlyTagWithNeedsIndexes() {
    let a = IteratorArgument::tagged("foo", None, true, Vec::new(), true);
    assert_eq!(Some("foo".to_string()), a.tag());
    assert_eq!(Some("foo".to_string()), a.index_attribute());
    assert_eq!(Some("getDefaultFooName".to_string()), a.default_index_method(false));
    assert_eq!(Some("getDefaultFooPriority".to_string()), a.default_priority_method(false));
}

#[test]
fn testOnlyTagWithNeedsIndexesAndDotTag() {
    let a = IteratorArgument::tagged("foo.bar", None, true, Vec::new(), true);
    assert_eq!(Some("foo.bar".to_string()), a.tag());
    assert_eq!(Some("bar".to_string()), a.index_attribute());
    assert_eq!(Some("getDefaultBarName".to_string()), a.default_index_method(false));
    assert_eq!(Some("getDefaultBarPriority".to_string()), a.default_priority_method(false));
}

#[test]
fn testOnlyTagWithNeedsIndexesAndDotsTag() {
    let a = IteratorArgument::tagged("foo.bar.baz.qux", None, true, Vec::new(), true);
    assert_eq!(Some("foo.bar.baz.qux".to_string()), a.tag());
    assert_eq!(Some("qux".to_string()), a.index_attribute());
    assert_eq!(Some("getDefaultQuxName".to_string()), a.default_index_method(false));
    assert_eq!(Some("getDefaultQuxPriority".to_string()), a.default_priority_method(false));
}

const ROWS: [(Option<&str>, Option<&str>, Option<&str>, &str); 6] = [
    (None, None, None, ""),
    (Some("bar"), None, Some("getDefaultBar"), ""),
    (None, Some("getBaz"), Some("getBaz"), "fixed"),
    (Some("bar"), Some("getBaz"), Some("getBaz"), "fixed"),
    (Some("bar_baz"), None, Some("getDefaultBarBaz"), ""),
    (Some("bar-baz-qux"), None, Some("getDefaultBarBazQux"), ""),
];

fn expected(prefix: Option<&str>, fixed: &str, suffix: &str) -> Option<String> {
    prefix.map(|p| if fixed.is_empty() { format!("{p}{suffix}") } else { p.to_string() })
}

#[test]
fn testDefaultIndexMethod() {
    for (index_attribute, default_index_method, prefix, fixed) in ROWS {
        let (method, deprecations) = collect(|| {
            // `new TaggedIteratorArgument('foo', $indexAttribute, $defaultIndexMethod)`
            IteratorArgument::tagged_legacy("foo", index_attribute, default_index_method, false, None, Vec::new(), true)
                .default_index_method(true)
        });
        assert_eq!(expected(prefix, fixed, "Name"), method);
        assert_eq!(2, deprecations.len());
    }
}

#[test]
fn testDefaultPriorityIndexMethod() {
    for (index_attribute, default_priority_method, prefix, fixed) in ROWS {
        let (method, deprecations) = collect(|| {
            // `new TaggedIteratorArgument('foo', $indexAttribute, null, false, $defaultPriorityMethod)`
            IteratorArgument::tagged_legacy("foo", index_attribute, None, false, default_priority_method, Vec::new(), true)
                .default_priority_method(true)
        });
        assert_eq!(expected(prefix, fixed, "Priority"), method);
        assert_eq!(2, deprecations.len());
    }
}
