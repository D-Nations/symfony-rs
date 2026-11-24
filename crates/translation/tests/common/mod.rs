//! Catalogue test fixtures; no upstream behaviour.
//!
//! Upstream: none
#![allow(dead_code)]
use php_value::{Key, PhpArray, Value, vectors::phpunit_equals};
use symfony_translation::MessageCatalogue;
pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub fn messages(pairs: &[(&str, &str)]) -> PhpArray {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), Value::string(*v));
    }
    a
}
pub fn catalogue(pairs: &[(&str, PhpArray)]) -> MessageCatalogue {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), Value::Array(v.clone()));
    }
    MessageCatalogue::new("en", a)
}
pub fn same_catalogue(expected: &MessageCatalogue, actual: &MessageCatalogue) {
    assert_eq!(expected.get_locale(), actual.get_locale());
    let e = expected.raw();
    let a = actual.raw();
    for (e, a) in [(e.0, a.0), (e.1, a.1), (e.2, a.2)] {
        assert!(
            phpunit_equals(&Value::Array(e.clone()), &Value::Array(a.clone())),
            "expected {e:?}, got {a:?}"
        );
    }
    assert_eq!(expected.get_resources().len(), actual.get_resources().len());
    assert!(actual.get_fallback_catalogue().is_none());
}
