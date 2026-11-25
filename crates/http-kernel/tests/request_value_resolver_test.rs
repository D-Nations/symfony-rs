//! RequestValueResolverTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's; `assertSame()` on a request is handle
//! identity.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ArgumentResolver/RequestValueResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{Handle, Value};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    argument_resolver::ValueResolverInterface,
    controller::{REQUEST_CLASS, as_request, request_value},
    controller_metadata::ArgumentMetadata,
    exception::instance_of,
    value_resolver::RequestValueResolver,
};

fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

fn metadata(type_name: &str) -> ArgumentMetadata {
    ArgumentMetadata::simple("request", Some(type_name), false, false, Value::Null)
}

#[test]
fn testSameRequestReturned() {
    let resolver = RequestValueResolver;
    let expected_request = create("/");
    let actual_request = resolver
        .resolve(&expected_request, &metadata(REQUEST_CLASS))
        .unwrap();
    assert_eq!(1, actual_request.len());
    assert!(
        as_request(&actual_request[0])
            .unwrap()
            .same(&expected_request)
    );
}

#[test]
fn testRequestIsNotResolvedForRandomClass() {
    let resolver = RequestValueResolver;
    let expected_request = create("/");
    let actual_request = resolver
        .resolve(
            &expected_request,
            &metadata(
                "Symfony\\Component\\HttpKernel\\Tests\\Controller\\ArgumentResolver\\RequestValueResolverTest",
            ),
        )
        .unwrap();
    assert_eq!(0, actual_request.len());
}

#[test]
fn testExceptionThrownForRandomRequestClass() {
    let resolver = RequestValueResolver;
    let expected_request = create("/");
    let e = resolver
        .resolve(
            &expected_request,
            // `use Symfony\Component\BrowserKit\Request as RandomRequest`
            &metadata("Symfony\\Component\\BrowserKit\\Request"),
        )
        .unwrap_err();
    assert!(instance_of(
        e.as_ref(),
        "Symfony\\Component\\HttpKernel\\Exception\\NearMissValueResolverException"
    ));
}

#[test]
fn testResolveDefersWhenMatchingNamedAttributeExists() {
    let resolver = RequestValueResolver;
    let request = create("/");
    let other = create("/other");
    request.with_mut(|r| r.attributes_mut().set("request", request_value(&other)));

    assert_eq!(
        0,
        resolver
            .resolve(&request, &metadata(REQUEST_CLASS))
            .unwrap()
            .len()
    );
}

#[test]
fn testResolveIgnoresUnrelatedAttributeName() {
    let resolver = RequestValueResolver;
    let request = create("/");
    let other = create("/other");
    request.with_mut(|r| r.attributes_mut().set("foo", request_value(&other)));

    let values = resolver
        .resolve(&request, &metadata(REQUEST_CLASS))
        .unwrap();
    assert_eq!(1, values.len());
    assert!(as_request(&values[0]).unwrap().same(&request));
}
