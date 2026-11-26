//! Tests/Loader/FileLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; a data provider's rows
//! run in a loop. `TestFileLoader` is a Rust loader embedding
//! `FileLoader`. Fixture classes are class metadata
//! (`loader_support::registry()`, plus `PrototypeTaggedPriority` here).
//! `assertSame()` of two objects compares handles. Tests skip, with a
//! note, when `upstream/` is not fetched.
//!
//! Not ported, by reason:
//!
//! - needs `PhpFileLoader` (not ported): testImportWithGlobPattern
//! - needs PHP object creation (`$container->get()` of a definition):
//!   testRegisterClassesDoesNotDuplicateAutoconfigurationFromAbstractTypes,
//!   testRegisterClassesTwiceDoesNotDuplicateAutoconfigurationFromAbstractTypes
//!
//! Upstream: symfony/dependency-injection Tests/Loader/FileLoaderTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::unnecessary_literal_bound,
    clippy::too_many_lines,
    clippy::type_complexity
)]

mod loader_support;

use std::sync::{Arc, Weak};

use loader_support::{attr, f};
use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::loader::{
    FileLoader as BaseFileLoader, LoaderInterface, LoaderResolverInterface,
};
use symfony_config::{Error as ConfigError, FileLocator};
use symfony_dependency_injection::argument::{BoundArgument, IteratorArgument};
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, RegisterAutoconfigureAttributesPass,
};
use symfony_dependency_injection::loader::{FileLoader, SharedContainer, shared};
use symfony_dependency_injection::{ContainerBuilder, Definition, Error, Reference};

const ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\";

/// `TestFileLoader`: a `FileLoader` whose `load()` returns the resource
/// and which supports nothing.
struct TestFileLoader {
    file: FileLoader,
}

impl TestFileLoader {
    fn new(container: SharedContainer, path: &str, env: Option<&str>) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                file: FileLoader::new(
                    container,
                    Arc::new(FileLocator::new([path.to_string()])),
                    env,
                    false,
                    this,
                ),
            }
        })
    }

    /// `noAutoRegisterAliasesForSinglyImplementedInterfaces()`.
    fn no_auto_register_aliases_for_singly_implemented_interfaces(&self) {
        self.file
            .set_auto_register_aliases_for_singly_implemented_interfaces(false);
    }

    fn register_classes(
        &self,
        prototype: &Definition,
        namespace: &str,
        resource: &str,
        exclude: impl Into<Value>,
    ) -> Result<(), Error> {
        self.file
            .register_classes(prototype, namespace, resource, &exclude.into(), None)
    }
}

