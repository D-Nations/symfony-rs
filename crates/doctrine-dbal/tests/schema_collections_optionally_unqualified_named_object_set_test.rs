//! Port of tests/Schema/Collections/OptionallyUnqualifiedNamedObjectSetTest.php
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; `assertSame()` on the anonymous objects
//! compares their name and value.
//!
//! Upstream: doctrine/dbal tests/Schema/Collections/OptionallyUnqualifiedNamedObjectSetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]
#![allow(clippy::similar_names, reason = "the upstream variable names")]

use doctrine_dbal::Exception;
use doctrine_dbal::schema::name::UnqualifiedName;
use doctrine_dbal::schema::{
    CollectionError, OptionallyNamedObject, OptionallyUnqualifiedNamedObjectSet,
};

#[derive(Debug, Clone, PartialEq)]
struct Object {
    name: Option<UnqualifiedName>,
    value: i64,
}

impl OptionallyNamedObject for Object {
    fn optional_object_name(&self) -> Result<Option<UnqualifiedName>, Exception> {
        Ok(self.name.clone())
    }
}

type Set = OptionallyUnqualifiedNamedObjectSet<Object>;

fn create_object(name: Option<&str>, value: i64) -> Object {
    Object {
        name: name.map(|name| UnqualifiedName::unquoted(name).unwrap()),
        value,
    }
}

fn u(name: &str) -> UnqualifiedName {
    UnqualifiedName::unquoted(name).unwrap()
}

fn set(objects: &[&Object]) -> Set {
    Set::new(objects.iter().map(|o| (*o).clone())).unwrap()
}

fn list(set: &Set) -> Vec<Object> {
    set.to_list().into_iter().cloned().collect()
}

fn assert_already_exists(result: Result<(), CollectionError>) {
    let e = result.unwrap_err().exception();
    assert!(
        e.is_instance_of("Doctrine\\DBAL\\Schema\\Collections\\Exception\\ObjectAlreadyExists"),
        "{e}"
    );
}

fn assert_does_not_exist(result: Result<(), CollectionError>) {
    let e = result.unwrap_err().exception();
    assert!(
        e.is_instance_of("Doctrine\\DBAL\\Schema\\Collections\\Exception\\ObjectDoesNotExist"),
        "{e}"
    );
}

#[test]
fn testInstantiationWithoutArguments() {
    let set = Set::default();
    assert!(set.is_empty());
    assert!(set.to_list().is_empty());
}

#[test]
fn testInstantiationWithArguments() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let set = set(&[&object1, &object2]);
    assert_eq!(list(&set), [object1, object2]);
}

#[test]
fn testAdd() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let mut set = set(&[&object1]);
    set.add(object2.clone()).unwrap();
    assert_eq!(list(&set), [object1, object2]);
}

#[test]
fn testAddExistingObject() {
    let object = create_object(Some("object"), 1);
    let mut set = set(&[&object]);
    assert_already_exists(set.add(object));
}

#[test]
fn testRemove() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let mut set = set(&[&object1, &object2]);
    set.remove(&u("object1")).unwrap();
    assert_eq!(list(&set), [object2]);
}

#[test]
fn testRemoveNonExistingObject() {
    let object1 = create_object(Some("object1"), 1);
    let mut set = set(&[&object1]);
    assert_does_not_exist(set.remove(&u("object2")));
}

#[test]
fn testGetExistingObject() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let object3 = create_object(Some("object3"), 3);
    let set = set(&[&object1, &object2, &object3]);
    assert_eq!(set.get(&u("object1")), Some(&object1));
    assert_eq!(set.get(&u("object3")), Some(&object3));
}

#[test]
fn testGetNonExistingObject() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let set = set(&[&object1, &object2]);
    assert!(set.get(&u("object3")).is_none());
}

#[test]
fn testModifyObject() {
    let object11 = create_object(Some("object1"), 11);
    let object12 = create_object(Some("object1"), 12);
    let object2 = create_object(Some("object2"), 2);
    let mut set = set(&[&object11, &object2]);
    set.modify(&u("object1"), |_| Ok(object12.clone())).unwrap();
    assert_eq!(list(&set), [object12, object2]);
}

#[test]
fn testModifyNonExistingObject() {
    let object1 = create_object(Some("object1"), 1);
    let mut set = set(&[&object1]);
    assert_does_not_exist(set.modify(&u("object2"), |object| Ok(object.clone())));
}

#[test]
fn testRenameToExistingName() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let object3 = create_object(Some("object3"), 3);
    let mut set = set(&[&object1, &object2, &object3]);
    assert_already_exists(set.modify(&u("object1"), |_| Ok(object3.clone())));
}

#[test]
fn testModifyingAnElementToANewNameMovesItUnderThatName() {
    let mut set = set(&[&create_object(Some("old_name"), 1)]);
    let renamed = create_object(Some("new_name"), 1);
    set.modify(&u("old_name"), |_| Ok(renamed.clone())).unwrap();
    assert_eq!(set.get(&u("new_name")), Some(&renamed));
    assert!(set.get(&u("old_name")).is_none());
}

#[test]
fn testRemoveKeepsOtherElementsAddressable() {
    let first = create_object(Some("first"), 1);
    let middle = create_object(Some("middle"), 2);
    let last = create_object(Some("last"), 3);
    let mut set = set(&[&first, &middle, &last]);
    set.remove(&u("middle")).unwrap();
    assert_eq!(set.get(&u("first")), Some(&first));
    assert!(set.get(&u("middle")).is_none());
    assert_eq!(set.get(&u("last")), Some(&last));
}

#[test]
fn testRenameToNullName() {
    let object1 = create_object(Some("object1"), 1);
    let object2 = create_object(None, 2);
    let object3 = create_object(Some("object3"), 3);
    let mut set = set(&[&object1, &object2, &object3]);
    set.modify(&u("object1"), |_| Ok(object2.clone())).unwrap();
    assert_eq!(list(&set), [object2.clone(), object2, object3]);
}
