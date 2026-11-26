//! Tests/Compiler/MergeExtensionConfigurationPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture extensions are Rust extensions
//! ([`TestExtension`]) standing for `Extension` subclasses: their
//! `processConfiguration()` runs symfony-config's `Processor` and keeps
//! the processed configs; `FooConfiguration` and `TestCccConfiguration`
//! are config trees. PHPUnit's mocks are counters and closures.
//!
//! Ported with a change: testUnserializableExtensionDefinitionReplacedByUserDefinition
//! puts a `stdClass` object where upstream puts a closure (the port does
//! not serialize the definition, so no value is unserializable).
//!
//! Pending: testExpressionLanguageProviderForwarding (the port has no
//! `ExpressionLanguage` providers).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/MergeExtensionConfigurationPassTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::disallowed_names,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    clippy::unnecessary_literal_bound
)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use php_value::object::StdClass;
use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_config::definition::Processor;
use symfony_config::definition::builder::TreeBuilder;
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, MergeExtensionConfigurationPass,
};
use symfony_dependency_injection::container_builder::EnvFormat;
use symfony_dependency_injection::extension::ExtensionInterface;
use symfony_dependency_injection::{ContainerBuilder, Error, ErrorKind, ParameterBag, Result};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";

type LoadFn = fn(&TestExtension, &[PhpArray], &mut ContainerBuilder) -> Result<()>;

/// An `Extension` subclass of the test file.
struct TestExtension {
    class: String,
    alias: &'static str,
    /// `getConfiguration()`: the tree of the configuration class it returns.
    configuration: Option<(&'static str, fn() -> TreeBuilder)>,
    load: LoadFn,
    is_extension: bool,
    processed: Mutex<Vec<PhpArray>>,
    get_configuration_calls: AtomicUsize,
    load_calls: AtomicUsize,
}

impl TestExtension {
    fn new(short: &str, alias: &'static str, load: LoadFn) -> Self {
        Self {
            class: format!("{NS}{short}"),
            alias,
            configuration: None,
            load,
            is_extension: true,
            processed: Mutex::new(Vec::new()),
            get_configuration_calls: AtomicUsize::new(0),
            load_calls: AtomicUsize::new(0),
        }
    }

    fn with_configuration(mut self, class: &'static str, tree: fn() -> TreeBuilder) -> Self {
        self.configuration = Some((class, tree));
        self
    }

    /// `processConfiguration($this->getConfiguration(...), $configs)`.
    fn process_configuration(
        &self,
        configs: &[PhpArray],
        container: &mut ContainerBuilder,
    ) -> Result<PhpArray> {
        self.get_configuration_class(configs, container);
        let (_, tree) = self.configuration.unwrap();
        let tree = tree().build_tree().map_err(config_error)?;
        let configs = PhpArray::list(configs.iter().cloned().map(Value::Array));
        let processed = Processor::new()
            .process(&tree, &configs)
            .map_err(config_error)?;
        self.processed.lock().unwrap().push(processed.clone());
        Ok(processed)
    }
}

fn config_error(e: symfony_config::definition::Error) -> Error {
    Error::new(
        ErrorKind::Other(format!("{:?}", e.class())),
        String::from_utf8_lossy(e.get_message()).into_owned(),
    )
}

impl ExtensionInterface for TestExtension {
    fn class(&self) -> &str {
        &self.class
    }

    fn load(&self, configs: &[PhpArray], container: &mut ContainerBuilder) -> Result<()> {
        self.load_calls.fetch_add(1, Ordering::SeqCst);
        (self.load)(self, configs, container)
    }

    fn get_alias(&self) -> String {
        self.alias.to_string()
    }

    fn is_extension(&self) -> bool {
        self.is_extension
    }

    fn get_processed_configs(&self) -> Option<Vec<PhpArray>> {
        self.is_extension
            .then(|| std::mem::take(&mut *self.processed.lock().unwrap()))
    }