impl LoaderInterface for TestFileLoader {
    fn class(&self) -> &str {
        "Symfony\\Component\\DependencyInjection\\Tests\\Loader\\TestFileLoader"
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, ConfigError> {
        Ok(resource.clone())
    }

    fn supports(&self, _resource: &Value, _type: Option<&[u8]>) -> bool {
        false
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, ConfigError> {
        self.file.base().loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.base().loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&BaseFileLoader> {
        Some(self.file.base())
    }
}

/// `loader_support::registry()` with `Fixtures/PrototypeTaggedPriority`.
fn classes() -> Arc<ClassRegistry> {
    let mut r = Arc::unwrap_or_clone(loader_support::registry());
    r.add_class(
        ClassMetadata::class(&format!("{ATTRIBUTE}AutoconfigureTag"))
            .with_parent(&format!("{ATTRIBUTE}Autoconfigure")),
    );
    let interface = f("PrototypeTaggedPriority\\TaggedHandlerInterface");
    let mut handler_interface = ClassMetadata::interface(&interface);
    handler_interface.attributes = vec![attr("AutoconfigureTag", php_array!["app.handler"])];
    r.add_class(handler_interface);
    for (short, priority) in [("FirstHandler", 10), ("SecondHandler", 100)] {
        let mut handler = ClassMetadata::class(&f(&format!("PrototypeTaggedPriority\\{short}")))
            .with_interfaces(&[&interface]);
        handler.attributes = vec![attr(
            "AutoconfigureTag",
            php_array!["app.handler", php_array!["priority" => priority]],
        )];
        r.add_class(handler);
    }
    r.add_class(ClassMetadata::class("ArrayObject"));
    r.add_class(ClassMetadata::class("stdClass"));
    Arc::new(r)
}

/// A container, a `TestFileLoader` of `Tests/Fixtures`.
fn set_up(env: Option<&str>) -> Option<(SharedContainer, Arc<TestFileLoader>)> {
    let fixtures = loader_support::fixtures()?;
    let mut container = ContainerBuilder::new();
    container.set_class_registry(classes());
    let container = shared(container);
    let loader = TestFileLoader::new(container.clone(), &fixtures, env);
    Some((container, loader))
}

fn definition(container: &SharedContainer, id: &str) -> Definition {
    container.lock().unwrap().get_definition(id).unwrap()
}

fn has(container: &SharedContainer, id: &str) -> bool {
    container.lock().unwrap().has(id)
}

fn alias_ids(container: &SharedContainer) -> Vec<String> {
    container
        .lock()
        .unwrap()
        .get_aliases()
        .keys()
        .map(ToString::to_string)
        .collect()
}

fn prototype() -> Definition {
    Definition::new(None, PhpArray::new())
}

fn autoconfigured() -> Definition {
    let definition = prototype();
    definition.set_autoconfigured(true);
    definition
}

#[track_caller]
fn assert_equals(expected: impl Into<Value>, actual: impl Into<Value>) {
    let (expected, actual) = (expected.into(), actual.into());
    assert!(
        phpunit_equals(&expected, &actual),
        "expected: {expected:?}\n  actual: {actual:?}"
    );
}

/// `$a === $b`: the same object handle.
fn same(a: &Value, b: &Value) -> bool {
    if let (Some(a), Some(b)) = (Definition::from_value(a), Definition::from_value(b)) {
        return a.ptr_eq(&b);
    }
    if let (Some(a), Some(b)) = (
        IteratorArgument::from_value(a),
        IteratorArgument::from_value(b),
    ) {
        return a.ptr_eq(&b);
    }
    if let (Some(a), Some(b)) = (BoundArgument::from_value(a), BoundArgument::from_value(b)) {
        return a.ptr_eq(&b);
    }
    a.identical(b)
}

fn at(array: &PhpArray, key: impl Into<Key>) -> Value {
    array.get(&key.into()).unwrap().clone()
}

fn bound(value: impl Into<Value>) -> Value {
    BoundArgument::new(value.into(), true, 0, None).to_value()
}

fn proto(short: &str) -> String {
    f(&format!("Prototype\\{short}"))
}

const PROTOTYPE_NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\";
const SUB_NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\Sub\\";
const AS_ALIAS_NS: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\PrototypeAsAlias\\";

#[test]
fn testRegisterClasses() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    container
        .lock()
        .unwrap()
        .set_parameter("sub_dir", "Sub")
        .unwrap();
    loader.no_auto_register_aliases_for_singly_implemented_interfaces();

    loader
        .register_classes(&prototype(), SUB_NS, "Prototype/%sub_dir%/*", Value::Null)
        .unwrap();
    // loading twice should not be an issue
    loader
        .register_classes(&prototype(), SUB_NS, "Prototype/%sub_dir%/*", Value::Null)
        .unwrap();
    loader
        .file
        .register_aliases_for_singly_implemented_interfaces()
        .unwrap();

    let ids: Vec<String> = container
        .lock()
        .unwrap()
        .get_definitions()
        .keys()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        vec![
            "service_container".to_string(),
            proto("Sub\\Bar"),
            format!(".abstract.{}", proto("Sub\\BarInterface")),
        ],
        ids
    );
    assert_eq!(vec![proto("Sub\\BarInterface")], alias_ids(&container));
}

#[test]
fn testRegisterClassesSharesImmutablePrototypeParts() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    loader.no_auto_register_aliases_for_singly_implemented_interfaces();

    let prototype = prototype();
    prototype.set_arguments(php_array![Reference::new("foo")]);
    prototype.set_bindings(&php_array![
        "string $foo" => bound("bar"),
        "Bar $baz" => bound(Reference::new("baz")),
    ]);

    loader
        .register_classes(&prototype, SUB_NS, "Prototype/Sub/*", Value::Null)
        .unwrap();

    let bar = definition(&container, &proto("Sub\\Bar"));
    let bar_interface = definition(
        &container,
        &format!(".abstract.{}", proto("Sub\\BarInterface")),
    );

