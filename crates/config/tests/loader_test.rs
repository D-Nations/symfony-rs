//! LoaderTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; `assertSame()` on objects is `Arc::ptr_eq`, and a mock's
//! `expects($this->once())->with(...)` is checked on its recorded calls.
//!
//! Upstream: symfony/config Tests/Loader/LoaderTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{
    TestResult, expect_class,
    loaders::{MockLoader, MockResolver, ProjectLoader1},
};
use php_value::Value;
use symfony_config::{
    Class,
    loader::{LoaderInterface, LoaderResolver, LoaderResolverInterface},
};

fn same<T: ?Sized, U: ?Sized>(a: &Arc<T>, b: &Arc<U>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

fn s(v: &str) -> Value {
    Value::string(v)
}

#[test]
fn testGetSetResolver() -> TestResult {
    let resolver = LoaderResolver::new(vec![]);

    let loader = ProjectLoader1::new();
    loader.set_resolver(resolver.clone());

    assert!(
        same(&resolver, &loader.get_resolver()?),
        "->setResolver() sets the resolver loader"
    );
    Ok(())
}

#[test]
fn testGetResolverWithoutSetResolver() {
    let loader = ProjectLoader1::new();
    expect_class(loader.get_resolver().map(|_| ()), Class::Logic);
}

#[test]
fn testResolve() -> TestResult {
    let resolved_loader = MockLoader::stub();

    let resolver = MockResolver::new(Some(resolved_loader.clone()));

    let loader = ProjectLoader1::new();
    loader.set_resolver(resolver.clone());

    assert!(
        same(&loader, &loader.loader.resolve(&s("foo.foo"), None)?),
        "->resolve() finds a loader"
    );
    assert!(
        same(
            &resolved_loader,
            &loader.loader.resolve(&s("foo.xml"), None)?
        ),
        "->resolve() finds a loader"
    );
    let calls = resolver.resolves();
    assert_eq!(1, calls.len());
    assert!(calls[0].0.identical(&s("foo.xml")));
    Ok(())
}

#[test]
fn testResolveWithoutSetResolver() {
    let loader = ProjectLoader1::new();
    expect_class(
        loader.loader.resolve(&s("foo.xml"), None).map(|_| ()),
        Class::LoaderLoad,
    );
}

#[test]
fn testResolveWhenResolverCannotFindLoader() {
    let resolver = MockResolver::new(None);

    let loader = ProjectLoader1::new();
    loader.set_resolver(resolver.clone() as Arc<dyn LoaderResolverInterface>);

    expect_class(
        loader.loader.resolve(&s("FOOBAR"), None).map(|_| ()),
        Class::LoaderLoad,
    );
    let calls = resolver.resolves();
    assert_eq!(1, calls.len());
    assert!(calls[0].0.identical(&s("FOOBAR")));
}

#[test]
fn testImport() -> TestResult {
    let resolved_loader = MockLoader::new(false, s("yes"));

    let resolver = MockResolver::new(Some(resolved_loader.clone()));

    let loader = ProjectLoader1::new();
    loader.set_resolver(resolver.clone());

    assert!(loader.loader.import(&s("foo"), None)?.identical(&s("yes")));
    let loads = resolved_loader.loads();
    assert_eq!(1, loads.len());
    assert!(loads[0].0.identical(&s("foo")));
    let calls = resolver.resolves();
    assert_eq!(1, calls.len());
    assert!(calls[0].0.identical(&s("foo")));
    Ok(())
}

#[test]
fn testImportWithType() -> TestResult {
    let resolved_loader = MockLoader::new(false, s("yes"));

    let resolver = MockResolver::new(Some(resolved_loader.clone()));

    let loader = ProjectLoader1::new();
    loader.set_resolver(resolver.clone());

    assert!(
        loader
            .loader
            .import(&s("foo"), Some(b"bar"))?
            .identical(&s("yes"))
    );
    let loads = resolved_loader.loads();
    assert_eq!(1, loads.len());
    assert!(loads[0].0.identical(&s("foo")));
    assert_eq!(Some(b"bar".to_vec()), loads[0].1);
    let calls = resolver.resolves();
    assert_eq!(1, calls.len());
    assert!(calls[0].0.identical(&s("foo")));
    assert_eq!(Some(b"bar".to_vec()), calls[0].1);
    Ok(())
}
