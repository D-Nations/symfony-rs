//! ControllerResolverTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's; the test methods are in
//! `common/controller_resolver_test_class.rs`, shared with the test
//! classes that extend this one.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ControllerResolverTest.php

#![allow(non_snake_case)]

#[path = "common/controller_resolver_test_class.rs"]
mod test_class;

use std::sync::Arc;

use psr_log::LoggerInterface;
use symfony_http_kernel::controller_resolver::{ClassRegistry, ControllerResolver};

/// `ControllerResolverTest` itself.
struct ControllerResolverTest;

impl test_class::TestClass for ControllerResolverTest {
    const UNDEFINED_CONTROLLER_COUNT: usize = 14;

    fn createControllerResolver(
        logger: Option<Arc<dyn LoggerInterface>>,
        program: Arc<ClassRegistry>,
    ) -> ControllerResolver {
        ControllerResolver::new(program, logger)
    }
}

#[test]
fn testGetControllerWithoutControllerParameter() {
    test_class::testGetControllerWithoutControllerParameter::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithLambda() {
    test_class::testGetControllerWithLambda::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithObjectAndInvokeMethod() {
    test_class::testGetControllerWithObjectAndInvokeMethod::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithObjectAndMethod() {
    test_class::testGetControllerWithObjectAndMethod::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithClassAndMethodAsArray() {
    test_class::testGetControllerWithClassAndMethodAsArray::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithClassAndMethodAsString() {
    test_class::testGetControllerWithClassAndMethodAsString::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithInvokableClass() {
    test_class::testGetControllerWithInvokableClass::<ControllerResolverTest>();
}

#[test]
fn testGetControllerOnObjectWithoutInvokeMethod() {
    test_class::testGetControllerOnObjectWithoutInvokeMethod::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithFunction() {
    test_class::testGetControllerWithFunction::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithClosure() {
    test_class::testGetControllerWithClosure::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithStaticController() {
    test_class::testGetControllerWithStaticController::<ControllerResolverTest>();
}

#[test]
fn testGetControllerWithUndefinedController() {
    test_class::testGetControllerWithUndefinedController::<ControllerResolverTest>();
}

#[test]
fn testAllowedControllerTypes() {
    test_class::testAllowedControllerTypes::<ControllerResolverTest>();
}

#[test]
fn testAllowedControllerAttributes() {
    test_class::testAllowedControllerAttributes::<ControllerResolverTest>();
}

#[test]
fn testAllowedAsControllerAttribute() {
    test_class::testAllowedAsControllerAttribute::<ControllerResolverTest>();
}
