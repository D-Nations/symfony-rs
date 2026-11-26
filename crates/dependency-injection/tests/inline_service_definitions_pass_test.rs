//! Tests/Compiler/InlineServiceDefinitionsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `assertEquals()` is PHP's `==` (`loose_equals`),
//! `assertSame()` on definitions is object identity and on references
//! equality (a `Reference` is an immutable value here).
//!
//! testDoNotInline's `compile()` is `support::compile()` (nothing is
//! autowired).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/InlineServiceDefinitionsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod support;

use php_value::compare::loose_equals;
use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::argument::{IteratorArgument, ServiceClosureArgument};
use symfony_dependency_injection::compiler::{
    AnalyzeServiceReferencesPass, CompilerPassInterface, InlineServiceDefinitionsPass,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference, Result,
};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    InlineServiceDefinitionsPass::new(Some(AnalyzeServiceReferencesPass::default()))
        .process(container)
}

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

fn arg(definition: &Definition, index: i64) -> Value {
    definition.argument(index).unwrap()
}

fn def(value: &Value) -> Definition {
    Definition::from_value(value).unwrap()
}

fn reference(value: &Value) -> Reference {
    Reference::from_value(value).unwrap().clone()
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    let inlineable = container.register("inlinable.service", None).unwrap();

    container
        .register("service", None)
        .unwrap()
        .set_arguments(list([Reference::new("inlinable.service").into()]));

    process(&mut container).unwrap();

    let arguments = container.get_definition("service").unwrap().arguments();
    let first = arguments.get(&Key::Int(0)).unwrap();
    assert!(Definition::from_value(first).is_some());
    assert!(inlineable.ptr_eq(&def(first)));
    assert!(!container.has("inlinable.service"));
}

#[test]
fn testProcessDoesNotInlinesWhenAliasedServiceIsShared() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();
    container.set_alias("moo", "foo").unwrap().set_public(true);

    let r = Reference::new("foo");
    container
        .register("service", None)
        .unwrap()
        .set_arguments(list([r.clone().into()]));

    process(&mut container).unwrap();

    let service = container.get_definition("service").unwrap();
    assert_eq!(r, reference(&arg(&service, 0)));
}

#[test]
fn testProcessDoesInlineNonSharedService() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .set_shared(false);
    let bar = container.register("bar", None).unwrap();
    bar.set_shared(false);
    container.set_alias("moo", "bar").unwrap();

    let r = Reference::new("moo");
    container
        .register("service", None)
        .unwrap()
        .set_arguments(list([
            Reference::new("foo").into(),
            r.clone().into(),
            Reference::new("bar").into(),
        ]));

    process(&mut container).unwrap();

    let service = container.get_definition("service").unwrap();
    let foo = container.get_definition("foo").unwrap();
    assert!(loose_equals(&foo.to_value(), &arg(&service, 0)));
    assert!(!foo.ptr_eq(&def(&arg(&service, 0))));
    assert_eq!(r, reference(&arg(&service, 1)));
    assert!(loose_equals(&bar.to_value(), &arg(&service, 2)));
    assert!(!bar.ptr_eq(&def(&arg(&service, 2))));
    assert!(!container.has("bar"));
}

#[test]
fn testProcessDoesNotInlineMixedServicesLoop() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("bar"))
        .set_shared(false);
    container
        .register("bar", None)
        .unwrap()
        .set_public(true)
        .add_method_call("setFoo", list([Reference::new("foo").into()]), false)
        .unwrap();

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    assert!(loose_equals(&Reference::new("bar").into(), &arg(&foo, 0)));
}

#[test]
fn testProcessThrowsOnNonSharedLoops() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .add_argument(Reference::new("bar"))
        .set_shared(false);
    container
        .register("bar", None)
        .unwrap()
        .set_shared(false)
        .add_method_call("setFoo", list([Reference::new("foo").into()]), false)
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert_eq!(
        "Circular reference detected for service \"bar\", path: \"bar -> foo -> bar\".",
        e.message()
    );
}

