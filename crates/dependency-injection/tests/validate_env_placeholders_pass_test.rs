//! Tests/Compiler/ValidateEnvPlaceholdersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `EnvExtension` is a Rust extension standing for the
//! `Extension` subclass; `EnvConfiguration` and
//! `ConfigurationWithArrayNodeRequiringOneElement` are config trees.
//! An exception class is checked by its class name.
//!
//! testDiscardedEnvInConfig's `compile()` is `support::compile()`
//! (nothing is autowired), with `support::EnvDefaults` reading env vars.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ValidateEnvPlaceholdersPassTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::too_many_lines)]

mod support;

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, Value, php_array};
use symfony_config::definition::builder::TreeBuilder;
use symfony_config::definition::{ConfigurationInterface, Processor};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, MergeExtensionConfigurationPass, RegisterEnvVarProcessorsPass,
    ValidateEnvPlaceholdersPass,
};
use symfony_dependency_injection::container_builder::EnvFormat;
use symfony_dependency_injection::extension::ExtensionInterface;
use symfony_dependency_injection::{ContainerBuilder, Error, ErrorKind, Result};

const INVALID_CONFIGURATION: &str =
    "Symfony\\Component\\Config\\Definition\\Exception\\InvalidConfigurationException";
const INVALID_TYPE: &str =
    "Symfony\\Component\\Config\\Definition\\Exception\\InvalidTypeException";

/// `EnvConfiguration`.
struct EnvConfiguration;

impl ConfigurationInterface for EnvConfiguration {
    fn get_config_tree_builder(&self) -> std::result::Result<TreeBuilder, symfony_config::Error> {
        let tree_builder = TreeBuilder::new("env_extension")?;
        tree_builder
            .get_root_node()
            .children()
            .scalar_node("scalar_node")?
            .end()
            .scalar_node("scalar_node_not_empty")?
            .cannot_be_empty()?
            .end()
            .scalar_node("scalar_node_not_empty_validated")?
            .cannot_be_empty()?
            .validate()
            .always_then(Ok)
            .end()?
            .end()
            .integer_node("int_node")?
            .end()
            .float_node("float_node")?
            .end()
            .boolean_node("bool_node")?
            .end()
            .array_node("array_node", None)?
            .before_normalization()
            .if_true_with(|value| !matches!(value, Value::Array(_)))
            .then(|value| Ok(Value::Array(php_array!["child_node" => value])))
            .end()?
            .before_normalization()
            .if_array()
            .then(|v| {
                let Value::Array(mut v) = v else {
                    return Ok(v);
                };
                let key = php_value::array::Key::from("bool_force_cast");
                if let Some(cast) = v.get(&key).filter(|c| !c.is_null()) {
                    let cast = Value::Bool(cast.is_truthy());
                    v.insert(key, cast);
                }
                Ok(Value::Array(v))
            })
            .end()?
            .children()
            .scalar_node("child_node")?
            .end()
            .boolean_node("bool_force_cast")?
            .end()
            .integer_node("int_unset_at_zero")?
            .validate()
            .if_true_with(|value| matches!(value, Value::Int(0)))
            .then_unset()
            .end()?
            .end()
            .end()
            .end()
            .array_node("simple_array_node", None)?
            .end()
            .enum_node("enum_node")?
            .values(vec![Value::from("a"), Value::from("b")])?
            .end()
            .variable_node("variable_node")?
            .end()
            .scalar_node("string_node")?
            .validate()
            .if_true_with(|value| {
                !matches!(value, Value::String(_))
                    || matches!(value, Value::String(s) if s.as_bytes() == b"fail")
            })
            .then_invalid("%s is not a valid string")
            .end()?
            .end()
            .end();

        Ok(tree_builder)
    }
}

/// `ConfigurationWithArrayNodeRequiringOneElement`.
struct ConfigurationWithArrayNodeRequiringOneElement;

