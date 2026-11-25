//! RequestStackTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's; `assertSame()` on requests is handle
//! identity.
//!
//! Upstream: symfony/http-foundation Tests/RequestStackTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{Handle, PhpString};
use symfony_http_foundation::{
    request::Request,
    request_stack::{RequestHandle, RequestStack},
};

fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

fn same(expected: &RequestHandle, actual: Option<RequestHandle>) -> bool {
    actual.is_some_and(|actual| expected.same(&actual))
}

#[test]
fn testConstruct() {
    let request = create("/foo");
    let request_stack = RequestStack::new([request.clone()]);
    assert!(same(&request, request_stack.get_current_request()));
}

#[test]
fn testGetCurrentRequest() {
    let request_stack = RequestStack::default();
    assert!(request_stack.get_current_request().is_none());

    let request = create("/foo");

    request_stack.push(request.clone());
    assert!(same(&request, request_stack.get_current_request()));

    assert!(same(&request, request_stack.pop()));
    assert!(request_stack.get_current_request().is_none());

    assert!(request_stack.pop().is_none());
}

#[test]
fn testGetMainRequest() {
    let request_stack = RequestStack::default();
    assert!(request_stack.get_main_request().is_none());

    let main_request = create("/foo");
    let sub_request = create("/bar");

    request_stack.push(main_request.clone());
    request_stack.push(sub_request);

    assert!(same(&main_request, request_stack.get_main_request()));
}

#[test]
fn testGetParentRequest() {
    let request_stack = RequestStack::default();
    assert!(request_stack.get_parent_request().is_none());

    let main_request = create("/foo");

    request_stack.push(main_request.clone());
    assert!(request_stack.get_parent_request().is_none());

    let first_sub_request = create("/bar");

    request_stack.push(first_sub_request.clone());
    assert!(same(&main_request, request_stack.get_parent_request()));

    let second_sub_request = create("/baz");

    request_stack.push(second_sub_request);
    assert!(same(&first_sub_request, request_stack.get_parent_request()));
}

#[test]
fn testResetRequestFormats() {
    Request::reset_statics();
    let request_stack = RequestStack::default();

    let request = Request::create("/foo").unwrap();
    request.set_format("foo", ["application/foo"]);

    assert_eq!(
        vec![PhpString::from("application/foo")],
        Request::get_mime_types(b"foo")
    );

    request_stack.reset_request_formats();

    assert_eq!(
        Request::get_mime_types(b"foo"),
        [] as [php_value::PhpString; 0]
    );
}
