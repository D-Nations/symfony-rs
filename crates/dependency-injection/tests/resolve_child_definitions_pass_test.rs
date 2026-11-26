//! Tests/Compiler/ResolveChildDefinitionsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `$x::class` is the definition's object class.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveChildDefinitionsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::BoundArgument;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveChildDefinitionsPass};
use symfony_dependency_injection::{
    ContainerBuilder, DecoratedService, Definition, ErrorKind, InvalidBehavior, Result,
};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    let mut pass = ResolveChildDefinitionsPass::new();
    pass.process(container)
}

fn object_class(value: &Value) -> String {
    value.as_object().unwrap().class().to_string()
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container
        .register("parent", Some("foo"))
        .unwrap()
        .set_arguments(php_array!["moo", "b"])
        .set_property("foo", "moo");
    container
        .set_definition("child", Definition::child("parent"))
        .unwrap()
        .replace_argument(0, "a")
        .unwrap()
        .set_property("foo", "bar")
        .set_class(Some("bar"));

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert!(!def.is_child());
    assert_eq!(Some("bar".to_string()), def.class());
    assert_eq!(php_array!["a", "b"], def.arguments());
    assert_eq!(php_array!["foo" => "bar"], def.properties());
}

#[test]
fn testProcessAppendsMethodCallsAlways() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .add_method_call("foo", php_array!["bar"], false)
        .unwrap();

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap()
        .add_method_call("bar", php_array!["foo"], false)
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert_eq!(
        php_array![
            php_array![Value::from("foo"), Value::from(php_array!["bar"])],
            php_array![Value::from("bar"), Value::from(php_array!["foo"])],
        ],
        def.method_calls()
    );
}

#[test]
fn testProcessDoesNotCopyAbstract() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .set_abstract(true);

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert!(!def.is_abstract());
}

#[test]
fn testProcessDoesNotCopyShared() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .set_shared(false);

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert!(def.is_shared());
}

#[test]
fn testProcessDoesNotCopyTags() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .add_tag("foo", PhpArray::new());

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert_eq!(PhpArray::new(), def.tags());
}

#[test]
fn testProcessCopiesTagsProxy() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .add_tag("proxy", php_array!["a" => "b"]);

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap();
    let child2 = Definition::child("parent");
    child2.add_tag("proxy", php_array!["c" => "d"]);
    container.set_definition("child2", child2).unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child1").unwrap();
    assert_eq!(
        php_array!["proxy" => php_array![php_array!["a" => "b"]]],
        def.tags()
    );

    let def = container.get_definition("child2").unwrap();
    assert_eq!(
        php_array!["proxy" => php_array![php_array!["c" => "d"]]],
        def.tags()
    );
}

#[test]
fn testProcessDoesNotCopyDecoratedService() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", None)
        .unwrap()
        .set_decorated_service(Some("foo"), None, 0, InvalidBehavior::Exception)
        .unwrap();

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert_eq!(None, def.decorated_service());
}

#[test]
fn testProcessDoesNotDropShared() {
    let mut container = ContainerBuilder::new();

    container.register("parent", None).unwrap();

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap()
        .set_shared(false);

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert!(!def.is_shared());
}

#[test]
fn testProcessHandlesMultipleInheritance() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("foo"))
        .unwrap()
        .set_arguments(php_array!["foo", "bar", "c"]);

    container
        .set_definition("child2", Definition::child("child1"))
        .unwrap()
        .replace_argument(1, "b")
        .unwrap();

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap()
        .replace_argument(0, "a")
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child2").unwrap();
    assert_eq!(php_array!["a", "b", "c"], def.arguments());
    assert_eq!(Some("foo".to_string()), def.class());
}

#[test]
fn testSetLazyOnServiceHasParent() {
    let mut container = ContainerBuilder::new();

    container.register("parent", Some("stdClass")).unwrap();

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap()
        .set_lazy(true);

    process(&mut container).unwrap();

    assert!(container.get_definition("child1").unwrap().is_lazy());
}

#[test]
fn testSetLazyOnServiceIsParent() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("stdClass"))
        .unwrap()
        .set_lazy(true);

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    assert!(container.get_definition("child1").unwrap().is_lazy());
}

#[test]
fn testSetAutowiredOnServiceHasParent() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("stdClass"))
        .unwrap()
        .set_autowired(true);

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap()
        .set_autowired(false);

    process(&mut container).unwrap();

    assert!(!container.get_definition("child1").unwrap().is_autowired());
}

#[test]
fn testSetAutowiredOnServiceIsParent() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("stdClass"))
        .unwrap()
        .set_autowired(true);

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    assert!(container.get_definition("child1").unwrap().is_autowired());
}

