//! Port of tests/Types/TypeRegistryTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/TypeRegistryTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use doctrine_dbal::types::{
    BinaryType, BlobType, StringType, TextType, Type, TypeRegistry, TypesException,
};

const TEST_TYPE_NAME: &str = "test";
const OTHER_TEST_TYPE_NAME: &str = "other";

struct Fixture {
    registry: TypeRegistry,
    test_type: Arc<dyn Type>,
    other_test_type: Arc<dyn Type>,
}

fn set_up() -> Fixture {
    let test_type: Arc<dyn Type> = Arc::new(BlobType);
    let other_test_type: Arc<dyn Type> = Arc::new(BinaryType);

    let registry = TypeRegistry::with_instances([
        (TEST_TYPE_NAME.to_owned(), Arc::clone(&test_type)),
        (
            OTHER_TEST_TYPE_NAME.to_owned(),
            Arc::clone(&other_test_type),
        ),
    ])
    .unwrap();

    Fixture {
        registry,
        test_type,
        other_test_type,
    }
}

fn text_type() -> Arc<dyn Type> {
    Arc::new(TextType)
}

#[test]
fn testGet() {
    let f = set_up();
    assert!(Arc::ptr_eq(
        &f.test_type,
        &f.registry.get(TEST_TYPE_NAME).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &f.other_test_type,
        &f.registry.get(OTHER_TEST_TYPE_NAME).unwrap()
    ));

    assert!(f.registry.get("unknown").is_err());
}

#[test]
fn testGetReturnsSameInstances() {
    let f = set_up();
    assert!(Arc::ptr_eq(
        &f.registry.get(TEST_TYPE_NAME).unwrap(),
        &f.registry.get(TEST_TYPE_NAME).unwrap()
    ));
}

#[test]
fn testLookupName() {
    let f = set_up();
    assert_eq!(
        f.registry.lookup_name(&f.test_type).unwrap(),
        TEST_TYPE_NAME
    );
    assert_eq!(
        f.registry.lookup_name(&f.other_test_type).unwrap(),
        OTHER_TEST_TYPE_NAME
    );

    assert!(matches!(
        f.registry.lookup_name(&text_type()),
        Err(TypesException::TypeNotRegistered(_))
    ));
}

#[test]
fn testHas() {
    let f = set_up();
    assert!(f.registry.has(TEST_TYPE_NAME));
    assert!(f.registry.has(OTHER_TEST_TYPE_NAME));
    assert!(!f.registry.has("unknown"));
}

#[test]
fn testRegister() {
    let f = set_up();
    let new_type = text_type();

    f.registry.register("some", Arc::clone(&new_type)).unwrap();

    assert!(f.registry.has("some"));
    assert!(Arc::ptr_eq(&new_type, &f.registry.get("some").unwrap()));
}

#[test]
fn testRegisterWithAlreadyRegisteredName() {
    let f = set_up();
    f.registry.register("some", text_type()).unwrap();

    assert!(f.registry.register("some", text_type()).is_err());
}

#[test]
fn testRegisterWithAlreadyRegisteredInstance() {
    let f = set_up();
    let new_type = text_type();

    f.registry.register("type1", Arc::clone(&new_type)).unwrap();
    assert!(f.registry.register("type2", new_type).is_err());
}

#[test]
fn testConstructorWithDuplicateInstance() {
    let new_type = text_type();

    assert!(
        TypeRegistry::with_instances([
            ("a".to_owned(), Arc::clone(&new_type)),
            ("b".to_owned(), new_type),
        ])
        .is_err()
    );
}

#[test]
fn testOverride() {
    let f = set_up();
    let base_type = text_type();
    let override_type: Arc<dyn Type> = Arc::new(StringType);

    f.registry.register("some", base_type).unwrap();
    f.registry
        .override_type("some", Arc::clone(&override_type))
        .unwrap();

    assert!(Arc::ptr_eq(
        &override_type,
        &f.registry.get("some").unwrap()
    ));
}

#[test]
fn testOverrideAllowsExistingInstance() {
    let f = set_up();
    let type_ = text_type();

    f.registry.register("some", Arc::clone(&type_)).unwrap();
    f.registry
        .override_type("some", Arc::clone(&type_))
        .unwrap();

    assert!(Arc::ptr_eq(&type_, &f.registry.get("some").unwrap()));
}

#[test]
fn testOverrideWithUnknownType() {
    let f = set_up();
    assert!(f.registry.override_type("unknown", text_type()).is_err());
}

#[test]
fn testOverrideWithAlreadyRegisteredInstance() {
    let f = set_up();
    let new_type = text_type();

    f.registry.register("first", Arc::clone(&new_type)).unwrap();
    f.registry.register("second", Arc::new(StringType)).unwrap();

    assert!(f.registry.override_type("second", new_type).is_err());
}

#[test]
fn testGetMap() {
    let f = set_up();
    let registered_types = f.registry.get_map();

    assert_eq!(registered_types.len(), 2);
    let find = |name: &str| {
        registered_types
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| Arc::clone(t))
    };
    let test = find(TEST_TYPE_NAME).expect("test registered");
    let other = find(OTHER_TEST_TYPE_NAME).expect("other registered");
    assert!(Arc::ptr_eq(&f.test_type, &test));
    assert!(Arc::ptr_eq(&f.other_test_type, &other));
}
