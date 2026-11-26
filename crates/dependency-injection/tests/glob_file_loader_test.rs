//! Tests/Loader/GlobFileLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's. Upstream's
//! `GlobFileLoaderWithoutImport` overrides `import()` to do nothing; here
//! the loader's resolver holds a loader that accepts every path and
//! loads nothing, which leaves the container as that override does.
//! The test that reads the fixtures skips when `upstream/` is not
//! fetched.
//!
//! Upstream: symfony/dependency-injection Tests/Loader/GlobFileLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::unnecessary_literal_bound)]

mod loader_support;

use std::sync::{Arc, Weak};

use php_value::{PhpArray, Value};
use symfony_config::loader::{Loader, LoaderInterface, LoaderResolver, LoaderResolverInterface};
use symfony_config::resource::GlobResource;
use symfony_config::{Error, FileLocator};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::loader::{GlobFileLoader, shared};

/// A loader that supports everything and loads nothing.
struct NullLoader(Loader);

impl LoaderInterface for NullLoader {
    fn class(&self) -> &str {
        "NullLoader"
    }
    fn load(&self, _resource: &Value, _type: Option<&[u8]>) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn supports(&self, _resource: &Value, _type: Option<&[u8]>) -> bool {
        true
    }
    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.0.get_resolver()
    }
    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.0.set_resolver(resolver);
    }
}

#[test]
fn testSupports() {
    let loader = GlobFileLoader::new(
        shared(ContainerBuilder::new()),
        Arc::new(FileLocator::default()),
        None,
        false,
    );

    assert!(
        loader.supports(&Value::from("any-path"), Some(b"glob")),
        "->supports() returns true if the resource has the glob type"
    );
    assert!(
        !loader.supports(&Value::from("any-path"), None),
        "->supports() returns false if the resource is not of glob type"
    );
}

#[test]
fn testLoadAddsTheGlobResourceToTheContainer() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = shared(ContainerBuilder::new());
    let loader = GlobFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::default()),
        None,
        false,
    );
    let null: Arc<dyn LoaderInterface> = Arc::new_cyclic(|this: &Weak<NullLoader>| {
        let this: Weak<dyn LoaderInterface> = this.clone();
        NullLoader(Loader::new(None, this))
    });
    let resolver = LoaderResolver::new(vec![null]);
    loader.set_resolver(resolver);
    loader
        .load(&Value::from(format!("{fixtures}/config/*")), None)
        .unwrap();

    let expected = GlobResource::new(
        format!("{fixtures}/config").as_bytes(),
        b"/*",
        false,
        false,
        PhpArray::new(),
    )
    .unwrap();
    let resources = container.lock().unwrap().get_resources();
    assert_eq!(
        symfony_config::resource::ResourceInterface::to_string(&expected),
        resources[0].to_string()
    );
}
