//! Tests/Compiler/AnalyzeServiceReferencesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `assertSame()` on references is equality (a `Reference` is
//! an immutable value here).
//!
//! Pending: testExpressionReferenceKeepsConstructorFlag (needs
//! `ExpressionLanguage`'s `Expression`, which the port does not have).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AnalyzeServiceReferencesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::compiler::{
    AnalyzeServiceReferencesPass, CompilerPassInterface, ServiceReferenceGraph,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, Reference};

fn process(container: &mut ContainerBuilder) -> ServiceReferenceGraph {
    let mut pass = AnalyzeServiceReferencesPass::default();
    pass.process(container).unwrap();
    container
        .get_compiler()
        .get_service_reference_graph()
        .clone()
}

fn in_values(graph: &ServiceReferenceGraph, id: &str) -> Vec<Option<Reference>> {
    let node = graph.node(graph.get_node(id).unwrap());
    node.get_in_edges()
        .iter()
        .map(|e| graph.edge(*e).get_value().cloned())
        .collect()
}

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    let ref1 = Reference::new("b");
    container
        .register("a", None)
        .unwrap()
        .add_argument(ref1.clone());

    let ref2 = Reference::new("a");
    container
        .register("b", None)
        .unwrap()
        .add_method_call("setA", list([ref2.into()]), false)
        .unwrap();

    let ref3 = Reference::new("a");
    let ref4 = Reference::new("b");
    container
        .register("c", None)
        .unwrap()
        .add_argument(ref3)
        .add_argument(ref4.clone());

    let ref5 = Reference::new("b");
    container
        .register("d", None)
        .unwrap()
        .set_property("foo", ref5.clone());

    let ref6 = Reference::new("b");
    container
        .register("e", None)
        .unwrap()
        .set_configurator(Value::Array(list([
            ref6.clone().into(),
            Value::from("methodName"),
        ])));

    let graph = process(&mut container);

    let edges = in_values(&graph, "b");
    assert_eq!(4, edges.len());

    assert_eq!(Some(ref1), edges[0]);
    assert_eq!(Some(ref4), edges[1]);
    assert_eq!(Some(ref5), edges[2]);
    assert_eq!(Some(ref6), edges[3]);
}

#[test]
fn testProcessMarksEdgesLazyWhenReferencedServiceIsLazy() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", None)
        .unwrap()
        .set_lazy(true)
        .add_argument(Reference::new("b"));

    let ref2 = Reference::new("a");
    container
        .register("b", None)
        .unwrap()
        .add_argument(ref2.clone());

    let graph = process(&mut container);

    assert_eq!(1, in_values(&graph, "b").len());
    let edges = graph.node(graph.get_node("a").unwrap()).get_in_edges();
    assert_eq!(1, edges.len());

    assert_eq!(Some(&ref2), graph.edge(edges[0]).get_value());
    assert!(graph.edge(edges[0]).is_lazy());
}

#[test]
fn testProcessMarksEdgesLazyWhenReferencedFromIteratorArgument() {
    let mut container = ContainerBuilder::new();
    container.register("a", None).unwrap();
    container.register("b", None).unwrap();

    let ref1 = Reference::new("a");
    let ref2 = Reference::new("b");
    container
        .register("c", None)
        .unwrap()
        .add_argument(ref1.clone())
        .add_argument(IteratorArgument::new(list([ref2.clone().into()])).to_value());

    let graph = process(&mut container);

    assert_eq!(1, in_values(&graph, "a").len());
    assert_eq!(1, in_values(&graph, "b").len());
    let edges = graph.node(graph.get_node("c").unwrap()).get_out_edges();
    assert_eq!(2, edges.len());

    assert_eq!(Some(&ref1), graph.edge(edges[0]).get_value());
    assert!(!graph.edge(edges[0]).is_lazy());
    assert_eq!(Some(&ref2), graph.edge(edges[1]).get_value());
    assert!(graph.edge(edges[1]).is_lazy());
}

fn closure(class: &str, reference: &Reference) -> Definition {
    let definition = Definition::with_class(class);
    definition.set_factory(Value::Array(list([
        Value::from("Closure"),
        Value::from("fromCallable"),
    ])));
    definition.set_arguments(list([Value::Array(list([
        reference.clone().into(),
        Value::from("foo"),
    ]))]));
    definition
}

#[test]
fn testProcessMarksEdgesLazyWhenReferencedFromLazyClosure() {
    let mut container = ContainerBuilder::new();
    container.register("a", None).unwrap();
    container.register("b", None).unwrap();
    container.register("c", None).unwrap();

    let ref1 = Reference::new("a");
    let ref2 = Reference::new("b");
    let ref3 = Reference::new("c");
    let lazy = closure("Closure", &ref2);
    lazy.set_lazy(true);
    container
        .register("d", None)
        .unwrap()
        .add_argument(closure("Closure", &ref1))
        .add_argument(lazy)
        .add_argument(closure("Countable", &ref3));

    let graph = process(&mut container);

    let edges = graph.node(graph.get_node("d").unwrap()).get_out_edges();
    assert_eq!(3, edges.len());

    assert_eq!(Some(&ref1), graph.edge(edges[0]).get_value());
    assert!(!graph.edge(edges[0]).is_lazy());
    assert_eq!(Some(&ref2), graph.edge(edges[1]).get_value());
    assert!(graph.edge(edges[1]).is_lazy());
    assert_eq!(Some(&ref3), graph.edge(edges[2]).get_value());
    assert!(graph.edge(edges[2]).is_lazy());
}

#[test]
fn testProcessDetectsReferencesFromInlinedDefinitions() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();

    let reference = Reference::new("a");
    container
        .register("b", None)
        .unwrap()
        .add_argument(Definition::new(None, list([reference.clone().into()])));

    let graph = process(&mut container);

    let refs = in_values(&graph, "a");
    assert_eq!(1, refs.len());
    assert_eq!(Some(reference), refs[0]);
}

#[test]
fn testProcessDetectsReferencesFromIteratorArguments() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();

    let reference = Reference::new("a");
    container
        .register("b", None)
        .unwrap()
        .add_argument(IteratorArgument::new(list([reference.clone().into()])).to_value());

    let graph = process(&mut container);

    let refs = in_values(&graph, "a");
    assert_eq!(1, refs.len());
    assert_eq!(Some(reference), refs[0]);
}

#[test]
fn testProcessDetectsReferencesFromInlinedFactoryDefinitions() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();

    let factory = Definition::default();
    factory.set_factory(Value::Array(list([
        Reference::new("a").into(),
        Value::from("a"),
    ])));

    container.register("b", None).unwrap().add_argument(factory);

    let graph = process(&mut container);

    assert!(graph.has_node("a"));
    assert_eq!(1, in_values(&graph, "a").len());
}

#[test]
fn testProcessDoesNotSaveDuplicateReferences() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();
    container
        .register("b", None)
        .unwrap()
        .add_argument(Definition::new(None, list([Reference::new("a").into()])))
        .add_argument(Definition::new(None, list([Reference::new("a").into()])));

    let graph = process(&mut container);

    assert_eq!(2, in_values(&graph, "a").len());
}

#[test]
fn testProcessDetectsFactoryReferences() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from("stdClass"),
            Value::from("getInstance"),
        ])));

    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .set_factory(Value::Array(list([
            Reference::new("foo").into(),
            Value::from("getInstance"),
        ])));

    let graph = process(&mut container);

    assert!(graph.has_node("foo"));
    assert_eq!(1, in_values(&graph, "foo").len());
}
