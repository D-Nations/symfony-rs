//! Tests/DefinitionTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's; `assertSame($def, $def->setX())`
//! (the fluent interface) is `ptr_eq`.
//!
//! Upstream: symfony/dependency-injection Tests/DefinitionTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::{
    DecoratedService, Definition, DefinitionError, ErrorKind, InvalidBehavior, Reference,
};

fn changes(keys: &[&str]) -> Vec<String> {
    keys.iter().map(ToString::to_string).collect()
}

#[test]
fn testConstructor() {
    let def = Definition::with_class("stdClass");
    assert_eq!(
        Some("stdClass".to_string()),
        def.class(),
        "__construct() takes the class name as its first argument"
    );
    assert_eq!(changes(&["class"]), def.changes());

    let def = Definition::new(Some("stdClass"), php_array!["foo"]);
    assert_eq!(
        php_array!["foo"],
        def.arguments(),
        "__construct() takes an optional array of arguments as its second argument"
    );
}

#[test]
fn testSetGetFactory() {
    let def = Definition::default();

    assert!(
        def.set_factory("foo").ptr_eq(&def),
        "->setFactory() implements a fluent interface"
    );
    assert_eq!(
        Value::from("foo"),
        def.factory(),
        "->getFactory() returns the factory"
    );

    def.set_factory("Foo::bar");
    assert_eq!(
        Value::from(php_array!["Foo", "bar"]),
        def.factory(),
        "->setFactory() converts string static method call to the array"
    );

    let reference = Value::from(Reference::new("baz"));
    def.set_factory(reference.clone());
    assert_eq!(
        Value::from(php_array![reference, "__invoke"]),
        def.factory(),
        "->setFactory() converts service reference to class invoke call"
    );

    let inner_def = Definition::default();
    def.set_factory(&inner_def);
    assert_eq!(
        Value::from(php_array![&inner_def, "__invoke"]),
        def.factory(),
        "->setFactory() converts inline definition to class invoke call"
    );
    assert_eq!(changes(&["factory"]), def.changes());
}

#[test]
fn testSetGetClass() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.set_class(Some("foo")).ptr_eq(&def),
        "->setClass() implements a fluent interface"
    );
    assert_eq!(
        Some("foo".to_string()),
        def.class(),
        "->getClass() returns the class name"
    );
}

#[test]
fn testSetGetDecoratedService() {
    let decorated = |id: &str, renamed: Option<&str>, priority, behavior| {
        Some(DecoratedService {
            id: id.to_string(),
            renamed_id: renamed.map(str::to_string),
            priority,
            invalid_behavior: behavior,
        })
    };

    let def = Definition::with_class("stdClass");
    assert_eq!(None, def.decorated_service());
    def.set_decorated_service(Some("foo"), Some("foo.renamed"), 5, InvalidBehavior::Null)
        .unwrap();
    assert_eq!(
        decorated("foo", Some("foo.renamed"), 5, Some(InvalidBehavior::Null)),
        def.decorated_service()
    );
    assert_eq!(
        php_array!["foo", "foo.renamed", 5, 2],
        def.decorated_service().unwrap().to_array()
    );
    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)
        .unwrap();
    assert_eq!(None, def.decorated_service());

    let def = Definition::with_class("stdClass");
    assert_eq!(None, def.decorated_service());
    def.set_decorated_service(
        Some("foo"),
        Some("foo.renamed"),
        5,
        InvalidBehavior::Exception,
    )
    .unwrap();
    assert_eq!(
        php_array!["foo", "foo.renamed", 5],
        def.decorated_service().unwrap().to_array()
    );
    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)
        .unwrap();
    assert_eq!(None, def.decorated_service());

    let def = Definition::with_class("stdClass");
    assert_eq!(None, def.decorated_service());
    def.set_decorated_service(
        Some("foo"),
        Some("foo.renamed"),
        0,
        InvalidBehavior::Exception,
    )
    .unwrap();
    assert_eq!(
        php_array!["foo", "foo.renamed", 0],
        def.decorated_service().unwrap().to_array()
    );
    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)
        .unwrap();
    assert_eq!(None, def.decorated_service());

    let def = Definition::with_class("stdClass");
    def.set_decorated_service(Some("foo"), None, 0, InvalidBehavior::Exception)
        .unwrap();
    assert_eq!(
        php_array!["foo", Value::Null, 0],
        def.decorated_service().unwrap().to_array()
    );
    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)
        .unwrap();
    assert_eq!(None, def.decorated_service());

    let def = Definition::with_class("stdClass");
    let e = def
        .set_decorated_service(Some("foo"), Some("foo"), 0, InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.instance_of("InvalidArgumentException"));
    assert_eq!(
        "The decorated service inner name for \"foo\" must be different than the service name itself.",
        e.message()
    );
}

