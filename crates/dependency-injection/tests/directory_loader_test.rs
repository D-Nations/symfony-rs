//! Tests/Loader/DirectoryLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's. The resolver holds the
//! ported loaders (no `PhpFileLoader`). Tests skip, with a note, when
//! `upstream/` is not fetched.
//!
//! Not ported: testDirectoryCanBeLoadedRecursively, which loads
//! `directory/simple.php` with `PhpFileLoader` (it runs PHP code).
//!
//! Upstream: symfony/dependency-injection Tests/Loader/DirectoryLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod loader_support;

use std::sync::Arc;

use loader_support::assert_error;
use php_value::{Value, php_array};
use symfony_config::FileLocator;
use symfony_config::loader::{LoaderInterface, LoaderResolver};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::loader::{
    DirectoryLoader, IniFileLoader, SharedContainer, YamlFileLoader, shared,
};

fn set_up() -> Option<(SharedContainer, Arc<DirectoryLoader>)> {
    let fixtures = loader_support::fixtures()?;
    let locator = Arc::new(FileLocator::new([fixtures]));
    let container = shared(ContainerBuilder::new());
    let loader = DirectoryLoader::new(container.clone(), locator.clone(), None, false);
    let resolver = LoaderResolver::new(vec![
        IniFileLoader::new(container.clone(), locator.clone(), None, false),
        YamlFileLoader::new(container.clone(), locator, None, false),
        loader.clone(),
    ]);
    loader.set_resolver(resolver);
    Some((container, loader))
}

#[test]
fn testImports() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    let resource = Value::from("directory/import/import.yml");
    // $this->loader->resolve(...)
    let resolved = if loader.supports(&resource, None) {
        let l: Arc<dyn LoaderInterface> = loader.clone();
        l
    } else {
        loader
            .get_resolver()
            .unwrap()
            .resolve(&resource, None)
            .unwrap()
    };
    resolved.load(&resource, None).unwrap();
    assert_eq!(
        &php_array!["ini" => "ini", "yaml" => "yaml"],
        container.lock().unwrap().get_parameter_bag().all(),
        "->load() takes a single file that imports a directory"
    );
}

#[test]
fn testExceptionIsRaisedWhenDirectoryDoesNotExist() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = loader.load(&Value::from("foo/"), None).unwrap_err();
    assert_error(
        &e,
        "InvalidArgumentException",
        "The file \"foo\" does not exist (in:",
    );
}

#[test]
fn testSupports() {
    let loader = DirectoryLoader::new(
        shared(ContainerBuilder::new()),
        Arc::new(FileLocator::default()),
        None,
        false,
    );

    assert!(
        loader.supports(&Value::from("directory/"), None),
        "->supports(\"directory/\") returns true"
    );
    assert!(
        loader.supports(&Value::from("directory/"), Some(b"directory")),
        "->supports(\"directory/\", \"directory\") returns true"
    );
    assert!(
        !loader.supports(&Value::from("directory"), None),
        "->supports(\"directory\") returns false"
    );
    assert!(
        loader.supports(&Value::from("directory"), Some(b"directory")),
        "->supports(\"directory\", \"directory\") returns true"
    );
    assert!(
        !loader.supports(&Value::from("directory"), Some(b"foo")),
        "->supports(\"directory\", \"foo\") returns false"
    );
}
