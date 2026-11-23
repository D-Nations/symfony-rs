//! QtFileLoaderTest.php at symfony/translation v8.1.5. Names and
//! assertions are PHPUnit's; the fixtures are upstream's Tests/Fixtures,
//! copied to tests/fixtures. `getResources()` is compared as the
//! resources' class and `__toString()`.
//!
//! Upstream: symfony/translation Tests/Loader/QtFileLoaderTest.php
#![allow(non_snake_case)]

use php_value::{Key, PhpArray, Value};
use symfony_config::resource::{FileResource, ResourceInterface, file_resource};
use symfony_translation::{Class, LoaderInterface, QtFileLoader};

fn fixture(name: &str) -> Vec<u8> {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")).into_bytes()
}

fn load_err(resource: &[u8]) -> symfony_translation::Error {
    match QtFileLoader.load(resource, b"en", b"domain1") {
        Ok(c) => panic!("expected an exception, got {c:?}"),
        Err(e) => e,
    }
}

#[test]
fn testLoad() {
    let resource = fixture("resources.ts");
    let catalogue = QtFileLoader
        .load(&resource, b"en", b"resources")
        .expect("load");

    let mut want = PhpArray::new();
    for (k, v) in [("foo", "bar"), ("foo_bar", "foobar"), ("bar_foo", "barfoo")] {
        want.insert(Key::from_bytes(k.as_bytes()), Value::string(v));
    }
    assert_eq!(catalogue.all(Some(b"resources")), want);
    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    let resources = catalogue.get_resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].class(), file_resource::CLASS);
    assert_eq!(
        resources[0].to_string(),
        FileResource::new(&resource)
            .expect("FileResource")
            .to_string()
    );
}

#[test]
fn testLoadNonExistingResource() {
    assert_eq!(
        *load_err(&fixture("non-existing.ts")).class(),
        Class::NotFoundResource
    );
}

#[test]
fn testLoadNonLocalResource() {
    assert_eq!(
        *load_err(b"http://domain1.com/resources.ts").class(),
        Class::InvalidResource
    );
}

#[test]
fn testLoadInvalidResource() {
    assert_eq!(
        *load_err(&fixture("invalid-xml-resources.xlf")).class(),
        Class::InvalidResource
    );
}

#[test]
fn testLoadEmptyResource() {
    let resource = fixture("empty.xlf");
    let e = load_err(&resource);
    assert_eq!(*e.class(), Class::InvalidResource);
    let mut message = b"Unable to load \"".to_vec();
    message.extend_from_slice(&resource);
    message.extend_from_slice(b"\".");
    assert!(e.get_message().starts_with(&message));
}