#[test]
fn testProcessNestedNonSharedServices() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .add_argument(Reference::new("bar1"))
        .add_argument(Reference::new("bar2"));
    container
        .register("bar1", None)
        .unwrap()
        .set_shared(false)
        .add_argument(Reference::new("baz"));
    container
        .register("bar2", None)
        .unwrap()
        .set_shared(false)
        .add_argument(Reference::new("baz"));
    container
        .register("baz", None)
        .unwrap()
        .set_public(true)
        .set_shared(false);

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let baz1 = arg(&def(&arg(&foo, 0)), 0);
    let baz2 = arg(&def(&arg(&foo, 1)), 0);

    let baz = container.get_definition("baz").unwrap().to_value();
    assert!(loose_equals(&baz, &baz1));
    assert!(loose_equals(&baz, &baz2));
    assert!(!def(&baz1).ptr_eq(&def(&baz2)));
}

#[test]
fn testProcessInlinesIfMultipleReferencesButAllFromTheSameDefinition() {
    let mut container = ContainerBuilder::new();

    let a = container.register("a", None).unwrap();
    let b = container.register("b", None).unwrap();
    b.add_argument(Reference::new("a"))
        .add_argument(Definition::new(None, list([Reference::new("a").into()])));

    process(&mut container).unwrap();

    assert!(a.ptr_eq(&def(&arg(&b, 0))));

    let inlined = def(&arg(&b, 1));
    assert!(a.ptr_eq(&def(&arg(&inlined, 0))));
}

fn factory(service: &str, method: &str) -> Value {
    Value::Array(list([Reference::new(service).into(), Value::from(method)]))
}

#[test]
fn testProcessInlinesPrivateFactoryReference() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();
    let b = container.register("b", None).unwrap();
    b.set_factory(factory("a", "a"));

    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([Reference::new("b").into()]));

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    assert!(b.ptr_eq(&def(&arg(&foo, 0))));
}

#[test]
fn testProcessDoesNotInlinePrivateFactoryIfReferencedMultipleTimesWithinTheSameDefinition() {
    let mut container = ContainerBuilder::new();
    container.register("a", None).unwrap();
    container
        .register("b", None)
        .unwrap()
        .set_factory(factory("a", "a"));

    let ref1 = Reference::new("b");
    let ref2 = Reference::new("b");
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([ref1.clone().into(), ref2.clone().into()]));
    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    assert_eq!(ref1, reference(&arg(&foo, 0)));
    assert_eq!(ref2, reference(&arg(&foo, 1)));
}

#[test]
fn testProcessDoesNotInlineReferenceWhenUsedByInlineFactory() {
    let mut container = ContainerBuilder::new();
    container.register("a", None).unwrap();
    container
        .register("b", None)
        .unwrap()
        .set_factory(factory("a", "a"));

    let inline_factory = Definition::default();
    inline_factory.set_factory(factory("b", "b"));

    let r = Reference::new("b");
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([r.clone().into(), inline_factory.into()]));
    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    assert_eq!(r, reference(&arg(&foo, 0)));
}

#[test]
fn testProcessDoesNotInlineWhenServiceIsPrivateButLazy() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().set_lazy(true);

    let r = Reference::new("foo");
    container
        .register("service", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([r.clone().into()]));

    process(&mut container).unwrap();

    let service = container.get_definition("service").unwrap();
    assert_eq!(r, reference(&arg(&service, 0)));
}

fn closure(reference: &Reference) -> Definition {
    let definition = Definition::with_class("Closure");
    definition.set_factory(Value::Array(list([
        Value::from("Closure"),
        Value::from("fromCallable"),
    ])));
    definition.set_arguments(list([Value::Array(list([
        reference.clone().into(),
        Value::from("baz"),
    ]))]));
    definition
}

fn first_of(value: &Value) -> Value {
    value.as_array().unwrap().get(&Key::Int(0)).unwrap().clone()
}