    assert!(same(
        &at(&prototype.arguments(), 0),
        &at(&bar.arguments(), 0)
    ));
    assert!(same(
        &at(&bar.arguments(), 0),
        &at(&bar_interface.arguments(), 0)
    ));

    assert!(same(
        &at(&prototype.bindings(), "string $foo"),
        &at(&bar.bindings(), "string $foo")
    ));
    assert!(same(
        &at(&bar.bindings(), "Bar $baz"),
        &at(&bar_interface.bindings(), "Bar $baz")
    ));
}

#[test]
fn testRegisterClassesDeepClonesMutablePrototypeParts() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    loader.no_auto_register_aliases_for_singly_implemented_interfaces();

    let prototype = prototype();
    prototype.set_arguments(php_array![Definition::with_class("stdClass").to_value()]);
    prototype
        .add_method_call(
            "setFoo",
            php_array![IteratorArgument::tagged(
                "foo",
                None,
                false,
                Vec::new(),
                true
            )],
            false,
        )
        .unwrap();
    prototype.set_bindings(&php_array![
        "string $foo" => bound("bar"),
        "iterable $bar" => bound(IteratorArgument::tagged("bar", None, false, Vec::new(), true)),
    ]);

    loader
        .register_classes(&prototype, SUB_NS, "Prototype/Sub/*", Value::Null)
        .unwrap();

    let bar = definition(&container, &proto("Sub\\Bar"));
    let bar_interface = definition(
        &container,
        &format!(".abstract.{}", proto("Sub\\BarInterface")),
    );

    assert!(!same(
        &at(&prototype.arguments(), 0),
        &at(&bar.arguments(), 0)
    ));
    assert!(!same(
        &at(&bar.arguments(), 0),
        &at(&bar_interface.arguments(), 0)
    ));
    assert_equals(prototype.arguments(), bar.arguments());

    let first_call_arg = |d: &Definition| {
        let Value::Array(call) = at(&d.method_calls(), 0) else {
            panic!("not a call");
        };
        let Value::Array(args) = at(&call, 1) else {
            panic!("no arguments");
        };
        at(&args, 0)
    };
    assert!(!same(
        &first_call_arg(&bar),
        &first_call_arg(&bar_interface)
    ));
    assert_equals(prototype.method_calls(), bar.method_calls());

    assert!(!same(
        &at(&bar.bindings(), "string $foo"),
        &at(&bar_interface.bindings(), "string $foo")
    ));
    assert_equals(prototype.bindings(), bar.bindings());
}

#[test]
fn testRegisterClassesWithExclude() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    container
        .lock()
        .unwrap()
        .set_parameter("other_dir", "OtherDir")
        .unwrap();

    loader
        .register_classes(
            &prototype(),
            PROTOTYPE_NS,
            "Prototype/*",
            // load everything, except OtherDir/AnotherSub & Foo.php
            "Prototype/{%other_dir%/AnotherSub,Foo.php,StaticConstructor}",
        )
        .unwrap();

    let bar = proto("Sub\\Bar");
    let baz = proto("OtherDir\\Baz");
    let foo = proto("Foo");
    let another_sub = proto("OtherDir\\AnotherSub");
    assert!(!definition(&container, &bar).is_abstract());
    assert!(!definition(&container, &baz).is_abstract());
    assert!(definition(&container, &foo).is_abstract());
    assert!(definition(&container, &another_sub).is_abstract());

    assert!(!definition(&container, &bar).has_tag("container.excluded"));
    assert!(!definition(&container, &baz).has_tag("container.excluded"));
    assert!(definition(&container, &foo).has_tag("container.excluded"));
    assert!(definition(&container, &another_sub).has_tag("container.excluded"));

    assert_eq!(vec![proto("Sub\\BarInterface")], alias_ids(&container));

    loader
        .register_classes(
            &prototype(),
            PROTOTYPE_NS,
            "Prototype/*",
            "Prototype/NotExistingDir",
        )
        .unwrap();
}

#[test]
fn testRegisterClassesWithExcludeAttribute() {
    for autoconfigure in [true, false] {
        let Some((container, loader)) = set_up(None) else {
            return;
        };
        let prototype = prototype();
        prototype.set_autoconfigured(autoconfigure);
        loader
            .register_classes(
                &prototype,
                "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Utils\\",
                "Utils/*",
                Value::Null,
            )
            .unwrap();

        assert_eq!(
            autoconfigure,
            definition(&container, &f("Utils\\NotAService")).has_tag("container.excluded")
        );
    }
}

