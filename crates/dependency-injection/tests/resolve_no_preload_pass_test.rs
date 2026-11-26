//! Tests/Compiler/ResolveNoPreloadPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveNoPreloadPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::PhpArray;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveNoPreloadPass};
use symfony_dependency_injection::{ContainerBuilder, Reference};

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    container
        .register("entry_point", None)
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("preloaded"))
        .add_argument(Reference::new("not_preloaded"));

    container
        .register("preloaded", None)
        .unwrap()
        .add_argument(Reference::new("preloaded_dep"))
        .add_argument(Reference::new("common_dep"));

    container
        .register("not_preloaded", None)
        .unwrap()
        .set_public(true)
        .add_tag("container.no_preload", PhpArray::new())
        .add_argument(Reference::new("not_preloaded_dep"))
        .add_argument(Reference::new("common_dep"));

    container.register("preloaded_dep", None).unwrap();
    container.register("not_preloaded_dep", None).unwrap();
    container.register("common_dep", None).unwrap();

    ResolveNoPreloadPass::new().process(&mut container).unwrap();

    let has = |id: &str| {
        container
            .get_definition(id)
            .unwrap()
            .has_tag("container.no_preload")
    };
    assert!(!has("entry_point"));
    assert!(!has("preloaded"));
    assert!(!has("preloaded_dep"));
    assert!(!has("common_dep"));
    assert!(has("not_preloaded"));
    assert!(has("not_preloaded_dep"));
}
