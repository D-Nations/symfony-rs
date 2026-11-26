//! Tests/Loader/ClosureLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's. A PHP closure is a
//! `LoaderClosure`.
//!
//! Upstream: symfony/dependency-injection Tests/Loader/ClosureLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{Value, php_array};
use symfony_config::loader::LoaderInterface;
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::loader::closure_loader::LoaderClosure;
use symfony_dependency_injection::loader::{ClosureLoader, shared};

#[test]
fn testSupports() {
    let loader = ClosureLoader::new(shared(ContainerBuilder::new()), None);

    let closure = LoaderClosure::new(|_container, _env| Ok(Value::Null)).to_value();
    assert!(
        loader.supports(&closure, None),
        "->supports() returns true if the resource is loadable"
    );
    assert!(
        !loader.supports(&Value::from("foo.foo"), None),
        "->supports() returns true if the resource is loadable"
    );
}

#[test]
fn testLoad() {
    let container = shared(ContainerBuilder::new());
    let loader = ClosureLoader::new(container.clone(), Some("some-env"));

    let closure = LoaderClosure::new(|container, env| {
        container.set_parameter("foo", "foo")?;
        container.set_parameter("env", env.map_or(Value::Null, Value::from))?;
        Ok(Value::Null)
    });
    loader.load(&closure.to_value(), None).unwrap();

    assert_eq!(
        &php_array!["foo" => "foo", "env" => "some-env"],
        container.lock().unwrap().get_parameter_bag().all()
    );
}
