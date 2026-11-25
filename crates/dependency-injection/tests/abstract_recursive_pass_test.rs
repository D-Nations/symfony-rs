//! Tests/Compiler/AbstractRecursivePassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The anonymous pass classes are `Probe`, and the fixture
//! classes `Bar` and `FactoryDummy` are class metadata.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AbstractRecursivePassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value};
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, FunctionReflector, MethodMetadata, ParameterMetadata, Visibility,
};
use symfony_dependency_injection::compiler::{
    AbstractRecursivePass, CompilerPassInterface, RecursivePassState, recursive_process,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind, Reference, Result};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";

fn bar_class() -> String {
    format!("{FIXTURES}Bar")
}

fn factory_dummy_class() -> String {
    format!("{FIXTURES}FactoryDummy")
}

/// `Tests/Fixtures/Bar.php` and `Tests/Fixtures/FactoryDummy.php`.
fn fixtures() -> Arc<ClassRegistry> {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::interface(&format!("{FIXTURES}BarInterface")));
    registry.add_class(
        ClassMetadata::class(&bar_class())
            .with_interfaces(&[&format!("{FIXTURES}BarInterface")])
            .with_method(
                MethodMetadata::public("__construct").with_parameters(vec![
                    ParameterMetadata::new("quz", None).with_default(Value::Null),
                    ParameterMetadata::new("nonExistent", Some("?NonExistent"))
                        .with_default(Value::Null),
                    ParameterMetadata::new("decorated", Some(&format!("?{FIXTURES}BarInterface")))
                        .with_default(Value::Null),
                    ParameterMetadata::new("foo", Some("array"))
                        .with_default(Value::Array(PhpArray::new())),
                    ParameterMetadata::new("baz", Some("iterable"))
                        .with_default(Value::Array(PhpArray::new())),
                ]),
            )
            .with_method(
                MethodMetadata::public("create")
                    .with_static(true)
                    .with_parameters(vec![
                        ParameterMetadata::new("nonExistent", Some("?NonExistent"))
                            .with_default(Value::Null),
                        ParameterMetadata::new("factory", None).with_default(Value::Null),
                    ]),
            )
            .with_method(MethodMetadata::public("createNonStatic"))
            .with_method(
                MethodMetadata::public("createPrivateStatic")
                    .with_static(true)
                    .with_visibility(Visibility::Private),
            ),
    );
    registry.add_class(ClassMetadata::class(&format!("{FIXTURES}FactoryParent")));
    registry.add_class(
        ClassMetadata::class(&factory_dummy_class())
            .with_parent(&format!("{FIXTURES}FactoryParent"))
            .with_method(
                MethodMetadata::public("createFactory")
                    .with_static(true)
                    .with_return_type("self"),
            )
            .with_method(MethodMetadata::public("create").with_return_type("stdClass"))
            .with_method(MethodMetadata::public("createBuiltin").with_return_type("int"))
            .with_method(
                MethodMetadata::public("createSelf")
                    .with_static(true)
                    .with_return_type("self"),
            )
            .with_method(
                MethodMetadata::public("createParent")
                    .with_static(true)
                    .with_return_type("parent"),
            ),
    );
    Arc::new(registry)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