#[test]
fn testArguments() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.set_arguments(php_array!["foo"]).ptr_eq(&def),
        "->setArguments() implements a fluent interface"
    );
    assert_eq!(
        php_array!["foo"],
        def.arguments(),
        "->getArguments() returns the arguments"
    );
    assert!(
        def.add_argument("bar").ptr_eq(&def),
        "->addArgument() implements a fluent interface"
    );
    assert_eq!(
        php_array!["foo", "bar"],
        def.arguments(),
        "->addArgument() adds an argument"
    );
}

fn calls(list: &[(&str, &str, bool)]) -> PhpArray {
    PhpArray::list(list.iter().map(|(method, arg, clone)| {
        let mut call = php_array![*method, php_array![*arg]];
        if *clone {
            let _ = call.push(Value::Bool(true));
        }
        Value::from(call)
    }))
}

/// The calls without their keys, as `assertEquals()` compares lists.
fn listed(calls: &PhpArray) -> PhpArray {
    PhpArray::list(calls.values().cloned())
}

#[test]
fn testMethodCalls() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.set_method_calls(&calls(&[("foo", "foo", false)]))
            .unwrap()
            .ptr_eq(&def),
        "->setMethodCalls() implements a fluent interface"
    );
    assert_eq!(
        calls(&[("foo", "foo", false)]),
        def.method_calls(),
        "->getMethodCalls() returns the methods to call"
    );
    assert!(
        def.add_method_call("bar", php_array!["bar"], false)
            .unwrap()
            .ptr_eq(&def),
        "->addMethodCall() implements a fluent interface"
    );
    assert_eq!(
        calls(&[("foo", "foo", false), ("bar", "bar", false)]),
        def.method_calls(),
        "->addMethodCall() adds a method to call"
    );
    assert!(
        def.add_method_call("foobar", php_array!["foobar"], true)
            .unwrap()
            .ptr_eq(&def),
        "->addMethodCall() implements a fluent interface with third parameter"
    );
    assert_eq!(
        calls(&[
            ("foo", "foo", false),
            ("bar", "bar", false),
            ("foobar", "foobar", true)
        ]),
        def.method_calls(),
        "->addMethodCall() adds a method to call"
    );
    assert!(
        def.has_method_call("bar"),
        "->hasMethodCall() returns true if first argument is a method to call registered"
    );
    assert!(
        !def.has_method_call("no_registered"),
        "->hasMethodCall() returns false if first argument is not a method to call registered"
    );
    assert!(
        def.remove_method_call("bar").ptr_eq(&def),
        "->removeMethodCall() implements a fluent interface"
    );
    assert!(
        def.has_method_call("foobar"),
        "->hasMethodCall() returns true if first argument is a method to call registered"
    );
    assert!(
        def.remove_method_call("foobar").ptr_eq(&def),
        "->removeMethodCall() implements a fluent interface"
    );
    assert_eq!(
        calls(&[("foo", "foo", false)]),
        listed(&def.method_calls()),
        "->removeMethodCall() removes a method to call"
    );
    assert!(
        def.set_method_calls(&calls(&[("foobar", "foobar", true)]))
            .unwrap()
            .ptr_eq(&def),
        "->setMethodCalls() implements a fluent interface with third parameter"
    );
    assert_eq!(
        calls(&[("foobar", "foobar", true)]),
        def.method_calls(),
        "->addMethodCall() adds a method to call"
    );
}