#[test]
fn testRegisterClassesWithExcludeAsArray() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    container
        .lock()
        .unwrap()
        .set_parameter("sub_dir", "Sub")
        .unwrap();
    loader
        .register_classes(
            &prototype(),
            PROTOTYPE_NS,
            "Prototype/*",
            php_array![
                "Prototype/%sub_dir%",
                "Prototype/OtherDir/AnotherSub/DeeperBaz.php",
            ],
        )
        .unwrap();

    assert!(has(&container, &proto("Foo")));
    assert!(has(&container, &proto("OtherDir\\Baz")));
    assert!(!has(&container, &proto("Sub\\Bar")));
    let deeper_baz = proto("OtherDir\\AnotherSub\\DeeperBaz");
    assert!(has(&container, &deeper_baz));
    assert!(definition(&container, &deeper_baz).has_tag("container.excluded"));
}

#[test]
fn testNestedRegisterClasses() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };

    loader
        .register_classes(
            &autoconfigured(),
            PROTOTYPE_NS,
            "Prototype/*",
            "Prototype/{StaticConstructor}",
        )
        .unwrap();

    assert!(has(&container, &proto("Sub\\Bar")));
    assert!(has(&container, &proto("OtherDir\\Baz")));
    assert!(has(&container, &proto("Foo")));
    assert!(has(&container, &proto("NotFoo")));

    assert_eq!(vec![proto("FooInterface")], alias_ids(&container));

    let alias = container
        .lock()
        .unwrap()
        .get_alias(&proto("FooInterface"))
        .unwrap();
    assert_eq!(proto("Foo"), alias.to_string());
    assert!(!alias.is_public());
    assert!(alias.is_private());

    let mut c = container.lock().unwrap();
    RegisterAutoconfigureAttributesPass.process(&mut c).unwrap();

    let expected = Definition::child("");
    expected.add_tag("foo", PhpArray::new());
    let instanceof = c.get_autoconfigured_instanceof();
    assert_eq!(1, instanceof.len());
    assert_equals(
        expected.to_value(),
        instanceof.get(&proto("FooInterface")).unwrap().to_value(),
    );
}

#[test]
fn testRegisterClassesWithAbstractClassesAndAutoconfigure() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };

    loader
        .register_classes(
            &autoconfigured(),
            PROTOTYPE_NS,
            "Prototype/*",
            "Prototype/{StaticConstructor}",
        )
        .unwrap();

    let definition = definition(&container, &format!(".abstract.{}", proto("AbstractClass")));
    assert!(definition.is_abstract());
    assert!(definition.has_tag("container.excluded"));
    assert!(definition.is_autoconfigured());
}

#[test]
fn testRegisterClassesKeepsTagAttributesFromTheClassOverThoseFromTheInterface() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };

    let prototype = autoconfigured();
    prototype.set_public(true);
    loader
        .register_classes(
            &prototype,
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\PrototypeTaggedPriority\\",
            "PrototypeTaggedPriority/*",
            Value::Null,
        )
        .unwrap();
    let mut c = container.lock().unwrap();
    c.register("handlers", Some("ArrayObject"))
        .unwrap()
        .set_public(true)
        .add_argument(IteratorArgument::tagged(
            "app.handler",
            None,
            false,
            Vec::new(),
            true,
        ));

    c.compile(false).unwrap();

    let first = f("PrototypeTaggedPriority\\FirstHandler");
    let second = f("PrototypeTaggedPriority\\SecondHandler");
    assert_eq!(
        php_array![php_array!["priority" => 10], php_array![]],
        c.get_definition(&first).unwrap().tag("app.handler")
    );
    assert_eq!(
        php_array![php_array!["priority" => 100], php_array![]],
        c.get_definition(&second).unwrap().tag("app.handler")
    );

    let argument = c.get_definition("handlers").unwrap().argument(0).unwrap();
    let handlers: Vec<String> = IteratorArgument::from_value(&argument)
        .unwrap()
        .values()
        .iter()
        .map(|(_, v)| Reference::from_value(v).unwrap().to_string())
        .collect();
    assert_eq!(vec![second, first], handlers);
}