#[test]
fn testProcessDoesNotInlineServiceOfLazyClosure() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();
    container.register("bar", None).unwrap();

    let r = Reference::new("bar");
    let lazy = closure(&r);
    lazy.set_lazy(true);
    container
        .register("service", None)
        .unwrap()
        .set_public(true)
        .add_argument(closure(&Reference::new("foo")))
        .add_argument(lazy);

    process(&mut container).unwrap();

    let service = container.get_definition("service").unwrap();
    let first = first_of(&arg(&def(&arg(&service, 0)), 0));
    assert!(Definition::from_value(&first).is_some());
    let second = first_of(&arg(&def(&arg(&service, 1)), 0));
    assert_eq!(r, reference(&second));
}

#[test]
fn testProcessDoesNotInlineWhenServiceReferencesItself() {
    let mut container = ContainerBuilder::new();
    let r = Reference::new("foo");
    container
        .register("foo", None)
        .unwrap()
        .add_method_call("foo", list([r.clone().into()]), false)
        .unwrap();

    process(&mut container).unwrap();

    let calls = container.get_definition("foo").unwrap().method_calls();
    let call = calls.get(&Key::Int(0)).unwrap().as_array().unwrap();
    let arguments = call.get(&Key::Int(1)).unwrap();
    assert_eq!(r, reference(&first_of(arguments)));
}

#[test]
fn testProcessDoesNotSetLazyArgumentValuesAfterInlining() {
    let mut container = ContainerBuilder::new();
    container
        .register("inline", None)
        .unwrap()
        .set_shared(false);
    container
        .register("service-closure", None)
        .unwrap()
        .set_arguments(list([ServiceClosureArgument::new(
            Reference::new("inline").into(),
        )
        .to_value()]));
    container
        .register("iterator", None)
        .unwrap()
        .set_arguments(list([IteratorArgument::new(list([Reference::new(
            "inline",
        )
        .into()]))
        .to_value()]));

    process(&mut container).unwrap();

    let closure = container.get_definition("service-closure").unwrap();
    let values = ServiceClosureArgument::from_value(&arg(&closure, 0))
        .unwrap()
        .values();
    let value = values.get(&Key::Int(0)).unwrap();
    assert!(Reference::from_value(value).is_some());
    assert_eq!("inline", reference(value).id());

    let iterator = container.get_definition("iterator").unwrap();
    let values = IteratorArgument::from_value(&arg(&iterator, 0))
        .unwrap()
        .values();
    let value = values.get(&Key::Int(0)).unwrap();
    assert!(Reference::from_value(value).is_some());
    assert_eq!("inline", reference(value).id());
}

#[test]
fn testDoNotInline() {
    let mut container = ContainerBuilder::new();
    container
        .register("decorated1", Some("decorated1"))
        .unwrap()
        .add_tag("container.do_not_inline", PhpArray::new());
    container
        .register("decorated2", Some("decorated2"))
        .unwrap()
        .add_tag("container.do_not_inline", PhpArray::new());
    container.set_alias("alias2", "decorated2").unwrap();

    let s1 = container.register("s1", Some("s1")).unwrap();
    s1.set_decorated_service(Some("decorated1"), None, 0, InvalidBehavior::Exception)
        .unwrap();
    s1.set_public(true);
    let mut properties = PhpArray::new();
    properties.insert("inner", Reference::new("s1.inner").into());
    s1.set_properties(properties);

    let s2 = container.register("s2", Some("s2")).unwrap();
    s2.set_decorated_service(Some("alias2"), None, 0, InvalidBehavior::Exception)
        .unwrap();
    s2.set_public(true);
    let mut properties = PhpArray::new();
    properties.insert("inner", Reference::new("s2.inner").into());
    s2.set_properties(properties);

    support::compile(&mut container, false).unwrap();

    assert!(!container.has_alias("alias2"));
    let inner = |id: &str| {
        reference(
            &container
                .get_definition(id)
                .unwrap()
                .properties()
                .get(&Key::from("inner"))
                .unwrap()
                .clone(),
        )
    };
    assert_eq!(Reference::new("decorated2"), inner("s2"));
    assert_eq!(Reference::new("s1.inner"), inner("s1"));
    assert_eq!(
        Some("decorated2".to_string()),
        container.get_definition("decorated2").unwrap().class()
    );
    assert_eq!(
        Some("decorated1".to_string()),
        container.get_definition("s1.inner").unwrap().class()
    );
}