#[test]
fn testExceptionOnEmptyMethodCall() {
    let def = Definition::with_class("stdClass");
    let e = def.add_method_call("", PhpArray::new(), false).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!("Method name cannot be empty.", e.message());
}

#[test]
fn testSetGetFile() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.set_file(Some("foo")).ptr_eq(&def),
        "->setFile() implements a fluent interface"
    );
    assert_eq!(
        Some("foo".to_string()),
        def.file(),
        "->getFile() returns the file to include"
    );
}

#[test]
fn testSetIsShared() {
    let def = Definition::with_class("stdClass");
    assert!(def.is_shared(), "->isShared() returns true by default");
    assert!(
        def.set_shared(false).ptr_eq(&def),
        "->setShared() implements a fluent interface"
    );
    assert!(
        !def.is_shared(),
        "->isShared() returns false if the instance must not be shared"
    );
}

#[test]
fn testSetIsPublic() {
    let def = Definition::with_class("stdClass");
    assert!(!def.is_public(), "->isPublic() returns false by default");
    assert!(
        def.set_public(true).ptr_eq(&def),
        "->setPublic() implements a fluent interface"
    );
    assert!(
        def.is_public(),
        "->isPublic() returns true if the service is public."
    );
}

#[test]
fn testSetIsSynthetic() {
    let def = Definition::with_class("stdClass");
    assert!(
        !def.is_synthetic(),
        "->isSynthetic() returns false by default"
    );
    assert!(
        def.set_synthetic(true).ptr_eq(&def),
        "->setSynthetic() implements a fluent interface"
    );
    assert!(
        def.is_synthetic(),
        "->isSynthetic() returns true if the service is synthetic."
    );
}

#[test]
fn testSetIsLazy() {
    let def = Definition::with_class("stdClass");
    assert!(!def.is_lazy(), "->isLazy() returns false by default");
    assert!(
        def.set_lazy(true).ptr_eq(&def),
        "->setLazy() implements a fluent interface"
    );
    assert!(
        def.is_lazy(),
        "->isLazy() returns true if the service is lazy."
    );
}

#[test]
fn testSetIsAbstract() {
    let def = Definition::with_class("stdClass");
    assert!(
        !def.is_abstract(),
        "->isAbstract() returns false by default"
    );
    assert!(
        def.set_abstract(true).ptr_eq(&def),
        "->setAbstract() implements a fluent interface"
    );
    assert!(
        def.is_abstract(),
        "->isAbstract() returns true if the instance must not be public."
    );
}

#[test]
fn testSetIsDeprecated() {
    let def = Definition::with_class("stdClass");
    assert!(
        !def.is_deprecated(),
        "->isDeprecated() returns false by default"
    );
    assert!(
        def.set_deprecated("vendor/package", "1.1", "%service_id%")
            .unwrap()
            .ptr_eq(&def),
        "->setDeprecated() implements a fluent interface"
    );
    assert!(
        def.is_deprecated(),
        "->isDeprecated() returns true if the instance should not be used anymore."
    );

    let deprecation = def.deprecation("deprecated_service").unwrap();
    assert_eq!(
        "deprecated_service", deprecation.message,
        "->getDeprecation() should return an array with the formatted message template"
    );
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
}

#[test]
fn testSetDeprecatedWithInvalidDeprecationTemplate() {
    for message in [
        "invalid \r message %service_id%",
        "invalid \n message %service_id%",
        "invalid */ message %service_id%",
        "this is deprecated",
    ] {
        let def = Definition::with_class("stdClass");
        let e = def
            .set_deprecated("vendor/package", "1.1", message)
            .unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument), "{message:?}");
    }
}

#[test]
fn testSetGetConfigurator() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.set_configurator("foo").ptr_eq(&def),
        "->setConfigurator() implements a fluent interface"
    );
    assert_eq!(
        Value::from("foo"),
        def.configurator(),
        "->getConfigurator() returns the configurator"
    );

    def.set_configurator("Foo::bar");
    assert_eq!(
        Value::from(php_array!["Foo", "bar"]),
        def.configurator(),
        "->setConfigurator() converts string static method call to the array"
    );

    let reference = Value::from(Reference::new("baz"));
    def.set_configurator(reference.clone());
    assert_eq!(
        Value::from(php_array![reference, "__invoke"]),
        def.configurator(),
        "->setConfigurator() converts service reference to class invoke call"
    );

    let inner_def = Definition::default();
    def.set_configurator(&inner_def);
    assert_eq!(
        Value::from(php_array![&inner_def, "__invoke"]),
        def.configurator(),
        "->setConfigurator() converts inline definition to class invoke call"
    );
}

