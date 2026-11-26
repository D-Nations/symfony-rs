//! Session/Storage/Proxy/AbstractProxyTest.php at symfony/http-foundation
//! v8.1.11. Names and assertions are PHPUnit's; the anonymous subclass is
//! an `AbstractProxy` with no handler. Each test runs on a thread of its
//! own, as PHPUnit runs the session ones in a process of their own.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Proxy/AbstractProxyTest.php
#![allow(non_snake_case)]

#[path = "native_session/mod.rs"]
mod native_session;

use std::sync::Arc;

use native_session::{SavePath, TestResult};
use symfony_http_foundation::{
    php_session::{SessionHandler, session_id, session_name, session_start},
    session::storage::proxy::{
        abstract_proxy::AbstractProxy, session_handler_proxy::SessionHandlerProxy,
    },
};

/// `setUp()`: `new class extends AbstractProxy {}`.
fn set_up() -> AbstractProxy {
    AbstractProxy::default()
}

#[test]
fn testGetSaveHandlerName() {
    assert_eq!(None, set_up().get_save_handler_name());
}

#[test]
fn testIsSessionHandlerInterface() {
    assert!(!set_up().is_session_handler_interface());
    let sh = SessionHandlerProxy::new(Arc::new(SessionHandler));
    assert!(sh.is_session_handler_interface());
}

#[test]
fn testIsWrapper() {
    assert!(!set_up().is_wrapper());
}

#[test]
fn testIsActive() -> TestResult {
    let _save_path = SavePath::set_up();
    let proxy = set_up();
    assert!(!proxy.is_active());
    session_start()?;
    assert!(proxy.is_active());
    Ok(())
}

#[test]
fn testName() -> TestResult {
    let proxy = set_up();
    assert_eq!(session_name().to_string_lossy(), proxy.get_name());
    proxy.set_name("foo")?;
    assert_eq!("foo", proxy.get_name());
    assert_eq!(session_name().to_string_lossy(), proxy.get_name());
    Ok(())
}

#[test]
fn testNameException() -> TestResult {
    let _save_path = SavePath::set_up();
    let proxy = set_up();
    session_start()?;
    let error = proxy
        .set_name("foo")
        .err()
        .ok_or("expected a LogicException")?;
    assert!(error.is_a("LogicException"));
    Ok(())
}

#[test]
fn testId() -> TestResult {
    let proxy = set_up();
    assert_eq!(session_id().to_string_lossy(), proxy.get_id());
    proxy.set_id("foo")?;
    assert_eq!("foo", proxy.get_id());
    assert_eq!(session_id().to_string_lossy(), proxy.get_id());
    Ok(())
}

#[test]
fn testIdException() -> TestResult {
    let _save_path = SavePath::set_up();
    let proxy = set_up();
    session_start()?;
    let error = proxy
        .set_id("foo")
        .err()
        .ok_or("expected a LogicException")?;
    assert!(error.is_a("LogicException"));
    Ok(())
}
