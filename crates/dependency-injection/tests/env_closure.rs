//! Argument/EnvClosure.php has no upstream test of its own (the
//! container and dumper tests use it through compiled containers); these
//! pin `__invoke()` and `__toString()`.
//!
//! Upstream: none
#![allow(clippy::unwrap_used)]

use php_value::{PhpString, Value, value::PhpObject};
use symfony_dependency_injection::argument::EnvClosure;
use symfony_dependency_injection::{Error, ErrorKind};

#[test]
fn invoke_returns_the_closure_value() {
    let closure = EnvClosure::new(|| Ok(Value::from("foo")), Value::from("default"));
    assert_eq!(Value::from("foo"), closure.invoke().unwrap());
}

#[test]
fn invoke_returns_the_default_when_the_env_var_is_missing() {
    let closure = EnvClosure::new(
        || Err(Error::env_not_found("missing")),
        Value::from("default"),
    );
    assert_eq!(Value::from("default"), closure.invoke().unwrap());
}

#[test]
fn invoke_rethrows_without_a_default() {
    let closure = EnvClosure::new(|| Err(Error::env_not_found("missing")), Value::Null);
    let error = closure.invoke().unwrap_err();
    assert!(error.is(&ErrorKind::EnvNotFound));
    assert_eq!("missing", error.message());
}

#[test]
fn invoke_rethrows_other_exceptions() {
    let closure = EnvClosure::new(|| Err(Error::runtime("broken")), Value::from("default"));
    assert!(closure.invoke().unwrap_err().is(&ErrorKind::Runtime));
}

#[test]
fn to_string_coerces_scalars() {
    let closure = EnvClosure::new(|| Ok(Value::Int(12)), Value::Null);
    assert_eq!(PhpString::from("12"), closure.to_string().unwrap());
    assert_eq!(
        Some(PhpString::from("12")),
        PhpObject::to_php_string(&closure)
    );
}

#[test]
fn to_string_refuses_an_array() {
    let closure = EnvClosure::new(|| Ok(Value::Array(php_value::PhpArray::new())), Value::Null);
    let error = closure.to_string().unwrap_err();
    assert_eq!("TypeError", error.class());
    assert_eq!(
        "Symfony\\Component\\DependencyInjection\\Argument\\EnvClosure::__toString(): Return value must be of type string, array returned",
        error.message()
    );
}
