//! Tests/ChildDefinitionTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/ChildDefinitionTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{Value, php_array};
use symfony_dependency_injection::{Definition, ErrorKind};

#[test]
fn testConstructor() {
    let def = Definition::child("foo");
    assert_eq!(Some("foo".to_string()), def.parent());
    assert_eq!(def.changes(), [] as [String; 0]);
}

#[test]
fn testSetProperty() {
    type Get = fn(&Definition) -> Value;
    type Set = fn(&Definition) -> &Definition;
    let cases: [(Get, Set, &str); 4] = [
        (
            |d| d.class().map_or(Value::Null, Value::from),
            |d| d.set_class(Some("foo")),
            "class",
        ),
        (Definition::factory, |d| d.set_factory("foo"), "factory"),
        (
            Definition::configurator,
            |d| d.set_configurator("foo"),
            "configurator",
        ),
        (
            |d| d.file().map_or(Value::Null, Value::from),
            |d| d.set_file(Some("foo")),
            "file",
        ),
    ];
    for (getter, setter, change_key) in cases {
        let def = Definition::child("foo");
        assert!(getter(&def).is_null());
        assert!(setter(&def).ptr_eq(&def));
        assert_eq!(Value::from("foo"), getter(&def));
        assert_eq!(vec![change_key.to_string()], def.changes());
    }
}

#[test]
fn testSetPublic() {
    let def = Definition::child("foo");
    assert!(!def.is_public());
    assert!(def.set_public(true).ptr_eq(&def));
    assert!(def.is_public());
    assert_eq!(vec!["public".to_string()], def.changes());
}

#[test]
fn testSetLazy() {
    let def = Definition::child("foo");
    assert!(!def.is_lazy());
    assert!(def.set_lazy(false).ptr_eq(&def));
    assert!(!def.is_lazy());
    assert_eq!(vec!["lazy".to_string()], def.changes());
}

#[test]
fn testSetAutowired() {
    let def = Definition::child("foo");
    assert!(!def.is_autowired());
    assert!(def.set_autowired(true).ptr_eq(&def));
    assert!(def.is_autowired());
    assert_eq!(vec!["autowired".to_string()], def.changes());
}

#[test]
fn testSetArgument() {
    let def = Definition::child("foo");
    assert!(def.arguments().is_empty());
    assert!(def.replace_argument(0, "foo").unwrap().ptr_eq(&def));
    assert_eq!(php_array!["index_0" => "foo"], def.arguments());
}

#[test]
fn testReplaceArgumentShouldRequireIntegerIndex() {
    let def = Definition::child("foo");
    let e = def.replace_argument("0", "foo").unwrap_err();
    assert!(e.instance_of("InvalidArgumentException"));
}

#[test]
fn testReplaceArgument() {
    let def = Definition::child("foo");
    def.set_arguments(php_array![0 => "foo", 1 => "bar"]);
    assert_eq!(Value::from("foo"), def.argument(0).unwrap());
    assert_eq!(Value::from("bar"), def.argument(1).unwrap());

    assert!(def.replace_argument(1, "baz").unwrap().ptr_eq(&def));
    assert_eq!(Value::from("foo"), def.argument(0).unwrap());
    assert_eq!(Value::from("baz"), def.argument(1).unwrap());

    assert_eq!(
        php_array![0 => "foo", 1 => "bar", "index_1" => "baz"],
        def.arguments()
    );

    assert!(def.replace_argument("$bar", "val").unwrap().ptr_eq(&def));
    assert_eq!(Value::from("val"), def.argument("$bar").unwrap());
    assert_eq!(
        php_array![0 => "foo", 1 => "bar", "index_1" => "baz", "$bar" => "val"],
        def.arguments()
    );
}

#[test]
fn testGetArgumentShouldCheckBounds() {
    let def = Definition::child("foo");
    def.set_arguments(php_array![0 => "foo"]);
    def.replace_argument(0, "foo").unwrap();
    let e = def.argument(1).unwrap_err();
    assert!(e.is(&ErrorKind::OutOfBounds));
    assert!(e.instance_of("OutOfBoundsException"));
}

#[test]
fn testAutoconfigured() {
    let def = Definition::child("foo");
    def.set_autoconfigured(true);
    assert!(def.is_autoconfigured());
}

#[test]
fn testInstanceofConditionals() {
    let conditionals = php_array!["Foo" => Definition::child("")];
    let def = Definition::child("foo");
    def.set_instanceof_conditionals(conditionals.clone());
    assert_eq!(conditionals, def.instanceof_conditionals());
}
