//! Port of tests/Schema/Collections/UnqualifiedNamedObjectSetTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; `assertSame()` on the anonymous objects
//! compares their name and value.
//!
//! Upstream: doctrine/dbal tests/Schema/Collections/UnqualifiedNamedObjectSetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]
#![allow(clippy::similar_names, reason = "the upstream variable names")]

use doctrine_dbal::Exception;
use doctrine_dbal::schema::name::UnqualifiedName;
use doctrine_dbal::schema::{CollectionError, NamedObject, UnqualifiedNamedObjectSet};

#[derive(Debug, Clone, PartialEq)]
struct Object {
    name: UnqualifiedName,
    value: i64,
}

impl NamedObject for Object {
    fn named_object_name(&self) -> Result<UnqualifiedName, Exception> {
        Ok(self.name.clone())
    }
}

fn create_object(name: &str, value: i64) -> Object {
    Object {
        name: UnqualifiedName::unquoted(name).unwrap(),
        value,
    }
}

fn set(objects: &[&Object]) -> UnqualifiedNamedObjectSet<Object> {
    UnqualifiedNamedObjectSet::new(objects.iter().map(|o| (*o).clone())).unwrap()
}

fn list(set: &UnqualifiedNamedObjectSet<Object>) -> Vec<Object> {
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
    let set = UnqualifiedNamedObjectSet::<Object>::default();
    assert!(set.is_empty());
    assert_eq!(set.to_list(), [] as [&Object; 0]);
}

#[test]
fn testInstantiationWithArguments() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let set = set(&[&object1, &object2]);
    assert_eq!(list(&set), [object1, object2]);
}

#[test]
fn testAdd() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object1]);
    set.add(object2.clone()).unwrap();
    assert_eq!(list(&set), [object1, object2]);
}

#[test]
fn testAddExistingObject() {
    let object = create_object("object", 1);
    let mut set = set(&[&object]);
    assert_already_exists(set.add(object));
}

#[test]
fn testRemove() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object1, &object2]);
    set.remove(&object1.name).unwrap();
    assert_eq!(list(&set), [object2]);
}

#[test]
fn testRemoveNonExistingObject() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object1]);
    assert_does_not_exist(set.remove(&object2.name));
}

#[test]
fn testGetExistingObject() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let set = set(&[&object1, &object2]);
    assert_eq!(set.get(&object1.name), Some(&object1));
    assert_eq!(set.get(&object2.name), Some(&object2));
}

#[test]
fn testGetNonExistingObject() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let set = set(&[&object1]);
    assert!(set.get(&object2.name).is_none());
}

#[test]
fn testModifyObjectWithoutRenaming() {
    let object11 = create_object("object1", 11);
    let object12 = create_object("object1", 12);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object11, &object2]);
    set.modify(&object11.name, |_| Ok(object12.clone()))
        .unwrap();
    assert_eq!(list(&set), [object12, object2]);
}

#[test]
fn testModifyObjectWithRenaming() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let object3 = create_object("object3", 3);
    let mut set = set(&[&object1, &object2]);
    set.modify(&object1.name, |_| Ok(object3.clone())).unwrap();
    assert!(set.get(&object1.name).is_none());
    assert_eq!(set.get(&object3.name), Some(&object3));
    assert_eq!(list(&set), [object3, object2]);
}

#[test]
fn testModifyNonExistingObject() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object1]);
    assert_does_not_exist(set.modify(&object2.name, |object| Ok(object.clone())));
}

#[test]
fn testRenameToExistingName() {
    let object1 = create_object("object1", 1);
    let object2 = create_object("object2", 2);
    let mut set = set(&[&object1, &object2]);
    assert_already_exists(set.modify(&object1.name, |_| Ok(object2.clone())));
}