#[test]
fn testMissingParentClass() {
    let Some((container, loader)) = set_up(Some("test")) else {
        return;
    };
    container
        .lock()
        .unwrap()
        .set_parameter("bad_classes_dir", "BadClasses")
        .unwrap();

    loader
        .register_classes(
            &autoconfigured(),
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\BadClasses\\",
            "Prototype/%bad_classes_dir%/*",
            Value::Null,
        )
        .unwrap();

    let missing_parent = proto("BadClasses\\MissingParent");
    assert!(has(&container, &missing_parent));

    // {Class "?…\MissingClass"? not found}
    let error = definition(&container, &missing_parent).errors()[0].clone();
    let class = proto("BadClasses\\MissingClass");
    assert!(
        error.contains(&format!("Class \"{class}\" not found"))
            || error.contains(&format!("Class {class} not found")),
        "{error}"
    );
}

#[test]
fn testClassIsSetEvenWhenDefinitionHasErrors() {
    let Some((container, loader)) = set_up(Some("test")) else {
        return;
    };
    container
        .lock()
        .unwrap()
        .set_parameter("bad_classes_dir", "BadClasses")
        .unwrap();

    loader
        .register_classes(
            &autoconfigured(),
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\BadClasses\\",
            "Prototype/%bad_classes_dir%/*",
            Value::Null,
        )
        .unwrap();

    let missing_parent = proto("BadClasses\\MissingParent");
    let definition = definition(&container, &missing_parent);

    // The class should be set even when the definition has errors
    assert_eq!(Some(missing_parent), definition.class());
    assert_ne!(Vec::<String>::new(), definition.errors());
}

#[test]
fn testRegisterClassesWithBadPrefix() {
    let Some((_, loader)) = set_up(None) else {
        return;
    };

    // the Sub is missing from namespace prefix
    let e = loader
        .register_classes(&prototype(), PROTOTYPE_NS, "Prototype/Sub/*", Value::Null)
        .unwrap_err();
    assert!(e.instance_of(&loader_support::di("InvalidArgumentException")));
    let message = e.message();
    assert!(
        message.starts_with(
            "Expected to find class \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\Bar\" in file \""
        ),
        "{message}"
    );
    assert!(
        message.contains("\" while importing services from resource \"Prototype/Sub/*\", but it was not found! Check the namespace prefix used with the resource"),
        "{message}"
    );
}

#[test]
fn testRegisterClassesWithIncompatibleExclude() {
    let Some((_, loader)) = set_up(None) else {
        return;
    };

    let e = loader
        .register_classes(&prototype(), PROTOTYPE_NS, "Prototype/*", "yaml/*")
        .unwrap_err();
    assert!(e.instance_of(&loader_support::di("InvalidArgumentException")));
    assert!(
        e.message().contains("Invalid \"exclude\" pattern when importing classes for \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\\": make sure your \"exclude\" pattern (yaml/*) is a subset of the \"resource\" pattern (Prototype/*)"),
        "{}",
        e.message()
    );
}

#[test]
fn testExcludeTrailingSlashConsistency() {
    let another_sub = proto("OtherDir\\AnotherSub");
    let deeper_baz = proto("OtherDir\\AnotherSub\\DeeperBaz");
    for (exclude, excluded_id) in [
        ("Prototype/OtherDir/AnotherSub/", &another_sub),
        ("Prototype/OtherDir/AnotherSub", &another_sub),
        ("Prototype/OtherDir/AnotherSub/*", &deeper_baz),
        ("Prototype/*/AnotherSub", &another_sub),
        ("Prototype/*/AnotherSub/", &another_sub),
        ("Prototype/*/AnotherSub/*", &deeper_baz),
        ("Prototype/OtherDir/AnotherSub/DeeperBaz.php", &deeper_baz),
    ] {
        let Some((container, loader)) = set_up(None) else {
            return;
        };
        loader
            .register_classes(&prototype(), PROTOTYPE_NS, "Prototype/*", exclude)
            .unwrap();

        assert!(has(&container, &proto("Foo")), "{exclude}");
        assert!(has(&container, excluded_id), "{exclude}");
        assert!(
            definition(&container, excluded_id).has_tag("container.excluded"),
            "{exclude}"
        );
    }
}