/// What the test's anonymous pass reads for the definition `foo`.
enum Read {
    Constructor,
    Method(&'static str),
}

struct Probe {
    state: RecursivePassState,
    read: Read,
    actual: Option<FunctionReflector>,
}

impl Probe {
    fn new(read: Read) -> Self {
        Self {
            state: RecursivePassState::default(),
            read,
            actual: None,
        }
    }
}

impl CompilerPassInterface for Probe {
    fn class(&self) -> &'static str {
        "AbstractRecursivePass@anonymous"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for Probe {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(definition) = Definition::from_value(&value)
            && self.state.current_id.as_deref() == Some("foo")
        {
            self.actual = Some(match self.read {
                Read::Constructor => self.get_constructor(container, &definition, true)?.unwrap(),
                Read::Method(method) => {
                    self.get_reflection_method(container, &definition, method)?
                }
            });
        }
        self.parent_process_value(container, value, is_root)
    }
}

#[test]
fn testGetConstructorResolvesFactoryChildDefinitionsClass() {
    let mut container = container();
    container
        .set_parameter("factory_dummy_class", factory_dummy_class().as_str())
        .unwrap();
    container
        .register("parent", Some("%factory_dummy_class%"))
        .unwrap()
        .set_abstract(true);
    container
        .set_definition("child", Definition::child("parent"))
        .unwrap();
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_factory(Value::Array(PhpArray::list([
            Value::from(Reference::new("child")),
            Value::from("createFactory"),
        ])));

    let mut pass = Probe::new(Read::Constructor);
    CompilerPassInterface::process(&mut pass, &mut container).unwrap();

    let actual = pass.actual.unwrap();
    assert!(
        actual.is_method(),
        "assertInstanceOf(\\ReflectionMethod::class)"
    );
    assert_eq!(Some(factory_dummy_class().as_str()), actual.class());
}

#[test]
fn testGetConstructorResolvesChildDefinitionsClass() {
    let mut container = container();
    container
        .register("parent", Some(&bar_class()))
        .unwrap()
        .set_abstract(true);
    container
        .set_definition("foo", Definition::child("parent"))
        .unwrap();

    let mut pass = Probe::new(Read::Constructor);
    CompilerPassInterface::process(&mut pass, &mut container).unwrap();

    let actual = pass.actual.unwrap();
    assert!(
        actual.is_method(),
        "assertInstanceOf(\\ReflectionMethod::class)"
    );
    assert_eq!(Some(bar_class().as_str()), actual.class());
}

#[test]
fn testGetReflectionMethodResolvesChildDefinitionsClass() {
    let mut container = container();
    container
        .register("parent", Some(&bar_class()))
        .unwrap()
        .set_abstract(true);
    container
        .set_definition("foo", Definition::child("parent"))
        .unwrap();

    let mut pass = Probe::new(Read::Method("create"));
    CompilerPassInterface::process(&mut pass, &mut container).unwrap();

    let actual = pass.actual.unwrap();
    assert!(
        actual.is_method(),
        "assertInstanceOf(\\ReflectionMethod::class)"
    );
    assert_eq!(Some(bar_class().as_str()), actual.class());
}

#[test]
fn testGetConstructorDefinitionNoClass() {
    let mut container = container();
    container.register("foo", None).unwrap();

    let mut pass = Probe::new(Read::Constructor);
    let e = CompilerPassInterface::process(&mut pass, &mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "Invalid service \"foo\": the class is not set.",
        e.message()
    );
}

/// The walker reaches inline definitions and argument values, and the
/// root array's replacements do not reach the container.
#[test]
fn walks_arguments_and_inline_definitions() {
    struct Rename(RecursivePassState, Vec<String>);
    impl CompilerPassInterface for Rename {
        fn class(&self) -> &'static str {
            "Rename"
        }
        fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
            recursive_process(self, container)
        }
    }
    impl AbstractRecursivePass for Rename {
        fn state(&mut self) -> &mut RecursivePassState {
            &mut self.0
        }
        fn process_value(
            &mut self,
            container: &mut ContainerBuilder,
            value: Value,
            is_root: bool,
        ) -> Result<Value> {
            if let Some(reference) = Reference::from_value(&value) {
                self.1.push(format!(
                    "{}:{}",
                    self.0.current_id.clone().unwrap_or_default(),
                    reference.id()
                ));
                return Ok(Value::from(Reference::new(format!(
                    "{}.renamed",
                    reference.id()
                ))));
            }
            let walked = self.parent_process_value(container, value, is_root)?;
            if is_root && Definition::from_value(&walked).is_some() {
                // Dropped from the root array only.
                return Ok(Value::Null);
            }
            Ok(walked)
        }
    }

    let mut container = ContainerBuilder::new();
    let inline = Definition::with_class("Inline");
    inline.add_argument(Reference::new("deep"));
    container
        .register("foo", Some("Foo"))
        .unwrap()
        .add_argument(Reference::new("bar"))
        .add_argument(inline.to_value());
    container
        .register("excluded", Some("Foo"))
        .unwrap()
        .add_tag("container.excluded", PhpArray::new())
        .add_argument(Reference::new("baz"));

    let mut pass = Rename(RecursivePassState::default(), Vec::new());
    CompilerPassInterface::process(&mut pass, &mut container).unwrap();
    assert_eq!(vec!["foo:bar", "foo:deep"], pass.1);
    assert!(container.has_definition("foo"));
    let arguments = container.get_definition("foo").unwrap().arguments();
    let first = arguments.values().next().unwrap();
    assert_eq!("bar.renamed", Reference::from_value(first).unwrap().id());
    let deep = inline.arguments();
    assert_eq!(
        "deep.renamed",
        Reference::from_value(deep.values().next().unwrap())
            .unwrap()
            .id()
    );
    let excluded = container.get_definition("excluded").unwrap().arguments();
    assert_eq!(
        "baz",
        Reference::from_value(excluded.values().next().unwrap())
            .unwrap()
            .id()
    );
}
