//! Tests/Loader/LoaderResolverTest.php at symfony/dependency-injection
//! 9b34cad8. The data provider's rows run in a loop; `assertInstanceOf()`
//! compares the resolved loader's class name. Skips, with a note, when
//! `upstream/` is not fetched.
//!
//! Not ported: the `php_with_wrong_ext.yml` row of testResolvesForcedType
//! and the `PhpFileLoader` of the resolver (`PhpFileLoader` runs PHP code
//! and is not ported).
//!
//! Upstream: symfony/dependency-injection Tests/Loader/LoaderResolverTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod loader_support;

use std::sync::Arc;

use php_value::Value;
use symfony_config::FileLocator;
use symfony_config::loader::{LoaderInterface, LoaderResolver, LoaderResolverInterface};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::loader::{
    ClosureLoader, IniFileLoader, YamlFileLoader, ini_file_loader, shared, yaml_file_loader,
};

fn set_up() -> Option<Arc<LoaderResolver>> {
    let fixtures = loader_support::fixtures()?;
    let container = shared(ContainerBuilder::new());
    let locator = |dir: &str| Arc::new(FileLocator::new([format!("{fixtures}/{dir}")]));
    let loaders: Vec<Arc<dyn LoaderInterface>> = vec![
        YamlFileLoader::new(container.clone(), locator("yaml"), None, false),
        IniFileLoader::new(container.clone(), locator("ini"), None, false),
        ClosureLoader::new(container, None),
    ];
    Some(LoaderResolver::new(loaders))
}

#[test]
fn testResolvesForcedType() {
    let Some(resolver) = set_up() else {
        return;
    };
    for (resource, r#type, expected_class) in [
        ("ini_with_wrong_ext.xml", "ini", ini_file_loader::CLASS),
        ("yaml_with_wrong_ext.ini", "yaml", yaml_file_loader::CLASS),
    ] {
        let loader = resolver
            .resolve(&Value::from(resource), Some(r#type.as_bytes()))
            .unwrap();
        assert_eq!(expected_class, loader.class(), "{resource}");
    }
}
