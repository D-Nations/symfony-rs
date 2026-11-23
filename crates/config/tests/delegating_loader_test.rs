//! DelegatingLoaderTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; a mock's `expects($this->once())` is checked
//! on its call counts.
//!
//! Upstream: symfony/config Tests/Loader/DelegatingLoaderTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{TestResult, expect_class, loaders::MockLoader};
use php_value::Value;
use symfony_config::{
    Class,
    loader::{DelegatingLoader, LoaderInterface, LoaderResolver},
};

fn same<T: ?Sized, U: ?Sized>(a: &Arc<T>, b: &Arc<U>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

#[test]
fn testConstructor() {
    let _ = DelegatingLoader::new(LoaderResolver::new(vec![]));
    // __construct() takes a loader resolver as its first argument
}

#[test]
fn testGetSetResolver() -> TestResult {
    let resolver = LoaderResolver::new(vec![]);
    let loader = DelegatingLoader::new(resolver.clone());
    assert!(
        same(&resolver, &loader.get_resolver()?),
        "->getResolver() gets the resolver loader"
    );
    let resolver = LoaderResolver::new(vec![]);
    loader.set_resolver(resolver.clone());
    assert!(
        same(&resolver, &loader.get_resolver()?),
        "->setResolver() sets the resolver loader"
    );
    Ok(())
}

#[test]
fn testSupports() {
    let loader1 = MockLoader::new(true, Value::Null);
    let loader = DelegatingLoader::new(LoaderResolver::new(vec![loader1.clone()]));
    assert!(
        loader.supports(&Value::string("foo.xml"), None),
        "->supports() returns true if the resource is loadable"
    );
    assert_eq!(1, loader1.supports_count());

    let loader1 = MockLoader::new(false, Value::Null);
    let loader = DelegatingLoader::new(LoaderResolver::new(vec![loader1.clone()]));
    assert!(
        !loader.supports(&Value::string("foo.foo"), None),
        "->supports() returns false if the resource is not loadable"
    );
    assert_eq!(1, loader1.supports_count());
}

#[test]
fn testLoad() -> TestResult {
    let mock = MockLoader::new(true, Value::Null);
    let resolver = LoaderResolver::new(vec![mock.clone()]);
    let loader = DelegatingLoader::new(resolver);

    loader.load(&Value::string("foo"), None)?;
    assert_eq!(1, mock.supports_count());
    assert_eq!(1, mock.loads().len());
    Ok(())
}

#[test]
fn testLoadThrowsAnExceptionIfTheResourceCannotBeLoaded() {
    let mock = MockLoader::new(false, Value::Null);
    let resolver = LoaderResolver::new(vec![mock.clone()]);
    let loader = DelegatingLoader::new(resolver);

    expect_class(loader.load(&Value::string("foo"), None), Class::LoaderLoad);
    assert_eq!(1, mock.supports_count());
}