impl ConfigurationInterface for ConfigurationWithArrayNodeRequiringOneElement {
    fn get_config_tree_builder(&self) -> std::result::Result<TreeBuilder, symfony_config::Error> {
        let tree_builder = TreeBuilder::new("env_extension")?;
        tree_builder
            .get_root_node()
            .children()
            .array_node("nodes", None)?
            .is_required()?
            .requires_at_least_one_element()
            .scalar_prototype()?
            .end()
            .end()
            .end();

        Ok(tree_builder)
    }
}

/// `EnvExtension`.
struct EnvExtension {
    one_element: bool,
    config: Mutex<Option<PhpArray>>,
    processed: Mutex<Vec<PhpArray>>,
}

impl EnvExtension {
    fn new() -> Self {
        Self {
            one_element: false,
            config: Mutex::new(None),
            processed: Mutex::new(Vec::new()),
        }
    }

    fn configuration(&self) -> Box<dyn ConfigurationInterface> {
        if self.one_element {
            Box::new(ConfigurationWithArrayNodeRequiringOneElement)
        } else {
            Box::new(EnvConfiguration)
        }
    }

    fn get_config(&self) -> Value {
        Value::Array(self.config.lock().unwrap().clone().unwrap())
    }
}

fn config_error(e: &symfony_config::Error) -> Error {
    Error::new(
        ErrorKind::Other(e.class().name().to_string()),
        String::from_utf8_lossy(e.get_message()).into_owned(),
    )
}

impl ExtensionInterface for EnvExtension {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\EnvExtension"
    }

    fn load(&self, configs: &[PhpArray], _container: &mut ContainerBuilder) -> Result<()> {
        if configs.iter().all(PhpArray::is_empty) {
            return Ok(());
        }

        let configs = PhpArray::list(configs.iter().cloned().map(Value::Array));
        let processed = Processor::new()
            .process_configuration(self.configuration().as_ref(), &configs)
            .map_err(|e| config_error(&e))?;
        self.processed.lock().unwrap().push(processed.clone());
        *self.config.lock().unwrap() = Some(processed);
        Ok(())
    }

    fn get_alias(&self) -> String {
        "env_extension".to_string()
    }

    fn is_extension(&self) -> bool {
        true
    }

    fn get_processed_configs(&self) -> Option<Vec<PhpArray>> {
        Some(std::mem::take(&mut *self.processed.lock().unwrap()))
    }

    fn get_configuration(
        &self,
        _config: &[PhpArray],
        _container: &ContainerBuilder,
    ) -> Option<Box<dyn ConfigurationInterface>> {
        Some(self.configuration())
    }
}

fn do_process(container: &mut ContainerBuilder) -> Result<()> {
    MergeExtensionConfigurationPass::new().process(container)?;
    RegisterEnvVarProcessorsPass.process(container)?;
    ValidateEnvPlaceholdersPass::new().process(container)
}

fn register(container: &mut ContainerBuilder) -> Arc<EnvExtension> {
    let ext = Arc::new(EnvExtension::new());
    container.register_extension(ext.clone());
    ext
}

fn resolved(container: &mut ContainerBuilder, ext: &EnvExtension) -> Value {
    container
        .resolve_env_placeholders(ext.get_config(), &EnvFormat::Placeholder, None)
        .unwrap()
}

#[track_caller]
fn assert_same(expected: &Value, actual: &Value) {
    assert!(
        expected.identical(actual),
        "expected {expected:?}, got {actual:?}"
    );
}

#[track_caller]
fn assert_error(e: &Error, class: &str, message: &str) {
    // expectException() is `instanceof`: InvalidTypeException extends
    // InvalidConfigurationException.
    let actual = e.class();
    assert!(
        actual == class || class == INVALID_CONFIGURATION && actual == INVALID_TYPE,
        "{actual}"
    );
    assert_eq!(message, e.message());
}