#[test]
fn testDeepDefinitionsResolving() {
    let mut container = ContainerBuilder::new();

    container.register("parent", Some("parentClass")).unwrap();
    container
        .register("sibling", Some("siblingClass"))
        .unwrap()
        .set_configurator(php_array![Definition::child("parent"), "foo"])
        .set_factory(php_array![Definition::child("parent"), "foo"])
        .add_argument(Definition::child("parent"))
        .set_property("prop", Definition::child("parent"))
        .add_method_call("meth", php_array![Definition::child("parent")], false)
        .unwrap();

    process(&mut container).unwrap();

    let first = |value: Value| {
        value
            .as_array()
            .unwrap()
            .get(&Key::Int(0))
            .cloned()
            .unwrap()
    };
    let class = |value: &Value| Definition::from_value(value).unwrap().class();

    let configurator = first(container.get_definition("sibling").unwrap().configurator());
    assert_eq!(Definition::CLASS, object_class(&configurator));
    assert_eq!(Some("parentClass".to_string()), class(&configurator));

    let factory = first(container.get_definition("sibling").unwrap().factory());
    assert_eq!(Definition::CLASS, object_class(&factory));
    assert_eq!(Some("parentClass".to_string()), class(&factory));

    let argument = container
        .get_definition("sibling")
        .unwrap()
        .argument(0)
        .unwrap();
    assert_eq!(Definition::CLASS, object_class(&argument));
    assert_eq!(Some("parentClass".to_string()), class(&argument));

    let properties = container.get_definition("sibling").unwrap().properties();
    let prop = properties.get(&Key::from("prop")).unwrap();
    assert_eq!(Definition::CLASS, object_class(prop));
    assert_eq!(Some("parentClass".to_string()), class(prop));

    let method_calls = container.get_definition("sibling").unwrap().method_calls();
    let call = first(method_calls.get(&Key::Int(0)).cloned().unwrap());
    let call_argument = first(
        method_calls
            .get(&Key::Int(0))
            .unwrap()
            .as_array()
            .unwrap()
            .get(&Key::Int(1))
            .cloned()
            .unwrap(),
    );
    assert_eq!(Value::from("meth"), call);
    assert_eq!(Definition::CLASS, object_class(&call_argument));
    assert_eq!(Some("parentClass".to_string()), class(&call_argument));
}

#[test]
fn testSetDecoratedServiceOnServiceHasParent() {
    let mut container = ContainerBuilder::new();

    container.register("parent", Some("stdClass")).unwrap();

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap()
        .set_decorated_service(
            Some("foo"),
            Some("foo_inner"),
            5,
            InvalidBehavior::Exception,
        )
        .unwrap();

    process(&mut container).unwrap();

    assert_eq!(
        Some(DecoratedService {
            id: "foo".to_string(),
            renamed_id: Some("foo_inner".to_string()),
            priority: 5,
            invalid_behavior: None,
        }),
        container
            .get_definition("child1")
            .unwrap()
            .decorated_service()
    );
}

#[test]
fn testDecoratedServiceCopiesDeprecatedStatusFromParent() {
    let mut container = ContainerBuilder::new();
    container
        .register("deprecated_parent", None)
        .unwrap()
        .set_deprecated("vendor/package", "1.1", "%service_id%")
        .unwrap();

    container
        .set_definition(
            "decorated_deprecated_parent",
            Definition::child("deprecated_parent"),
        )
        .unwrap();

    process(&mut container).unwrap();

    assert!(
        container
            .get_definition("decorated_deprecated_parent")
            .unwrap()
            .is_deprecated()
    );
}

#[test]
fn testProcessResolvesAliases() {
    let mut container = ContainerBuilder::new();

    container.register("parent", Some("ParentClass")).unwrap();
    container.set_alias("parent_alias", "parent").unwrap();
    container
        .set_definition("child", Definition::child("parent_alias"))
        .unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert_eq!(Some("ParentClass".to_string()), def.class());
}

#[test]
fn testProcessSetsArguments() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("ParentClass"))
        .unwrap()
        .set_arguments(php_array![0]);
    let child = Definition::child("parent");
    child.set_arguments(php_array![0 => 1, "index_0" => 2, "foo" => 3]);
    container.set_definition("child", child).unwrap();

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert_eq!(php_array![0 => 2, 1 => 1, "foo" => 3], def.arguments());
}

#[test]
fn testBindings() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("stdClass"))
        .unwrap()
        .set_bindings(&php_array!["a" => "1", "b" => "2"]);

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap()
        .set_bindings(&php_array!["b" => "B", "c" => "C"]);

    process(&mut container).unwrap();

    let mut bindings = PhpArray::new();
    for (k, v) in container.get_definition("child").unwrap().bindings().iter() {
        let value = BoundArgument::from_value(v)
            .unwrap()
            .values()
            .get(&Key::Int(0))
            .cloned()
            .unwrap();
        bindings.insert(k.clone(), value);
    }
    assert_eq!(php_array!["b" => "B", "c" => "C", "a" => "1"], bindings);
}

#[test]
fn testSetAutoconfiguredOnServiceIsParent() {
    let mut container = ContainerBuilder::new();

    container
        .register("parent", Some("stdClass"))
        .unwrap()
        .set_autoconfigured(true);

    container
        .set_definition("child1", Definition::child("parent"))
        .unwrap();

    process(&mut container).unwrap();

    assert!(
        !container
            .get_definition("child1")
            .unwrap()
            .is_autoconfigured()
    );
}

#[test]
fn testProcessDetectsChildDefinitionIndirectCircularReference() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap();

    container
        .set_definition("b", Definition::child("a"))
        .unwrap();
    container
        .set_definition("c", Definition::child("b"))
        .unwrap();
    container
        .set_definition("a", Definition::child("c"))
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert!(
        e.message().starts_with(
            "Circular reference detected for service \"c\", path: \"c -> b -> a -> c\"."
        ),
        "{}",
        e.message()
    );
}

#[test]
fn testProcessCopiesSyntheticStatus() {
    let mut container = ContainerBuilder::new();

    container.register("parent", None).unwrap();

    container
        .set_definition("child", Definition::child("parent"))
        .unwrap()
        .set_synthetic(true);

    process(&mut container).unwrap();

    let def = container.get_definition("child").unwrap();
    assert!(def.is_synthetic());
}

/// Not upstream: a missing parent's error gets the service prefix.
#[test]
fn missing_parent_is_prefixed_with_the_service_id() {
    let mut container = ContainerBuilder::new();
    container
        .set_definition("child", Definition::child("missing"))
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "Service \"child\": Parent definition \"missing\" does not exist.",
        e.message()
    );
}
