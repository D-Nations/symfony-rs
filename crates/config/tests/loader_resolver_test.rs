//! LoaderResolverTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; `assertEquals()` on loaders compares
//! identity, and the resource closure is a PHP `Closure` object.
//!
//! Upstream: symfony/config Tests/Loader/LoaderResolverTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::loaders::MockLoader;
use php_value::{PhpArray, Value, object::GenericObject};
use symfony_config::loader::{LoaderInterface, LoaderResolver, LoaderResolverInterface};

fn same<T: ?Sized, U: ?Sized>(a: &Arc<T>, b: &Arc<U>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

#[test]
fn testConstructor() {
    let loader = MockLoader::stub();
    let resolver = LoaderResolver::new(vec![loader.clone()]);

    let loaders = resolver.get_loaders();
    assert_eq!(
        1,
        loaders.len(),
        "__construct() takes an array of loaders as its first argument"
    );
    assert!(same(&loader, &loaders[0]));
}

#[test]
fn testResolve() {
    let loader = MockLoader::stub();
    let resolver = LoaderResolver::new(vec![loader]);
    assert!(
        resolver.resolve(&Value::string("foo.foo"), None).is_none(),
        "->resolve() returns false if no loader is able to load the resource"
    );

    let loader = MockLoader::new(true, Value::Null);
    let resolver = LoaderResolver::new(vec![loader.clone()]);
    let closure = Value::Object(Arc::new(GenericObject::new("Closure", PhpArray::new())));
    let resolved = resolver.resolve(&closure, None);
    assert!(
        resolved.is_some_and(|r| same(&loader, &r)),
        "->resolve() returns the loader for the given resource"
    );
    assert_eq!(1, loader.supports_count());
}

#[test]
fn testLoaders() {
    let resolver = LoaderResolver::new(vec![]);
    let loader = MockLoader::stub();
    resolver.add_loader(loader.clone() as Arc<dyn LoaderInterface>);

    let loaders = resolver.get_loaders();
    assert_eq!(1, loaders.len(), "addLoader() adds a loader");
    assert!(same(&loader, &loaders[0]));
}