#[test]
fn testEnvsAreValidatedInConfig() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("env(NULLED)", Value::Null).unwrap();
    container.set_parameter("env(FLOATISH)", "3.2").unwrap();
    let ext = register(&mut container);
    let expected = php_array![
        "scalar_node" => "%env(NULLED)%",
        "scalar_node_not_empty" => "%env(FLOATISH)%",
        "int_node" => "%env(int:FOO)%",
        "float_node" => "%env(float:BAR)%",
        "string_node" => "%env(UNDEFINED)%",
    ];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testDefaultEnvIsValidatedInConfig() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("env(STRING)", "fail").unwrap();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["string_node" => "%env(STRING)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_CONFIGURATION,
        "Invalid configuration for path \"env_extension.string_node\": \"fail\" is not a valid string",
    );
}

#[test]
fn testDefaultEnvWithoutPrefixIsValidatedInConfig() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("env(FLOATISH)", 3.2).unwrap();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["float_node" => "%env(FLOATISH)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The default value of an env() parameter must be a string or null, but \"float\" given to \"env(FLOATISH)\".",
        e.message()
    );
}

#[test]
fn testDefaultProcessorWithScalarNode() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("parameter_int", 12134).unwrap();
    container.set_parameter("env(FLOATISH)", 4.2).unwrap();
    let ext = register(&mut container);
    let expected = php_array!["scalar_node" => "%env(default:parameter_int:FLOATISH)%"];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();
    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testDefaultProcessorAndAnotherProcessorWithScalarNode() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("parameter_int", 12134).unwrap();
    container
        .set_parameter("env(JSON)", "{ \"foo\": \"bar\" }")
        .unwrap();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["scalar_node" => "%env(default:parameter_int:json:JSON)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_TYPE,
        "Invalid type for path \"env_extension.scalar_node\". Expected one of \"bool\", \"int\", \"float\", \"string\", but got one of \"int\", \"array\".",
    );
}

#[test]
fn testEnvsAreValidatedInConfigWithInvalidPlaceholder() {
    let mut container = ContainerBuilder::new();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["bool_node" => "%env(const:BAZ)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_TYPE,
        "Invalid type for path \"env_extension.bool_node\". Expected \"bool\", but got one of \"bool\", \"int\", \"float\", \"string\", \"array\".",
    );
}

#[test]
fn testInvalidEnvInConfig() {
    let mut container = ContainerBuilder::new();
    register(&mut container);
    container
        .prepend_extension_config("env_extension", php_array!["int_node" => "%env(json:FOO)%"]);

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_TYPE,
        "Invalid type for path \"env_extension.int_node\". Expected \"int\", but got \"array\".",
    );
}

#[test]
fn testNulledEnvInConfig() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("env(NULLED)", Value::Null).unwrap();
    register(&mut container);
    container.prepend_extension_config("env_extension", php_array!["int_node" => "%env(NULLED)%"]);

    let e = do_process(&mut container).unwrap_err();
    assert_eq!(INVALID_TYPE, e.class());
    // expectExceptionMessageMatches('/^Invalid type for path "env_extension\.int_node"\. Expected "?int"?, but got (NULL|"null")\.$/')
    let message = e.message();
    assert!(
        [
            "Invalid type for path \"env_extension.int_node\". Expected \"int\", but got \"null\".",
            "Invalid type for path \"env_extension.int_node\". Expected int, but got NULL.",
            "Invalid type for path \"env_extension.int_node\". Expected \"int\", but got NULL.",
            "Invalid type for path \"env_extension.int_node\". Expected int, but got \"null\".",
        ]
        .contains(&message.as_str()),
        "{message}"
    );
}

#[test]
fn testValidateEnvOnMerge() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["int_node" => "%env(int:const:FOO)%", "bool_node" => true],
    );
    container.prepend_extension_config(
        "env_extension",
        php_array![
            "int_node" => "%env(int:BAR)%",
            "bool_node" => "%env(bool:int:BAZ)%",
            "scalar_node" => "%env(BAZ)%",
        ],
    );

    do_process(&mut container).unwrap();

    let expected = php_array![
        "int_node" => "%env(int:const:FOO)%",
        "bool_node" => true,
        "scalar_node" => "%env(BAZ)%",
    ];

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testConcatenatedEnvInConfig() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = "foo %env(BAR)% baz";
    container.prepend_extension_config("env_extension", php_array!["scalar_node" => expected]);

    do_process(&mut container).unwrap();

    assert_same(
        &Value::Array(php_array!["scalar_node" => expected]),
        &resolved(&mut container, &ext),
    );
}

