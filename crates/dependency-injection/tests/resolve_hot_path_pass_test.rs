//! Tests/Compiler/ResolveHotPathPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveHotPathPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::PhpArray;
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveHotPathPass};
use symfony_dependency_injection::{ContainerBuilder, Definition, InvalidBehavior, Reference};

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", None)
        .unwrap()
        .add_tag("container.hot_path", PhpArray::new())
        .add_argument(
            IteratorArgument::new(PhpArray::list([Reference::new("lazy").into()])).to_value(),
        )
        .add_argument(Reference::new("service_container"))
        .add_argument(
            Definition::new(Some(""), PhpArray::list([Reference::new("bar").into()])).to_value(),
        )
        .add_argument(Reference::with_behavior(
            "baz",
            InvalidBehavior::IgnoreUninitialized,
        ))
        .add_argument(Reference::new("missing"));

    container.register("lazy", None).unwrap();
    container
        .register("bar", None)
        .unwrap()
        .add_argument(Reference::new("buz"))
        .add_argument(Reference::new("deprec_ref_notag"));
    container
        .register("baz", None)
        .unwrap()
        .add_argument(Reference::new("lazy"))
        .add_argument(Reference::new("lazy"));
    container.register("buz", None).unwrap();
    container
        .register("deprec_with_tag", None)
        .unwrap()
        .set_deprecated("vendor/package", "1.1", "%service_id%")
        .unwrap()
        .add_tag("container.hot_path", PhpArray::new());
    container
        .register("deprec_ref_notag", None)
        .unwrap()
        .set_deprecated("vendor/package", "1.1", "%service_id%")
        .unwrap();

    ResolveHotPathPass::new().process(&mut container).unwrap();

    let has = |id: &str| {
        container
            .get_definition(id)
            .unwrap()
            .has_tag("container.hot_path")
    };
    assert!(!has("lazy"));
    assert!(has("bar"));
    assert!(has("buz"));
    assert!(!has("baz"));
    assert!(!has("service_container"));
    assert!(!has("deprec_with_tag"));
    assert!(!has("deprec_ref_notag"));
}