#[test]
fn testClearTags() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.clear_tags().ptr_eq(&def),
        "->clearTags() implements a fluent interface"
    );
    def.add_tag("foo", php_array!["foo" => "bar"]);
    def.clear_tags();
    assert!(
        def.tags().is_empty(),
        "->clearTags() removes all current tags"
    );
}

#[test]
fn testClearTag() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.clear_tags().ptr_eq(&def),
        "->clearTags() implements a fluent interface"
    );

    def.add_tag("1foo1", php_array!["foo1" => "bar1"]);
    def.add_tag("2foo2", php_array!["foo2" => "bar2"]);
    def.add_tag("3foo3", php_array!["foo3" => "bar3"]);
    def.clear_tag("2foo2");
    assert!(def.has_tag("1foo1"));
    assert!(!def.has_tag("2foo2"));
    assert!(def.has_tag("3foo3"));
    def.clear_tag("1foo1");
    assert!(!def.has_tag("1foo1"));
    assert!(def.has_tag("3foo3"));
}

#[test]
fn testTags() {
    let def = Definition::with_class("stdClass");
    assert!(
        def.tag("foo").is_empty(),
        "->getTag() returns an empty array if the tag is not defined"
    );
    assert!(!def.has_tag("foo"));
    assert!(
        def.add_tag("foo", PhpArray::new()).ptr_eq(&def),
        "->addTag() implements a fluent interface"
    );
    assert!(def.has_tag("foo"));
    assert_eq!(
        php_array![PhpArray::new()],
        def.tag("foo"),
        "->getTag() returns attributes for a tag name"
    );
    def.add_tag("foo", php_array!["foo" => "bar"]);
    assert_eq!(
        php_array![PhpArray::new(), php_array!["foo" => "bar"]],
        def.tag("foo"),
        "->addTag() can adds the same tag several times"
    );
    def.add_tag("bar", php_array!["bar" => "bar"]);
    assert_eq!(
        php_array![
            "foo" => php_array![PhpArray::new(), php_array!["foo" => "bar"]],
            "bar" => php_array![php_array!["bar" => "bar"]],
        ],
        def.tags(),
        "->getTags() returns all tags"
    );
}

#[test]
fn testAddResourceTag() {
    let def = Definition::with_class("stdClass");
    def.add_resource_tag("foo", php_array!["bar" => true]);

    assert_eq!(php_array![php_array!["bar" => true]], def.tag("foo"));
    assert!(!def.is_abstract());
    assert_eq!(
        php_array![php_array!["source" => "by tag \"foo\""]],
        def.tag("container.excluded")
    );
}

#[test]
fn testSetArgument() {
    let def = Definition::with_class("stdClass");

    def.add_argument("foo");
    assert_eq!(php_array!["foo"], def.arguments());

    assert!(def.replace_argument(0, "moo").unwrap().ptr_eq(&def));
    assert_eq!(php_array!["moo"], def.arguments());

    def.add_argument("moo");
    def.replace_argument(0, "foo")
        .unwrap()
        .replace_argument(1, "bar")
        .unwrap();
    assert_eq!(php_array!["foo", "bar"], def.arguments());
}

#[test]
fn testGetArgumentShouldCheckBounds() {
    let def = Definition::with_class("stdClass");
    def.add_argument("foo");
    let e = def.argument(1).unwrap_err();
    assert!(e.instance_of("OutOfBoundsException"));
}

#[test]
fn testReplaceArgumentShouldCheckBounds() {
    let def = Definition::with_class("stdClass");
    def.add_argument("foo");
    let e = def.replace_argument(1, "bar").unwrap_err();
    assert!(e.instance_of("OutOfBoundsException"));
    assert_eq!(
        "The argument \"1\" doesn't exist in class \"stdClass\".",
        e.message()
    );
}

