//! Tests/Compiler/CheckFactoryBuilderCircularReferencePassTest.php at
//! symfony/dependency-injection 9b34cad8. Names, comments and assertions
//! are PHPUnit's.
//!
//! `$container->compile()` is `support::compile()`: the default passes
//! without `ResolveAutowireInlineAttributesPass` and `AutowirePass`, not
//! ported yet (nothing is autowired in these containers).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckFactoryBuilderCircularReferencePassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod support;

use php_value::{PhpArray, Value};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind, Reference, Result};

fn compile(container: &mut ContainerBuilder) -> Result<()> {
    support::compile(container, false)
}

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

fn factory(service: &str, method: &str) -> Value {
    Value::Array(list([Reference::new(service).into(), Value::from(method)]))
}

fn call(container: &mut ContainerBuilder, id: &str, method: &str, argument: Value) {
    container
        .get_definition(id)
        .unwrap()
        .add_method_call(method, list([argument]), false)
        .unwrap();
}

fn assert_circular(container: &mut ContainerBuilder, message: Option<&str>) {
    let e = compile(container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    if let Some(message) = message {
        assert!(e.message().contains(message), "{}", e.message());
    }
}

#[test]
fn testThrowsWhenInlinedConsumerReferencesProduct() {
    // The consumer ("extension") is private and used only by the builder's
    // method call, so it gets inlined into the builder. The dumper would emit
    // "$instance = $a->build()" before "$a->useExtension(...)" and silently
    // produce a half-built service.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useExtension",
        Reference::new("extension").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("product"));

    assert_circular(
        &mut container,
        Some("Circular reference detected for service \"product\""),
    );
}

#[test]
fn testThrowsWhenPublicConsumerReferencesProduct() {
    // The consumer ("extension") is public, so it stays a Reference inside
    // the inlined builder's setter args. Same broken pattern, different graph
    // shape: the previous dumper-side check missed this one.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useExtension",
        Reference::new("extension").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("product"));

    assert_circular(
        &mut container,
        Some(
            "Circular reference detected for service \"product\", path: \"product -> extension -> product\"",
        ),
    );
}

#[test]
fn testThrowsWhenBuilderIsSharedWithAnotherConsumer() {
    // A second consumer keeps the builder out of the produced service: the
    // factory stays a Reference and the builder is a shared service of its
    // own. The dumper then shares the builder before running its setters, so
    // the re-entrant call builds the product from an unconfigured builder.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useExtension",
        Reference::new("extension").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("builder_consumer", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("builder"));
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("product"));

    assert_circular(
        &mut container,
        Some(
            "Circular reference detected for service \"product\", path: \"product -> builder -> extension -> product\"",
        ),
    );
}

#[test]
fn testAllowsBuilderWithoutSetup() {
    // The factory builder has no method calls/properties/configurator, so the
    // soft-circular pattern works: the cycle is on the produced service's own
    // setter, not on the builder.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .set_factory(factory("builder", "build"));
    call(
        &mut container,
        "product",
        "setExtension",
        Reference::new("extension").into(),
    );
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("product"));

    compile(&mut container).unwrap();
}

#[test]
fn testAllowsStringFactoryWithSelfSetterCycle() {
    let mut container = ContainerBuilder::new();
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .set_factory("create_product");
    call(
        &mut container,
        "product",
        "setExtension",
        Reference::new("extension").into(),
    );
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("product"));

    compile(&mut container).unwrap();
}

#[test]
fn testAllowsConstructorArgInlinedSetterCycle() {
    // FooCircular-style: foo's constructor takes bar; bar's setter takes foobar;
    // foobar's constructor takes foo. The inlined sub-Definition (bar) is a
    // *constructor arg*, not a factory, so foo holds bar and bar's deferred
    // setter is observable through that reference. Legit soft-circular.
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("bar"));
    container.register("bar", Some("stdClass")).unwrap();
    call(
        &mut container,
        "bar",
        "addFoobar",
        Reference::new("foobar").into(),
    );
    container
        .register("foobar", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("foo"));

    compile(&mut container).unwrap();
}

#[test]
fn testIgnoresLazyConsumer() {
    // The consumer is lazy: it gets a proxy, the eager construction chain is
    // broken. The same shape as Case B but with `extension` lazy.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useExtension",
        Reference::new("extension").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .set_lazy(true)
        .add_argument(Reference::new("product"));

    compile(&mut container).unwrap();
}

#[test]
fn testFollowsTransitiveConstructorChain() {
    // builder's setter doesn't reference product directly, but the chain
    // builder -> mid -> product still requires product before the factory
    // call. Same shape, one hop further.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useMid",
        Reference::new("mid").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("mid", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("extension"));
    container
        .register("extension", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("product"));

    assert_circular(&mut container, None);
}

#[test]
fn testIgnoresInlinedDefinitionFactoryWithoutCycle() {
    // The factory builder has setters but they don't lead back to the
    // produced service. Should compile cleanly.
    let mut container = ContainerBuilder::new();
    container.register("builder", Some("stdClass")).unwrap();
    call(
        &mut container,
        "builder",
        "useUnrelated",
        Reference::new("unrelated").into(),
    );
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);
    container
        .register("unrelated", Some("stdClass"))
        .unwrap()
        .set_public(true);

    compile(&mut container).unwrap();
}

#[test]
fn testInlinedExtensionDefinitionRouteAlsoCaught() {
    // Same as Case A but using a literal Definition argument (mimicking what
    // InlineServiceDefinitionsPass produces) so we exercise the
    // walk-Definition path of the pass.
    let mut container = ContainerBuilder::new();
    let extension = Definition::with_class("stdClass");
    extension.add_argument(Reference::new("product"));
    container.register("builder", Some("stdClass")).unwrap();
    call(&mut container, "builder", "useExtension", extension.into());
    container
        .register("product", Some("stdClass"))
        .unwrap()
        .set_factory(factory("builder", "build"))
        .set_public(true);

    assert_circular(&mut container, None);
}