    fn get_configuration_class(
        &self,
        _config: &[PhpArray],
        _container: &mut ContainerBuilder,
    ) -> Option<String> {
        self.get_configuration_calls.fetch_add(1, Ordering::SeqCst);
        self.configuration.map(|(class, _)| format!("{NS}{class}"))
    }
}

/// `FooConfiguration`.
fn foo_configuration() -> TreeBuilder {
    let tree_builder = TreeBuilder::new("foo").unwrap();
    tree_builder
        .get_root_node()
        .children()
        .scalar_node("bar")
        .unwrap()
        .end()
        .scalar_node("baz")
        .unwrap()
        .end()
        .boolean_node("bool_node")
        .unwrap()
        .end()
        .end();
    tree_builder
}

/// `TestCccConfiguration`.
fn test_ccc_configuration() -> TreeBuilder {
    let tree_builder = TreeBuilder::new("test_ccc").unwrap();
    tree_builder
        .get_root_node()
        .children()
        .boolean_node("bool_node")
        .unwrap()
        .end()
        .end();
    tree_builder
}

/// `FooExtension::load()`.
fn foo_load(
    extension: &TestExtension,
    configs: &[PhpArray],
    container: &mut ContainerBuilder,
) -> Result<()> {
    let config = extension.process_configuration(configs, container)?;

    if let Some(baz) = config.get(&Key::from("baz"))
        && !baz.is_null()
    {
        container.get_parameter_bag().get("env(BOZ)")?;
        container.resolve_env_placeholders(baz.clone(), &EnvFormat::Placeholder, None)?;
    }

    if let Some(bar) = config.get(&Key::from("bar"))
        && !bar.is_null()
    {
        container
            .register("foo.bar", Some("stdClass"))?
            .set_arguments(PhpArray::list([bar.clone()]));
    }

    container.set_parameter("foo.param", "ccc")
}

fn foo_extension() -> TestExtension {
    TestExtension::new("FooExtension", "foo", foo_load)
        .with_configuration("FooConfiguration", foo_configuration)
}

/// `TestCccExtension`.
fn test_ccc_extension() -> TestExtension {
    TestExtension::new(
        "TestCccExtension",
        "test_ccc",
        |extension, configs, container| {
            extension.process_configuration(configs, container)?;
            Ok(())
        },
    )
    .with_configuration("TestCccConfiguration", test_ccc_configuration)
}

/// `RawConfigExtension`.
fn raw_config_extension() -> TestExtension {
    TestExtension::new(
        "RawConfigExtension",
        "raw_config",
        |_, configs, container| {
            let value = configs[0]
                .get(&Key::from("bool_node"))
                .cloned()
                .unwrap_or_default();
            container.set_parameter("raw_config.bool_node", value)
        },
    )
}

fn register(container: &mut ContainerBuilder, extension: TestExtension) -> Arc<TestExtension> {
    let extension = Arc::new(extension);
    container.register_extension(extension.clone());
    extension
}

fn env_placeholder_names(container: &ContainerBuilder) -> Vec<String> {
    container
        .get_parameter_bag()
        .get_env_placeholders()
        .keys()
        .map(String::from)
        .collect()
}

#[test]
fn testExtensionLoadGetAMergeExtensionConfigurationContainerBuilderInstance() {
    let mut extension = foo_extension();
    extension.load = |_, _, container| {
        // isInstanceOf(MergeExtensionConfigurationContainerBuilder::class)
        assert_eq!(
            Some(format!("{NS}FooExtension").as_str()),
            container.merge_extension_class()
        );
        Ok(())
    };

    let mut container = ContainerBuilder::with_parameter_bag(ParameterBag::default());
    let extension = register(&mut container, extension);
    container.prepend_extension_config("foo", PhpArray::new());

    let mut pass = MergeExtensionConfigurationPass::new();
    pass.process(&mut container).unwrap();
    assert_eq!(1, extension.load_calls.load(Ordering::SeqCst));
}

#[test]
fn testExtensionConfigurationIsTrackedByDefault() {
    let file = file!().rsplit('/').next().unwrap();
    let file = format!("{}/tests/{file}", env!("CARGO_MANIFEST_DIR"));
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class(&format!("{NS}FooExtension")).with_file(&file));
    registry.add_class(ClassMetadata::class(&format!("{NS}FooConfiguration")).with_file(&file));

    let mut container = ContainerBuilder::with_parameter_bag(ParameterBag::default());
    container.set_class_registry(Arc::new(registry));
    let extension = register(&mut container, foo_extension());
    container.prepend_extension_config("foo", php_array!["bar" => true]);

    let mut pass = MergeExtensionConfigurationPass::new();
    pass.process(&mut container).unwrap();

    assert_eq!(2, extension.get_configuration_calls.load(Ordering::SeqCst));
    let real = std::fs::canonicalize(&file).unwrap();
    assert!(
        container
            .get_resources()
            .iter()
            .any(|r| r.to_string() == real.to_string_lossy().as_bytes()),
        "assertContainsEquals(new FileResource(__FILE__))"
    );
}

#[test]
fn testOverriddenEnvsAreMerged() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    container.prepend_extension_config("foo", php_array!["bar" => "%env(FOO)%"]);
    container.prepend_extension_config(
        "foo",
        php_array!["bar" => "%env(BAR)%", "baz" => "%env(BAZ)%"],
    );

    let mut pass = MergeExtensionConfigurationPass::new();
    pass.process(&mut container).unwrap();

    assert_eq!(vec!["BAZ", "FOO"], env_placeholder_names(&container));
    let counters: Vec<(String, i64)> = container
        .get_env_counters()
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    assert_eq!(
        vec![("BAZ".to_string(), 1), ("FOO".to_string(), 0)],
        counters
    );
}

#[test]
fn testEnvOfExtensionDefinitionReplacedByUserDefinitionIsCounted() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    container.prepend_extension_config("foo", php_array!["bar" => "%env(FOO)%"]);
    container.register("foo.bar", Some("ArrayObject")).unwrap();

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert_eq!(
        Some("ArrayObject".to_string()),
        container.get_definition("foo.bar").unwrap().class()
    );
    let counters: Vec<(String, i64)> = container
        .get_env_counters()
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    assert_eq!(vec![("FOO".to_string(), 1)], counters);
}