#[test]
fn testReplaceArgumentWithoutExistingArgumentsShouldCheckBounds() {
    let def = Definition::with_class("stdClass");
    let e = def.replace_argument(0, "bar").unwrap_err();
    assert!(e.instance_of("OutOfBoundsException"));
    assert_eq!(
        "Cannot replace arguments for class \"stdClass\" if none have been configured yet.",
        e.message()
    );
}

#[test]
fn testReplaceArgumentWithNonConsecutiveIntIndex() {
    let def = Definition::with_class("stdClass");
    def.set_arguments(php_array![1 => "foo"]);
    assert_eq!(php_array![1 => "foo"], def.arguments());

    def.replace_argument(1, "bar").unwrap();
    assert_eq!(php_array![1 => "bar"], def.arguments());
}

#[test]
fn testSetGetProperties() {
    let def = Definition::with_class("stdClass");
    assert!(def.properties().is_empty());
    assert!(def.set_properties(php_array!["foo" => "bar"]).ptr_eq(&def));
    assert_eq!(php_array!["foo" => "bar"], def.properties());
}

#[test]
fn testSetProperty() {
    let def = Definition::with_class("stdClass");
    assert!(def.properties().is_empty());
    assert!(def.set_property("foo", "bar").ptr_eq(&def));
    assert_eq!(php_array!["foo" => "bar"], def.properties());
}

#[test]
fn testAutowired() {
    let def = Definition::with_class("stdClass");
    assert!(!def.is_autowired());

    def.set_autowired(true);
    assert!(def.is_autowired());

    def.set_autowired(false);
    assert!(!def.is_autowired());
}

#[test]
fn testChangesNoChanges() {
    let def = Definition::default();
    assert!(def.changes().is_empty());
}

#[test]
fn testGetChangesWithChanges() {
    let def = Definition::new(Some("stdClass"), php_array!["fooarg"]);

    def.set_abstract(true);
    def.set_autowired(true);
    def.set_configurator("configuration_func");
    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)
        .unwrap();
    def.set_deprecated("vendor/package", "1.1", "%service_id%")
        .unwrap();
    def.set_factory("factory_func");
    def.set_file(Some("foo.php"));
    def.set_lazy(true);
    def.set_public(true);
    def.set_shared(true);
    def.set_synthetic(true);
    // changes aren't tracked for these, class or arguments
    def.set_instanceof_conditionals(PhpArray::new());
    def.add_tag("foo_tag", PhpArray::new());
    def.add_method_call("methodCall", PhpArray::new(), false)
        .unwrap();
    def.set_property("fooprop", true);
    def.set_autoconfigured(true);

    assert_eq!(
        changes(&[
            "class",
            "autowired",
            "configurator",
            "decorated_service",
            "deprecated",
            "factory",
            "file",
            "lazy",
            "public",
            "shared",
            "autoconfigured",
        ]),
        def.changes()
    );

    def.set_changes(Vec::new());
    assert!(def.changes().is_empty());
}

#[test]
fn testShouldAutoconfigure() {
    let def = Definition::with_class("stdClass");
    assert!(!def.is_autoconfigured());
    def.set_autoconfigured(true);
    assert!(def.is_autoconfigured());
}

#[test]
fn testAddError() {
    let def = Definition::with_class("stdClass");
    assert!(!def.has_errors());
    def.add_error(DefinitionError::Text("First error".into()));
    def.add_error(DefinitionError::Text("Second error".into()));
    assert_eq!(
        vec!["First error".to_string(), "Second error".to_string()],
        def.errors()
    );
}

#[test]
fn testMultipleMethodCalls() {
    let def = Definition::with_class("stdClass");

    def.add_method_call("configure", php_array!["arg1"], false)
        .unwrap();
    assert!(def.has_method_call("configure"));
    assert_eq!(1, def.method_calls().len());

    def.add_method_call("configure", php_array!["arg2"], false)
        .unwrap();
    assert!(def.has_method_call("configure"));
    assert_eq!(2, def.method_calls().len());

    def.remove_method_call("configure");
    assert!(!def.has_method_call("configure"));
}