#[test]
fn testRegisterClassesWithWhenEnv() {
    for (env, expected) in [
        (Some("prod"), false),
        (Some("dev"), false),
        (Some("bar"), true),
        (None, false),
    ] {
        let Some((container, loader)) = set_up(env) else {
            return;
        };
        loader
            .register_classes(
                &autoconfigured(),
                PROTOTYPE_NS,
                "Prototype/{Foo.php}",
                Value::Null,
            )
            .unwrap();

        assert_eq!(
            expected,
            definition(&container, &proto("Foo")).has_tag("container.excluded"),
            "{env:?}"
        );
    }
}

#[test]
fn testRegisterWithNotWhenAttributes() {
    for (env, expected_not_foo_exclusion) in [("dev", true), ("prod", true), ("test", false)] {
        let Some((container, loader)) = set_up(Some(env)) else {
            return;
        };

        loader
            .register_classes(
                &autoconfigured(),
                PROTOTYPE_NS,
                "Prototype/*",
                "Prototype/BadAttributes/*",
            )
            .unwrap();

        assert!(has(&container, &proto("NotFoo")));
        assert_eq!(
            expected_not_foo_exclusion,
            definition(&container, &proto("NotFoo")).has_tag("container.excluded"),
            "{env}"
        );
    }
}

#[test]
fn testRegisterThrowsWithBothWhenAndNotWhenAttribute() {
    let Some((_, loader)) = set_up(Some("dev")) else {
        return;
    };

    let e = loader
        .register_classes(
            &autoconfigured(),
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\BadAttributes\\",
            "Prototype/BadAttributes/*",
            Value::Null,
        )
        .unwrap_err();
    assert!(e.instance_of(&loader_support::di("LogicException")));
    assert!(e.message().contains("The \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\BadAttributes\\WhenNotWhenFoo\" class cannot have both #[When] and #[WhenNot] attributes."));
}

fn as_alias(short: &str) -> String {
    f(&format!("PrototypeAsAlias\\{short}"))
}

#[test]
fn testRegisterClassesWithAsAlias() {
    let foo = as_alias("AliasFooInterface");
    let bar = as_alias("AliasBarInterface");
    // (alias, target, public)
    let rows: Vec<(&str, &str, Vec<(String, String, bool)>, Option<&str>)> = vec![
        (
            "Private",
            "PrototypeAsAlias/{WithAsAlias,AliasFooInterface}.php",
            vec![(foo.clone(), as_alias("WithAsAlias"), false)],
            None,
        ),
        (
            "PrivateCustomAlias",
            "PrototypeAsAlias/{WithCustomAsAlias,AliasFooInterface}.php",
            vec![(foo.clone(), as_alias("WithCustomAsAlias"), false)],
            Some("prod"),
        ),
        (
            "PrivateCustomAliasNoMatch",
            "PrototypeAsAlias/{WithCustomAsAlias,AliasFooInterface}.php",
            vec![],
            Some("dev"),
        ),
        (
            "Interface",
            "PrototypeAsAlias/{WithAsAliasInterface,AliasFooInterface}.php",
            vec![(foo.clone(), as_alias("WithAsAliasInterface"), false)],
            None,
        ),
        (
            "Multiple",
            "PrototypeAsAlias/{WithAsAliasMultiple,AliasFooInterface}.php",
            vec![
                (foo.clone(), as_alias("WithAsAliasMultiple"), true),
                (
                    "some-alias".to_string(),
                    as_alias("WithAsAliasMultiple"),
                    false,
                ),
            ],
            None,
        ),
        (
            "Multiple with id",
            "PrototypeAsAlias/{WithAsAliasIdMultipleInterface,AliasBarInterface,AliasFooInterface}.php",
            vec![
                (
                    bar.clone(),
                    as_alias("WithAsAliasIdMultipleInterface"),
                    false,
                ),
                (
                    foo.clone(),
                    as_alias("WithAsAliasIdMultipleInterface"),
                    false,
                ),
            ],
            None,
        ),
        (
            "Dev-env specific",
            "PrototypeAsAlias/WithAsAlias*Env.php",
            vec![
                (foo.clone(), as_alias("WithAsAliasDevEnv"), false),
                (bar.clone(), as_alias("WithAsAliasBothEnv"), false),
            ],
            Some("dev"),
        ),
        (
            "Prod-env specific",
            "PrototypeAsAlias/WithAsAlias*Env.php",
            vec![
                (foo.clone(), as_alias("WithAsAliasProdEnv"), false),
                (bar.clone(), as_alias("WithAsAliasBothEnv"), false),
            ],
            Some("prod"),
        ),
        (
            "Test-env specific",
            "PrototypeAsAlias/WithAsAlias*Env.php",
            vec![],
            Some("test"),
        ),
    ];
    for (name, resource, mut expected_aliases, env) in rows {
        let Some((container, loader)) = set_up(env) else {
            return;
        };
        loader
            .register_classes(&autoconfigured(), AS_ALIAS_NS, resource, Value::Null)
            .unwrap();

        // assertEquals() of two arrays of Alias objects: keys in any order.
        let mut aliases: Vec<(String, String, bool)> = container
            .lock()
            .unwrap()
            .get_aliases()
            .iter()
            .map(|(id, alias)| {
                assert!(!alias.is_deprecated(), "{name}");
                (id.to_string(), alias.id(), alias.is_public())
            })
            .collect();
        aliases.sort();
        expected_aliases.sort();
        assert_eq!(expected_aliases, aliases, "{name}");
    }
}

