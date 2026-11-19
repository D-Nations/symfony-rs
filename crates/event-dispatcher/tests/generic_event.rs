//! GenericEventTest.php at symfony/event-dispatcher
//! 7458da64220376b2e0dc2d8451bf43382c1ad297. Names and assertions are
//! PHPUnit's; `\stdClass` is a unit struct compared by address.
//!
//! Upstream: symfony/event-dispatcher Tests/GenericEventTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use symfony_event_dispatcher::GenericEvent;

#[derive(Debug, PartialEq)]
struct StdClass;

type Subject = Arc<StdClass>;

fn set_up() -> (GenericEvent<Subject, &'static str>, Subject) {
    let subject = Arc::new(StdClass);
    let event = GenericEvent::new(subject.clone(), [("name".to_owned(), "Event")]);
    (event, subject)
}

fn args(pairs: &[(&str, &'static str)]) -> Vec<(String, &'static str)> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

#[test]
fn testConstruct() {
    let (event, subject) = set_up();
    assert_eq!(
        event,
        GenericEvent::new(subject, [("name".to_owned(), "Event")])
    );
}

/// Tests Event->getArgs().
#[test]
fn testGetArguments() {
    let (event, _) = set_up();
    // test getting all
    assert_eq!(args(&[("name", "Event")]), event.get_arguments());
}

#[test]
fn testSetArguments() {
    let (mut event, _) = set_up();
    let result: *const _ = event.set_arguments(args(&[("foo", "bar")]));
    assert_eq!(args(&[("foo", "bar")]), event.get_arguments());
    assert!(std::ptr::eq(&event, result));
}

#[test]
fn testSetArgument() {
    let (mut event, _) = set_up();
    let result = event.set_argument("foo2", "bar2").clone();
    assert_eq!(
        args(&[("name", "Event"), ("foo2", "bar2")]),
        event.get_arguments()
    );
    assert_eq!(event, result);
}

#[test]
fn testGetArgument() {
    let (event, _) = set_up();
    // test getting key
    assert_eq!(Ok(&"Event"), event.get_argument("name"));
}

#[test]
fn testGetArgException() {
    let (event, _) = set_up();
    let error = event.get_argument("nameNotExist").err();
    assert_eq!(
        error.map(|e| e.message),
        Some("Argument \"nameNotExist\" not found.".to_owned())
    );
}

#[test]
fn testOffsetGet() {
    let (event, _) = set_up();
    // test getting key
    assert_eq!(Ok(&"Event"), event.offset_get("name"));

    // test getting invalid arg
    assert!(event.offset_get("nameNotExist").is_err());
}

#[test]
fn testOffsetSet() {
    let (mut event, _) = set_up();
    event.offset_set("foo2", "bar2");
    assert_eq!(
        args(&[("name", "Event"), ("foo2", "bar2")]),
        event.get_arguments()
    );
}

#[test]
fn testOffsetUnset() {
    let (mut event, _) = set_up();
    event.offset_unset("name");
    assert_eq!(args(&[]), event.get_arguments());
}

#[test]
fn testOffsetIsset() {
    let (event, _) = set_up();
    assert!(event.offset_exists("name"));
    assert!(!event.offset_exists("nameNotExist"));
}

#[test]
fn testHasArgument() {
    let (event, _) = set_up();
    assert!(event.has_argument("name"));
    assert!(!event.has_argument("nameNotExist"));
}

#[test]
fn testGetSubject() {
    let (event, subject) = set_up();
    assert!(Arc::ptr_eq(&subject, event.get_subject()));
}

#[test]
fn testHasIterator() {
    let (event, _) = set_up();
    let mut data = Vec::new();
    for (key, value) in &event {
        data.push((key.to_owned(), *value));
    }
    assert_eq!(args(&[("name", "Event")]), data);
}

#[test]
fn an_existing_key_keeps_its_place() {
    let (mut event, _) = set_up();
    event.set_argument("foo", "1").set_argument("name", "Other");
    assert_eq!(
        args(&[("name", "Other"), ("foo", "1")]),
        event.get_arguments()
    );
}