#[test]
fn testEnvOfExtensionDefinitionReplacedByUserAliasIsCounted() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    container.prepend_extension_config("foo", php_array!["bar" => "%env(FOO)%"]);
    container.register("mock", Some("ArrayObject")).unwrap();
    container.set_alias("foo.bar", "mock").unwrap();

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert!(!container.has_definition("foo.bar"));
    let counters: Vec<(String, i64)> = container
        .get_env_counters()
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    assert_eq!(vec![("FOO".to_string(), 1)], counters);
}

#[test]
fn testUnserializableExtensionDefinitionReplacedByUserDefinition() {
    let mut extension = TestExtension::new(
        "ClosureArgumentExtension",
        "closure_argument",
        |_, _, container| {
            container
                .register("closure_argument", Some("stdClass"))?
                .set_arguments(PhpArray::list([StdClass::object(PhpArray::new())]));
            Ok(())
        },
    );
    extension.is_extension = false;

    let mut container = ContainerBuilder::new();
    register(&mut container, extension);
    container.prepend_extension_config("closure_argument", PhpArray::new());
    container
        .register("closure_argument", Some("ArrayObject"))
        .unwrap();

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert_eq!(
        Some("ArrayObject".to_string()),
        container
            .get_definition("closure_argument")
            .unwrap()
            .class()
    );
}

#[test]
fn testEnvOverriddenInPreviousExtensionIsTrackedWhenReused() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    register(&mut container, test_ccc_extension());
    container.prepend_extension_config("foo", php_array!["bool_node" => false]);
    container.prepend_extension_config("foo", php_array!["bool_node" => "%env(bool:FOO)%"]);
    container.prepend_extension_config("test_ccc", php_array!["bool_node" => "%env(bool:FOO)%"]);

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert_eq!(vec!["bool:FOO"], env_placeholder_names(&container));
}

#[test]
fn testEnvOverriddenInPreviousExtensionIsTrackedWhenReusedWithoutProcessConfiguration() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    register(&mut container, raw_config_extension());
    container.prepend_extension_config("foo", php_array!["bool_node" => false]);
    container.prepend_extension_config("foo", php_array!["bool_node" => "%env(bool:FOO)%"]);
    container.prepend_extension_config("raw_config", php_array!["bool_node" => "%env(bool:FOO)%"]);

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert_eq!(vec!["bool:FOO"], env_placeholder_names(&container));
}

#[test]
fn testProcessedEnvsAreIncompatibleWithResolve() {
    let mut container = ContainerBuilder::new();
    register(
        &mut container,
        TestExtension::new("BarExtension", "bar", |_, _, container| {
            container.resolve_env_placeholders(
                Value::from("%env(int:FOO)%"),
                &EnvFormat::Resolve,
                None,
            )?;
            Ok(())
        }),
    );
    container.prepend_extension_config("bar", PhpArray::new());

    let e = MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "Using a cast in \"env(int:FOO)\" is incompatible with resolution at compile time in \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\BarExtension\". The logic in the extension should be moved to a compiler pass, or an env parameter with no cast should be used instead.",
        e.message()
    );
}

#[test]
fn testThrowingExtensionsGetMergedBag() {
    let mut container = ContainerBuilder::new();
    register(
        &mut container,
        TestExtension::new("ThrowingExtension", "throwing", |_, _, _| {
            Err(Error::new(
                ErrorKind::Other("Exception".to_string()),
                "here",
            ))
        })
        .with_configuration("FooConfiguration", foo_configuration),
    );
    container.prepend_extension_config("throwing", php_array!["bar" => "%env(FOO)%"]);

    let mut pass = MergeExtensionConfigurationPass::new();
    match pass.process(&mut container) {
        Ok(()) => panic!("An exception should have been thrown."),
        Err(e) => assert_eq!("here", e.message()),
    }

    assert_eq!(vec!["FOO"], env_placeholder_names(&container));
}

#[test]
fn testMissingParameterIncludesExtension() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    container.prepend_extension_config("foo", php_array!["foo" => "%missing_parameter%"]);

    let mut pass = MergeExtensionConfigurationPass::new();
    match pass.process(&mut container) {
        Ok(()) => panic!("An exception should have been thrown."),
        Err(e) => {
            assert!(e.is(&ErrorKind::ParameterNotFound));
            assert_eq!(
                "You have requested a non-existent parameter \"missing_parameter\" while loading extension \"foo\".",
                e.message()
            );
        }
    }
}

#[test]
fn testReuseEnvPlaceholderGeneratedByPreviousExtension() {
    let mut container = ContainerBuilder::new();
    register(&mut container, foo_extension());
    register(&mut container, test_ccc_extension());
    container.prepend_extension_config("foo", php_array!["bool_node" => "%env(bool:MY_ENV_VAR)%"]);
    container.prepend_extension_config(
        "test_ccc",
        php_array!["bool_node" => "%env(bool:MY_ENV_VAR)%"],
    );

    MergeExtensionConfigurationPass::new()
        .process(&mut container)
        .unwrap();
}