#[test]
fn testRegisterClassesWithDuplicatedAsAlias() {
    let message = format!(
        "The \"{}\" alias has already been defined with the #[AsAlias] attribute in \"{}\".",
        as_alias("AliasFooInterface"),
        as_alias("WithAsAlias")
    );
    for (name, resource) in [
        (
            "Duplicated",
            "PrototypeAsAlias/{WithAsAlias,WithAsAliasDuplicate,AliasFooInterface}.php",
        ),
        (
            "Interface duplicated",
            "PrototypeAsAlias/{WithAsAliasInterface,WithAsAlias,AliasFooInterface}.php",
        ),
    ] {
        let Some((_, loader)) = set_up(None) else {
            return;
        };

        let e = loader
            .register_classes(&autoconfigured(), AS_ALIAS_NS, resource, Value::Null)
            .unwrap_err();
        assert!(
            e.instance_of(&loader_support::di("LogicException")),
            "{name}"
        );
        assert!(e.message().contains(&message), "{name}: {}", e.message());
    }
}

#[test]
fn testRegisterClassesWithAsAliasAndImplementingMultipleInterfaces() {
    let Some((_, loader)) = set_up(None) else {
        return;
    };

    let e = loader
        .register_classes(
            &autoconfigured(),
            AS_ALIAS_NS,
            "PrototypeAsAlias/{WithAsAliasMultipleInterface,AliasBarInterface,AliasFooInterface}.php",
            Value::Null,
        )
        .unwrap_err();
    assert!(e.instance_of(&loader_support::di("LogicException")));
    assert!(e.message().contains("Alias cannot be automatically determined for class \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\PrototypeAsAlias\\WithAsAliasMultipleInterface\". If you have used the #[AsAlias] attribute with a class implementing multiple interfaces, add the interface you want to alias to the first parameter of #[AsAlias]."));
}

#[test]
fn testRegisterClassesWithAsAliasTarget() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };
    loader
        .register_classes(
            &autoconfigured(),
            AS_ALIAS_NS,
            "PrototypeAsAlias/{WithAsAliasTargetOne,WithAsAliasTargetTwo,AliasFooInterface}.php",
            Value::Null,
        )
        .unwrap();
    // Verify target-specific aliases are created
    let foo = as_alias("AliasFooInterface");
    let c = container.lock().unwrap();
    assert!(c.has_alias(&format!("{foo} $one")));
    assert!(c.has_alias(&format!("{foo} $two")));
    assert_eq!(
        as_alias("WithAsAliasTargetOne"),
        c.get_alias(&format!("{foo} $one")).unwrap().to_string()
    );
    assert_eq!(
        as_alias("WithAsAliasTargetTwo"),
        c.get_alias(&format!("{foo} $two")).unwrap().to_string()
    );
}

#[test]
fn testRegisterClassesWithStaticConstructor() {
    let Some((container, loader)) = set_up(None) else {
        return;
    };

    loader
        .register_classes(
            &autoconfigured(),
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\StaticConstructor\\",
            "Prototype/StaticConstructor",
            Value::Null,
        )
        .unwrap();

    assert!(has(
        &container,
        &proto("StaticConstructor\\PrototypeStaticConstructor")
    ));
}