#[test]
fn testSurroundedEnvInConfig() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = "foo%env(BAR)%baz";
    container.prepend_extension_config("env_extension", php_array!["scalar_node" => expected]);

    do_process(&mut container).unwrap();

    assert_same(
        &Value::Array(php_array!["scalar_node" => expected]),
        &resolved(&mut container, &ext),
    );
}

#[test]
fn testEnvIsIncompatibleWithArrayNode() {
    let mut container = ContainerBuilder::new();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["simple_array_node" => "%env(json:FOO)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_CONFIGURATION,
        "A dynamic value is not compatible with a \"Symfony\\Component\\Config\\Definition\\ArrayNode\" node type at path \"env_extension.simple_array_node\".",
    );
}

#[test]
fn testNormalizedEnvIsCompatibleWithArrayNode() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = "%env(CHILD)%";
    container.prepend_extension_config("env_extension", php_array!["array_node" => expected]);

    do_process(&mut container).unwrap();

    assert_same(
        &Value::Array(
            php_array!["array_node" => Value::Array(php_array!["child_node" => expected])],
        ),
        &resolved(&mut container, &ext),
    );
}

#[test]
fn testEnvIsNotUnset() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = php_array![
        "array_node" => Value::Array(php_array!["int_unset_at_zero" => "%env(int:CHILD)%"])
    ];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testEmptyEnvWhichCannotBeEmptyForScalarNode() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = php_array!["scalar_node_not_empty" => "%env(SOME)%"];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testEmptyEnvWhichCannotBeEmptyForScalarNodeWithValidation() {
    let mut container = ContainerBuilder::new();
    register(&mut container);
    container.prepend_extension_config(
        "env_extension",
        php_array!["scalar_node_not_empty_validated" => "%env(SOME)%"],
    );

    let e = do_process(&mut container).unwrap_err();
    assert_error(
        &e,
        INVALID_CONFIGURATION,
        "The path \"env_extension.scalar_node_not_empty_validated\" cannot contain an environment variable when empty values are not allowed by definition and are validated.",
    );
}

#[test]
fn testPartialEnvWhichCannotBeEmptyForScalarNode() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = php_array!["scalar_node_not_empty_validated" => "foo %env(SOME)% bar"];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testEnvWithVariableNode() {
    let mut container = ContainerBuilder::new();
    let ext = register(&mut container);
    let expected = php_array!["variable_node" => "%env(SOME)%"];
    container.prepend_extension_config("env_extension", expected.clone());

    do_process(&mut container).unwrap();

    assert_same(&Value::Array(expected), &resolved(&mut container, &ext));
}

#[test]
fn testEmptyConfigFromMoreThanOneSource() {
    let mut container = ContainerBuilder::new();
    let ext = Arc::new(EnvExtension {
        one_element: true,
        ..EnvExtension::new()
    });
    container.register_extension(ext);
    container
        .load_from_extension("env_extension", Some(PhpArray::new()))
        .unwrap();

    do_process(&mut container).unwrap();
}

#[test]
fn testDiscardedEnvInConfig() {
    let mut container = ContainerBuilder::new();
    container.set_env_var_resolver(Arc::new(support::EnvDefaults));
    container.set_parameter("env(BOOLISH)", "1").unwrap();
    container.set_parameter("boolish", "%env(BOOLISH)%").unwrap();
    container.register_extension(Arc::new(EnvExtension::new()));
    container.prepend_extension_config(
        "env_extension",
        php_array!["array_node" => php_array!["bool_force_cast" => "%boolish%"]],
    );

    support::compile(&mut container, true).unwrap();

    assert_same(&Value::from("1"), &container.get_parameter("boolish").unwrap());
}
