//! Tests/Loader/YamlFileLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; a data provider's rows
//! run in a loop. Fixture classes are class metadata
//! (`loader_support::registry()` plus the few this file adds);
//! `ProjectExtension` is a Rust extension. `FooUnitEnum` is an enum of
//! the loader's YAML environment. `loadFile()` and `parseDefinition()`,
//! which the upstream tests call through reflection, are public here.
//! Regular expressions are checked by hand. Tests skip, with a note,
//! when `upstream/` is not fetched.
//!
//! Not ported, by reason:
//!
//! - the fixture holds an `@=` expression (no `ExpressionLanguage`
//!   component): testLoadServices (services6.yml), testParsesIteratorArgument
//!   (services9.yml)
//! - needs `PhpFileLoader` (not ported): testLoadImports (services4.yml
//!   imports a PHP file)
//! - needs PHP object creation (`$container->get()` of a definition):
//!   testCaseSensitivity, testOverriddenDefaultsBindings, testStack,
//!   testStackDecorates, testStackDecoratesTag
//! - needs the Opis JSON schema validator: testSchemaAcceptsTheFilesTheLoaderAccepts,
//!   testSchemaRejectsTagsGivenAsAMap
//!
//! testPrototype keeps its id and resource assertions but not the two
//! `reflection.*` ones: the port has no `ReflectionClassResource`.
//!
//! Upstream: symfony/dependency-injection Tests/Loader/YamlFileLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::disallowed_names)]

mod loader_support;

use std::path::Path;
use std::sync::Arc;

use loader_support::{assert_error, di, f, message};
use php_value::PhpObject;
use php_value::object::EnumCase;
use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::Error as ConfigError;
use symfony_config::FileLocator;
use symfony_config::loader::LoaderInterface;
use symfony_config::resource::{FileResource, GlobResource, ResourceInterface};
use symfony_dependency_injection::argument::{
    BoundArgument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, DecoratorServicePass, TagDecoratorPass,
};
use symfony_dependency_injection::extension::ExtensionInterface;
use symfony_dependency_injection::loader::{SharedContainer, YamlFileLoader, shared};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, InvalidBehavior, Reference, Result,
};
use symfony_yaml::Environment;

const FOO_UNIT_ENUM: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FooUnitEnum";

/// The fixture classes of `Tests/Fixtures` the YAML files name.
fn classes() -> Arc<ClassRegistry> {
    let mut r = Arc::unwrap_or_clone(loader_support::registry());
    let param = ParameterMetadata::new;
    let optional = |name: &str, type_: Option<&str>| param(name, type_).with_default(Value::Null);
    r.add_class(ClassMetadata::interface(&f("BarInterface")));
    r.add_class(
        ClassMetadata::class(&f("Bar"))
            .with_interfaces(&[&f("BarInterface")])
            .with_method(MethodMetadata::public("__construct").with_parameters(vec![
                optional("quz", None),
                optional("nonExistent", Some("?NonExistent")),
                optional("decorated", Some(&format!("?{}", f("BarInterface")))),
                param("foo", Some("array")).with_default(Value::Array(PhpArray::new())),
                param("baz", Some("iterable")).with_default(Value::Array(PhpArray::new())),
            ]))
            .with_method(
                MethodMetadata::public("create")
                    .with_static(true)
                    .with_parameters(vec![
                        optional("nonExistent", Some("?NonExistent")),
                        optional("factory", None),
                    ]),
            ),
    );
    r.add_class(ClassMetadata::class(&f("CaseSensitiveClass")).with_method(
        MethodMetadata::public("__construct").with_parameters(vec![optional("identifier", None)]),
    ));
    r.add_class(
        ClassMetadata::class(&f("NamedArgumentsDummy"))
            .with_method(MethodMetadata::public("__construct").with_parameters(vec![
                param("c", Some(&f("CaseSensitiveClass"))),
                param("apiKey", None),
                param("hostName", None),
                param("container", Some("Psr\\Container\\ContainerInterface")),
                param("objects", Some("iterable")),
            ]))
            .with_method(
                MethodMetadata::public("setApiKey").with_parameters(vec![param("apiKey", None)]),
            )
            .with_method(
                MethodMetadata::public("setSensitiveClass")
                    .with_parameters(vec![param("c", Some(&f("CaseSensitiveClass")))]),
            )
            .with_method(
                MethodMetadata::public("setAnotherC").with_parameters(vec![param("c", None)]),
            ),
    );
    r.add_class(
        ClassMetadata::class(&f("FooClassWithEnumAttribute")).with_method(
            MethodMetadata::public("__construct")
                .with_parameters(vec![param("bar", Some(FOO_UNIT_ENUM))]),
        ),
    );
    r.add_class(
        ClassMetadata::class(&f("ConstructNotExists")).with_method(
            MethodMetadata::public("__construct")
                .with_parameters(vec![param("notExist", Some(&f("NotExist")))]),
        ),
    );
    r.add_class(
        ClassMetadata::class(&f("BarFactory"))
            .with_method(
                MethodMetadata::public("__construct")
                    .with_parameters(vec![param("bars", Some("iterable"))]),
            )
            .with_method(
                MethodMetadata::public("getDefaultBar").with_return_type(&f("BarInterface")),
            ),
    );
    // Fixtures/includes/foo.php
    r.add_class(ClassMetadata::class("Acme\\Foo"));
    r.add_class(ClassMetadata::class("Acme\\WithShortCutArgs"));
    r.add_class(ClassMetadata::class("stdClass"));
    Arc::new(r)
}

fn container() -> SharedContainer {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(classes());
    shared(container)
}

/// `new YamlFileLoader($container, new FileLocator($path), $env)`.
fn loader(container: &SharedContainer, path: &str, env: Option<&str>) -> Arc<YamlFileLoader> {
    let loader = YamlFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::new([path.to_string()])),
        env,
        false,
    );
    let mut environment = Environment::new();
    environment.constants.define_enum(
        FOO_UNIT_ENUM,
        vec![
            EnumCase::new(FOO_UNIT_ENUM, "BAR", None),
            EnumCase::new(FOO_UNIT_ENUM, "FOO", None),
        ],
    );
    loader.set_yaml_environment(Arc::new(environment));
    loader
}

/// A container and a loader of `Fixtures/yaml`.
fn set_up() -> Option<(SharedContainer, Arc<YamlFileLoader>)> {
    let fixtures = loader_support::fixtures()?;
    let container = container();
    let loader = loader(&container, &format!("{fixtures}/yaml"), None);
    Some((container, loader))
}

fn load(loader: &YamlFileLoader, file: &str) -> std::result::Result<Value, ConfigError> {
    loader.load(&Value::from(file), None)
}

fn compile(container: &SharedContainer) -> Result<()> {
    container.lock().unwrap().compile(false)
}

fn definition(container: &SharedContainer, id: &str) -> Definition {
    container.lock().unwrap().get_definition(id).unwrap()
}

fn parameters(container: &SharedContainer) -> PhpArray {
    container.lock().unwrap().get_parameter_bag().all().clone()
}

#[track_caller]
fn assert_equals(expected: impl Into<Value>, actual: impl Into<Value>, msg: &str) {
    let (expected, actual) = (expected.into(), actual.into());
    assert!(
        phpunit_equals(&expected, &actual),
        "{msg}\nexpected: {expected:?}\n  actual: {actual:?}"
    );
}

fn tags(entries: &[(&str, PhpArray)]) -> PhpArray {
    let mut tags = PhpArray::new();
    for (name, attributes) in entries {
        tags.insert(*name, Value::Array(php_array![attributes.clone()]));
    }
    tags
}

fn reference(id: &str) -> Value {
    Value::from(Reference::new(id))
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_string()).collect()
}

/// `/^\.\d+_<class>~[._A-Za-z0-9]{7}$/`.
fn is_anonymous_id(id: &str, class: &str) -> bool {
    let Some(rest) = id.strip_prefix('.') else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let Some(hash) = rest[digits..].strip_prefix(&format!("_{class}~")) else {
        return false;
    };
    digits > 0
        && hash.len() == 7
        && hash
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
}

/// `Tests/Fixtures/includes/ProjectExtension.php`.
struct ProjectExtension;

impl ExtensionInterface for ProjectExtension {
    fn class(&self) -> &'static str {
        "ProjectExtension"
    }

    fn load(&self, configs: &[PhpArray], configuration: &mut ContainerBuilder) -> Result<()> {
        let list = PhpArray::list(configs.iter().cloned().map(Value::Array));
        configuration.set_parameter("project.configs", Value::Array(list))?;
        let mut config = PhpArray::new();
        for c in configs.iter().filter(|c| !c.is_empty()) {
            for (k, v) in c.iter() {
                config.insert(k.clone(), v.clone());
            }
        }
        let foo = config
            .get(&Key::from("foo"))
            .cloned()
            .unwrap_or(Value::from("foobar"));
        configuration
            .register("project.service.bar", Some("FooClass"))?
            .set_public(true);
        configuration.set_parameter("project.parameter.bar", foo.clone())?;
        configuration
            .register("project.service.foo", Some("FooClass"))?
            .set_public(true);
        configuration.set_parameter("project.parameter.foo", foo)?;
        Ok(())
    }

    fn get_alias(&self) -> String {
        "project".to_string()
    }
}

#[test]
fn testLoadUnExistFile() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let loader = loader(&container(), &format!("{fixtures}/ini"), None);
    let e = loader.load_file("foo.yml").unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "The file \"");
    assert!(message(&e).ends_with("\" does not exist."));
}

#[test]
fn testLoadInvalidYamlFile() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let path = format!("{fixtures}/ini");
    let loader = loader(&container(), &path, None);
    let e = loader
        .load_file(&format!("{path}/parameters.ini"))
        .unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "The file \"");
    assert!(message(&e).contains("\" does not contain valid YAML"));
}

#[test]
fn testLoadInvalidFile() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    for file in [
        "bad_parameters",
        "bad_imports",
        "bad_import",
        "bad_services",
        "bad_service",
        "bad_calls",
        "bad_format",
        "nonvalid1",
        "nonvalid2",
    ] {
        let e = load(&loader, &format!("{file}.yml")).unwrap_err();
        assert_error(&e, &di("InvalidArgumentException"), "");
    }
}

#[test]
fn testLoadParameters() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services2.yml").unwrap();
    assert_equals(
        php_array![
            "foo" => "bar",
            "mixedcase" => php_array!["MixedCaseKey" => "value"],
            "values" => php_array![true, false, 0, 1000.3, i64::MAX],
            "bar" => "foo",
            "escape" => "@escapeme",
            "foo_bar" => reference("foo_bar"),
        ],
        parameters(&container),
        "->load() converts YAML keys to lowercase",
    );
}

#[test]
fn testLoadWithEnvironment() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = container();
    for (env, value) in [
        ("dev", "value when on dev"),
        ("test", "value when on test"),
        ("prod", "value when on prod"),
        ("other", "default value"),
    ] {
        let loader = loader(&container, &format!("{fixtures}/yaml"), Some(env));
        load(&loader, "services29.yml").unwrap();
        assert_eq!(
            php_array![
                "imported_parameter" => value,
                ".container.known_envs" => php_array!["dev", "test", "prod"],
                "root_parameter" => value,
            ],
            parameters(&container)
        );
    }
}

#[test]
fn testResourceTags() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_resource_tags.yml").unwrap();

    let def = definition(&container, "foo");
    assert!(def.has_tag("container.excluded"));
    assert!(def.has_tag("my.tag"));
    assert!(def.has_tag("another.tag"));
    assert_eq!(php_array![php_array!["foo" => "bar"]], def.tag("my.tag"));
    assert_eq!(php_array![php_array![]], def.tag("another.tag"));
    assert!(!def.is_abstract());
}

#[test]
fn testLoadShortSyntax() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_short_syntax.yml").unwrap();

    assert_eq!(
        php_array![1, 2],
        definition(&container, "foo_bar").arguments()
    );
    assert_eq!(
        php_array!["$a" => "a", "App\\Foo" => "foo"],
        definition(&container, "bar_foo").arguments()
    );
}

#[test]
fn testLoadDeprecatedDefinitionWithoutMessageKey() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "deprecated_definition_without_message.yml").unwrap();

    let id = "service_without_deprecation_message";
    assert!(definition(&container, id).is_deprecated());
    let deprecation = definition(&container, id).deprecation(id).unwrap();
    let message = "The \"service_without_deprecation_message\" service is deprecated. You should stop using it, as it will be removed in the future.";
    assert_eq!(message, deprecation.message);
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);

    let id = "alias_without_deprecation_message";
    let alias = container.lock().unwrap().get_alias(id).unwrap();
    assert!(alias.is_deprecated());
    let deprecation = alias.deprecation(id).unwrap();
    let message = "The \"alias_without_deprecation_message\" service alias is deprecated. You should stop using it, as it will be removed in the future.";
    assert_eq!(message, deprecation.message);
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
}

#[test]
fn testDeprecatedAliases() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "deprecated_alias_definitions.yml").unwrap();

    let alias = container
        .lock()
        .unwrap()
        .get_alias("alias_for_foobar")
        .unwrap();
    assert!(alias.is_deprecated());
    let message = "The \"alias_for_foobar\" service alias is deprecated.";
    let deprecation = alias.deprecation("alias_for_foobar").unwrap();
    assert_eq!(message, deprecation.message);
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
}

#[test]
fn testDeprecatedAliasesWithoutPackageAndVersion() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(
        &loader,
        "deprecated_alias_definitions_without_package_and_version.yml",
    )
    .unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Missing attribute \"package\" of the \"deprecated\" option in \"",
    );
    assert!(message(&e).starts_with("Missing attribute"));
    assert!(message(&e).ends_with("\"."));
}

#[test]
fn testFactorySyntaxError() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_factory_syntax.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The value of the \"factory\" option for the \"invalid_factory\" service must be the id of the service without the \"@\" prefix (replace \"@factory:method\" with \"factory:method\"",
    );
}

#[test]
fn testStaticConstructorWithFactory() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "constructor_with_factory.yml").unwrap_err();
    assert_error(
        &e,
        &di("LogicException"),
        "The \"invalid_service\" service cannot declare a factory as well as a constructor.",
    );
}

#[test]
fn testExtensions() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    container
        .lock()
        .unwrap()
        .register_extension(Arc::new(ProjectExtension));
    load(&loader, "services10.yml").unwrap();
    compile(&container).unwrap();
    let services = container.lock().unwrap().get_definitions().clone();
    let parameters = parameters(&container);

    assert!(
        services.contains_key("project.service.bar"),
        "->load() parses extension elements"
    );
    assert!(
        parameters.contains_key(&Key::from("project.parameter.bar")),
        "->load() parses extension elements"
    );

    assert_eq!(
        Some("BAR".to_string()),
        services.get("project.service.foo").unwrap().class(),
        "->load() parses extension elements"
    );
    assert_equals(
        "BAR",
        parameters
            .get(&Key::from("project.parameter.foo"))
            .unwrap()
            .clone(),
        "->load() parses extension elements",
    );

    let e = load(&loader, "services11.yml").unwrap_err();
    assert_error(&e, "InvalidArgumentException", "");
    assert!(
        message(&e).starts_with(
            "There is no extension able to load the configuration for \"foobarfoobar\" (in"
        ),
        "->load() throws an InvalidArgumentException if the tag is not valid"
    );
}

#[test]
fn testPrependExtensionConfig() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = container();
    container
        .lock()
        .unwrap()
        .prepend_extension_config("project", php_array!["foo" => "bar"]);
    let loader = YamlFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::new([format!("{fixtures}/yaml")])),
        None,
        true,
    );
    load(&loader, "services10.yml").unwrap();

    let expected = vec![
        php_array!["test" => "%project.parameter.foo%"],
        php_array!["foo" => "bar"],
    ];
    assert_eq!(
        expected,
        container.lock().unwrap().get_extension_config("project")
    );
}

#[test]
fn testExtensionWithNullConfig() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    container
        .lock()
        .unwrap()
        .register_extension(Arc::new(ProjectExtension));
    load(&loader, "null_config.yml").unwrap();
    compile(&container).unwrap();

    assert_eq!(
        Value::Array(php_array![php_array![]]),
        container
            .lock()
            .unwrap()
            .get_parameter("project.configs")
            .unwrap()
    );
}

#[test]
fn testSupports() {
    let loader = YamlFileLoader::new(container(), Arc::new(FileLocator::default()), None, false);

    let supports =
        |resource: &str, r#type: Option<&[u8]>| loader.supports(&Value::from(resource), r#type);
    assert!(
        supports("foo.yml", None),
        "->supports() returns true if the resource is loadable"
    );
    assert!(
        supports("foo.yaml", None),
        "->supports() returns true if the resource is loadable"
    );
    assert!(
        !supports("foo.foo", None),
        "->supports() returns false if the resource is not loadable"
    );
    assert!(
        supports("with_wrong_ext.xml", Some(b"yml")),
        "->supports() returns true if the resource with forced type is loadable"
    );
    assert!(
        supports("with_wrong_ext.xml", Some(b"yaml")),
        "->supports() returns true if the resource with forced type is loadable"
    );
}

#[test]
fn testNonArrayTagsThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "badtag1.yml").unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "");
    assert!(
        message(&e).starts_with("Parameter \"tags\" must be an array for service"),
        "->load() throws an InvalidArgumentException if the tags key is not an array"
    );
}

#[test]
fn testTagWithoutNameThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "badtag2.yml").unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "");
    assert!(
        message(&e).starts_with("A \"tags\" entry is missing a \"name\" key for service "),
        "->load() throws an InvalidArgumentException if a tag is missing the name key"
    );
}

#[test]
fn testTaggedArguments() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_tagged_argument.yml").unwrap();

    assert_eq!(1, definition(&container, "foo_service").tag("foo").len());
    assert_eq!(
        1,
        definition(&container, "foo_service_tagged_iterator")
            .arguments()
            .len()
    );
    assert_eq!(
        1,
        definition(&container, "foo_service_tagged_locator")
            .arguments()
            .len()
    );

    let arg0 = |id: &str| definition(&container, id).argument(0).unwrap();

    let tagged_iterator = IteratorArgument::tagged("foo", None, false, strings(&["baz"]), true);
    assert_equals(tagged_iterator, arg0("foo_service_tagged_iterator"), "");
    let tagged_iterator2 =
        IteratorArgument::tagged("foo", None, false, strings(&["baz", "qux"]), false);
    assert_equals(tagged_iterator2, arg0("foo2_service_tagged_iterator"), "");

    let tagged_iterator =
        IteratorArgument::tagged("foo", Some("foo"), true, strings(&["baz"]), true);
    assert_equals(
        ServiceLocatorArgument::from_tagged(tagged_iterator),
        arg0("foo_service_tagged_locator"),
        "",
    );
    let tagged_iterator2 =
        IteratorArgument::tagged("foo", Some("foo"), true, strings(&["baz", "qux"]), false);
    assert_equals(
        ServiceLocatorArgument::from_tagged(tagged_iterator2),
        arg0("foo2_service_tagged_locator"),
        "",
    );

    let tagged_iterator = IteratorArgument::tagged("foo", None, true, Vec::new(), true);
    assert_equals(
        ServiceLocatorArgument::from_tagged(tagged_iterator),
        arg0("bar_service_tagged_locator"),
        "",
    );
}

#[test]
fn testDeprecatedTaggedArguments() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    symfony_deprecation_contracts::collect(|| {
        load(&loader, "services_with_deprecated_tagged_argument.yml").unwrap();
    });

    assert_eq!(1, definition(&container, "foo_service").tag("foo").len());
    assert_eq!(
        1,
        definition(&container, "foo_service_tagged_iterator")
            .arguments()
            .len()
    );
    assert_eq!(
        1,
        definition(&container, "foo_service_tagged_locator")
            .arguments()
            .len()
    );

    let (tagged_iterator, tagged_locator) = symfony_deprecation_contracts::collect(|| {
        (
            IteratorArgument::tagged_legacy(
                "foo",
                Some("barfoo"),
                Some("foobar"),
                false,
                Some("getPriority"),
                Vec::new(),
                true,
            ),
            IteratorArgument::tagged_legacy(
                "foo",
                Some("barfoo"),
                Some("foobar"),
                true,
                Some("getPriority"),
                Vec::new(),
                true,
            ),
        )
    })
    .0;
    assert_equals(
        tagged_iterator,
        definition(&container, "foo_service_tagged_iterator")
            .argument(0)
            .unwrap(),
        "",
    );
    assert_equals(
        ServiceLocatorArgument::from_tagged(tagged_locator),
        definition(&container, "foo_service_tagged_locator")
            .argument(0)
            .unwrap(),
        "",
    );
}

#[test]
fn testServiceWithServiceLocatorArgument() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_service_locator_argument.yml").unwrap();

    let values = php_array!["foo" => reference("foo_service"), "bar" => reference("bar_service")];
    assert_equals(
        php_array![ServiceLocatorArgument::new(values)],
        definition(&container, "locator_dependent_service_indexed").arguments(),
        "",
    );

    let values = php_array![reference("foo_service"), reference("bar_service")];
    assert_equals(
        php_array![ServiceLocatorArgument::new(values)],
        definition(&container, "locator_dependent_service_not_indexed").arguments(),
        "",
    );

    let values = php_array!["foo" => reference("foo_service"), 0 => reference("bar_service")];
    assert_equals(
        php_array![ServiceLocatorArgument::new(values)],
        definition(&container, "locator_dependent_service_mixed").arguments(),
        "",
    );

    let inlined_service_arguments =
        definition(&container, "locator_dependent_inline_service").arguments();
    let locator =
        ServiceLocatorArgument::from_value(inlined_service_arguments.get(&Key::Int(0)).unwrap())
            .unwrap();
    for key in ["foo", "bar"] {
        let Some(Value::Array(entry)) = locator.values().get(&Key::from(key)).cloned() else {
            panic!("{key} is not an array");
        };
        let id = Reference::from_value(entry.get(&Key::Int(0)).unwrap())
            .unwrap()
            .to_string();
        assert_equals(
            Definition::with_class("stdClass").to_value(),
            definition(&container, &id).to_value(),
            "",
        );
    }
}

#[test]
fn testParseServiceClosure() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_service_closure.yml").unwrap();

    assert_equals(
        ServiceClosureArgument::new(Value::from(Reference::with_behavior(
            "bar",
            InvalidBehavior::Ignore,
        ))),
        definition(&container, "foo").argument(0).unwrap(),
        "",
    );
}

#[test]
fn testParseShortServiceClosure() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_short_service_closure.yml").unwrap();

    assert_equals(
        ServiceClosureArgument::new(reference("bar")),
        definition(&container, "foo").argument(0).unwrap(),
        "",
    );
}

#[test]
fn testParseEnvClosure() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_env_closure.yml").unwrap();

    let arguments = definition(&container, "foo").arguments();
    let resolve = |value: &str| {
        let resolved = container
            .lock()
            .unwrap()
            .get_parameter_bag()
            .resolve_value(&Value::from(value), &[])
            .unwrap();
        String::from_utf8_lossy(resolved.as_string().unwrap().as_bytes()).into_owned()
    };
    let arg = |i: i64| arguments.get(&Key::Int(i)).unwrap().clone();

    assert_equals(
        EnvClosureArgument::new(resolve("%env(FOO)%"), Value::Null, false).unwrap(),
        arg(0),
        "",
    );
    assert_equals(
        EnvClosureArgument::new(resolve("%env(FOO)%"), Value::Null, true).unwrap(),
        arg(1),
        "",
    );
    assert_equals(
        EnvClosureArgument::new(resolve("%env(BAR)%"), Value::from("def"), true).unwrap(),
        arg(2),
        "",
    );
    assert_equals(
        EnvClosureArgument::new(resolve("%env(FOO)%"), Value::Int(42), false).unwrap(),
        arg(3),
        "",
    );
}

#[test]
fn testNameOnlyTagsAreAllowedAsString() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "tag_name_only.yml").unwrap();

    assert_eq!(1, definition(&container, "foo_service").tag("foo").len());
}

#[test]
fn testTagWithAttributeArray() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "tag_array_arguments.yml").unwrap();

    let definition = definition(&container, "foo_service");
    assert_equals(
        php_array!["foo" => php_array![php_array!["bar" => php_array!["foo" => "foo", "bar" => "bar"]]]],
        definition.tags(),
        "",
    );
}

#[test]
fn testLoadYamlOnlyWithKeys() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services21.yml").unwrap();

    let definition = definition(&container, "manager");
    assert_equals(
        php_array![
            php_array!["setLogger", php_array![reference("logger")]],
            php_array!["setClass", php_array!["User"]],
        ],
        definition.method_calls(),
        "",
    );
    assert_equals(php_array![true], definition.arguments(), "");
    assert_equals(
        php_array!["manager" => php_array![php_array!["alias" => "user"]]],
        definition.tags(),
        "",
    );
}

#[test]
fn testTagWithEmptyNameThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "tag_name_empty_string.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The tag name for service \"",
    );
    assert!(message(&e).contains(" must be a non-empty string"));
}

#[test]
fn testTagWithNonStringNameThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "tag_name_no_string.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The tag name for service \"",
    );
    assert!(message(&e).contains(" must be a non-empty string"));
}

#[test]
fn testAutowire() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services23.yml").unwrap();

    assert!(definition(&container, "bar_service").is_autowired());
}

#[test]
fn testClassFromId() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "class_from_id.yml").unwrap();
    compile(&container).unwrap();

    let class = f("CaseSensitiveClass");
    assert_eq!(Some(class.clone()), definition(&container, &class).class());
}

/// The ids of the definitions without the `container.excluded` tag,
/// sorted.
fn not_excluded_ids(container: &SharedContainer) -> Vec<String> {
    let mut ids: Vec<String> = container
        .lock()
        .unwrap()
        .get_definitions()
        .iter()
        .filter(|(_, d)| !d.has_tag("container.excluded"))
        .map(|(id, _)| id.to_string())
        .collect();
    ids.sort();
    ids
}

#[test]
fn testPrototype() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_prototype.yml").unwrap();

    assert_eq!(
        vec![
            f("Prototype\\Foo"),
            f("Prototype\\NotFoo"),
            f("Prototype\\Sub\\Bar"),
            "service_container".to_string()
        ],
        not_excluded_ids(&container)
    );

    let resources: Vec<String> = container
        .lock()
        .unwrap()
        .get_resources()
        .iter()
        .map(|r| String::from_utf8_lossy(&ResourceInterface::to_string(r.as_ref())).into_owned())
        .collect();

    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../upstream/symfony/dependency-injection/Tests/Fixtures");
    let file = fixtures_dir.join("yaml").join("services_prototype.yml");
    let file_resource = FileResource::new(file.to_string_lossy().as_bytes()).unwrap();
    assert!(resources.contains(
        &String::from_utf8_lossy(&ResourceInterface::to_string(&file_resource)).into_owned()
    ));

    let prototype = fixtures_dir.join("Prototype");
    let prototype_real_path = FileResource::new(prototype.to_string_lossy().as_bytes()).unwrap();
    let prototype_real_path =
        Path::new(std::str::from_utf8(prototype_real_path.get_resource()).unwrap());
    let mut excluded = PhpArray::new();
    for dir in [
        "BadClasses",
        "BadAttributes",
        "OtherDir",
        "SinglyImplementedInterface",
        "StaticConstructor",
    ] {
        let path = prototype_real_path
            .join(dir)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        excluded.insert(path.as_str(), Value::Bool(true));
    }
    let glob_resource = GlobResource::new(
        prototype.to_string_lossy().as_bytes(),
        b"",
        true,
        false,
        excluded,
    )
    .unwrap();
    assert!(
        resources.contains(
            &String::from_utf8_lossy(&ResourceInterface::to_string(&glob_resource)).into_owned()
        ),
        "{resources:?}"
    );
}

#[test]
fn testPrototypeWithNullOrEmptyNode() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    for file in [
        "services_prototype_with_null_node.yml",
        "services_prototype_with_empty_node.yml",
    ] {
        let e = load(&loader, file).unwrap_err();
        assert_error(
            &e,
            &di("InvalidArgumentException"),
            "The exclude list must not contain a \"null\" value.",
        );
    }
}

#[test]
fn testPrototypeWithNamespace() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_prototype_namespace.yml").unwrap();

    let service1 = f("Prototype\\OtherDir\\Component1\\Dir1\\Service1");
    let service2 = f("Prototype\\OtherDir\\Component1\\Dir2\\Service2");
    let service4 = f("Prototype\\OtherDir\\Component2\\Dir1\\Service4");
    let service5 = f("Prototype\\OtherDir\\Component2\\Dir2\\Service5");
    assert_eq!(
        vec![
            service1.clone(),
            service2.clone(),
            service4.clone(),
            service5.clone(),
            "service_container".to_string(),
        ],
        not_excluded_ids(&container)
    );

    assert!(definition(&container, &service1).has_tag("foo"));
    assert!(definition(&container, &service4).has_tag("foo"));
    assert!(!definition(&container, &service1).has_tag("bar"));
    assert!(!definition(&container, &service4).has_tag("bar"));

    assert!(definition(&container, &service2).has_tag("bar"));
    assert!(definition(&container, &service5).has_tag("bar"));
    assert!(!definition(&container, &service2).has_tag("foo"));
    assert!(!definition(&container, &service5).has_tag("foo"));
}

#[test]
fn testPrototypeWithNamespaceAndNoResource() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "services_prototype_namespace_without_resource.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "A \"resource\" attribute must be set when the \"namespace\" attribute is set for service \"",
    );
}

#[test]
fn testDefaults() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services28.yml").unwrap();
    let foo_tag = || tags(&[("foo", php_array![])]);

    let with_defaults = definition(&container, "with_defaults");
    assert!(!with_defaults.is_public());
    assert_eq!(foo_tag(), with_defaults.tags());
    assert!(with_defaults.is_autowired());
    assert!(!with_defaults.changes().contains(&"public".to_string()));
    assert!(!with_defaults.changes().contains(&"autowire".to_string()));

    for alias in ["with_defaults_aliased", "with_defaults_aliased_short"] {
        assert!(
            !container
                .lock()
                .unwrap()
                .get_alias(alias)
                .unwrap()
                .is_public()
        );
    }

    let short_cut = definition(&container, "Acme\\WithShortCutArgs");
    assert!(!short_cut.is_public());
    assert_eq!(foo_tag(), short_cut.tags());
    assert!(short_cut.is_autowired());

    compile(&container).unwrap();

    assert!(definition(&container, "with_null").is_public());
    assert!(definition(&container, "no_defaults").is_public());

    // foo tag is inherited from defaults
    assert_eq!(foo_tag(), definition(&container, "with_null").tags());
    assert_eq!(foo_tag(), definition(&container, "no_defaults").tags());

    assert!(definition(&container, "with_null").is_autowired());
    assert!(!definition(&container, "no_defaults").is_autowired());

    assert!(definition(&container, "child_def").is_public());
    assert_eq!(foo_tag(), definition(&container, "child_def").tags());
    assert!(!definition(&container, "child_def").is_autowired());
}

#[test]
fn testNamedArguments() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_named_args.yml").unwrap();
    let dummy = f("NamedArgumentsDummy");

    assert_equals(
        php_array![0 => Value::Null, "$apiKey" => "ABCD"],
        definition(&container, &dummy).arguments(),
        "",
    );
    assert_equals(
        php_array!["$apiKey" => "ABCD", f("CaseSensitiveClass").as_str() => Value::Null],
        definition(&container, "another_one").arguments(),
        "",
    );

    compile(&container).unwrap();

    assert_equals(
        php_array![Value::Null, "ABCD"],
        definition(&container, &dummy).arguments(),
        "",
    );
    assert_equals(
        php_array![Value::Null, "ABCD"],
        definition(&container, "another_one").arguments(),
        "",
    );
    assert_equals(
        php_array![php_array!["setApiKey", php_array!["123"]]],
        definition(&container, "another_one").method_calls(),
        "",
    );
}

#[test]
fn testInstanceof() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_instanceof.yml").unwrap();
    compile(&container).unwrap();

    let definition = definition(&container, &f("Bar"));
    assert!(definition.is_autowired());
    assert!(definition.is_lazy());
    assert_eq!(
        tags(&[("foo", php_array![]), ("bar", php_array![])]),
        definition.tags()
    );
}

#[test]
fn testInstanceOfAndChildDefinition() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_instanceof_with_parent.yml").unwrap();
    compile(&container).unwrap();

    assert!(definition(&container, "child_service").is_autowired());
}

#[test]
fn testAutoConfigureAndChildDefinition() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_autoconfigure_with_parent.yml").unwrap();
    compile(&container).unwrap();

    assert!(definition(&container, "child_service").is_autoconfigured());
}

#[test]
fn testDefaultsAndChildDefinition() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_defaults_with_parent.yml").unwrap();
    compile(&container).unwrap();

    assert!(definition(&container, "child_service").is_autowired());
}

#[test]
fn testChildDefinitionWithWrongSyntaxThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_parent.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The value of the \"parent\" option for the \"bar\" service must be the id of the service without the \"@\" prefix (replace \"@foo\" with \"foo\").",
    );
}

#[test]
fn testDecoratedServicesWithWrongSyntaxThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_decorates.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The value of the \"decorates\" option for the \"bar\" service must be the id of the service without the \"@\" prefix (replace \"@foo\" with \"foo\").",
    );
}

#[test]
fn testDecoratedServicesWithWrongOnInvalidSyntaxThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_decoration_on_invalid_null.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Did you mean null (without quotes)",
    );
}

#[test]
fn testInvalidTagsWithDefaults() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "services31_invalid_tags.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Parameter \"tags\" must be an array for service \"Foo\\Bar\" in \"",
    );
    assert!(message(&e).ends_with("services31_invalid_tags.yml\"."));
}

#[test]
fn testUnderscoreServiceId() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "services_underscore.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Service names that start with an underscore are reserved. Rename the \"_foo\" service or define it in XML instead.",
    );
}

#[test]
fn testAnonymousServices() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "anonymous_services.yml").unwrap();

    let definition = definition(&container, "Foo");
    assert!(definition.is_autowired());

    // Anonymous service in an argument
    let args = definition.arguments();
    assert_eq!(1, args.len());
    let arg = Reference::from_value(args.get(&Key::Int(0)).unwrap())
        .expect("assertInstanceOf(Reference::class)")
        .to_string();
    assert!(container.lock().unwrap().has(&arg));
    assert!(is_anonymous_id(&arg, "Bar"), "{arg}");

    let anonymous = self::definition(&container, &arg);
    assert_eq!(Some("Bar".to_string()), anonymous.class());
    assert!(!anonymous.is_public());
    assert!(anonymous.is_autowired());

    // Anonymous service in a callable
    let Value::Array(factory) = definition.factory() else {
        panic!("assertIsArray($factory)");
    };
    let factory0 = Reference::from_value(factory.get(&Key::Int(0)).unwrap())
        .expect("assertInstanceOf(Reference::class)")
        .to_string();
    assert!(container.lock().unwrap().has(&factory0));
    assert!(is_anonymous_id(&factory0, "Quz"), "{factory0}");
    assert_equals(
        "constructFoo",
        factory.get(&Key::Int(1)).unwrap().clone(),
        "",
    );

    let anonymous = self::definition(&container, &factory0);
    assert_eq!(Some("Quz".to_string()), anonymous.class());
    assert!(!anonymous.is_public());
    assert!(!anonymous.is_autowired());
}

#[test]
fn testAnonymousServicesIdsDoNotDependOnTheProjectDirectory() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let fixtures = Path::new(&fixtures).join("yaml");
    let mut ids = Vec::new();

    for project in ["project_a", "project_b"] {
        let dir = std::env::temp_dir().join(format!(
            "sf_anonymous_services_{project}_{}",
            std::process::id()
        ));
        let config = dir.join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::copy(
            fixtures.join("anonymous_services.yml"),
            config.join("services.yml"),
        )
        .unwrap();
        std::fs::copy(
            fixtures.join("anonymous_services_in_instanceof.yml"),
            config.join("anonymous_services_in_instanceof.yml"),
        )
        .unwrap();

        let container = container();
        container
            .lock()
            .unwrap()
            .set_parameter("kernel.project_dir", dir.to_string_lossy().as_ref())
            .unwrap();

        let result = load(
            &loader(&container, &config.to_string_lossy(), None),
            "services.yml",
        );
        // finally
        std::fs::remove_dir_all(&dir).unwrap();
        result.unwrap();

        let project_ids: Vec<String> = container
            .lock()
            .unwrap()
            .get_definitions()
            .keys()
            .map(ToString::to_string)
            .collect();
        ids.push(project_ids);
    }

    assert_eq!(ids[0], ids[1]);
}

#[test]
fn testAnonymousServicesInDifferentFilesWithSameNameDoNotConflict() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = container();

    let loader1 = loader(&container, &format!("{fixtures}/yaml/foo"), None);
    load(&loader1, "services.yml").unwrap();

    let loader2 = loader(&container, &format!("{fixtures}/yaml/bar"), None);
    load(&loader2, "services.yml").unwrap();

    assert_eq!(5, container.lock().unwrap().get_definitions().len());
}

#[test]
fn testAnonymousServicesInInstanceof() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "anonymous_services_in_instanceof.yml").unwrap();

    let definition = definition(&container, "Dummy");

    let instanceof = definition.instanceof_conditionals();
    assert_eq!(3, instanceof.len());
    let dummy_interface = instanceof
        .get(&Key::from("DummyInterface"))
        .expect("assertArrayHasKey('DummyInterface')");

    let args = Definition::from_value(dummy_interface)
        .unwrap()
        .properties();
    assert_eq!(1, args.len());
    let foo = Reference::from_value(args.get(&Key::from("foo")).unwrap())
        .expect("assertInstanceOf(Reference::class)")
        .to_string();
    assert!(container.lock().unwrap().has(&foo));

    let anonymous = self::definition(&container, &foo);
    assert_eq!(Some("Anonymous".to_string()), anonymous.class());
    assert!(!anonymous.is_public());
    assert_eq!(PhpArray::new(), anonymous.instanceof_conditionals());

    assert!(!container.lock().unwrap().has("Bar"));
}

#[test]
fn testAnonymousServicesWithAliases() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "anonymous_services_alias.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Creating an alias using the tag \"!service\" is not allowed in \"",
    );
    assert!(message(&e).ends_with("anonymous_services_alias.yml\"."));
}

#[test]
fn testAnonymousServicesInParameters() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "anonymous_services_in_parameters.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Using an anonymous service in a parameter is not allowed in \"",
    );
    assert!(message(&e).ends_with("anonymous_services_in_parameters.yml\"."));
}

#[test]
fn testAutoConfigureInstanceof() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_autoconfigure.yml").unwrap();

    assert!(definition(&container, "use_defaults_settings").is_autoconfigured());
    assert!(!definition(&container, "override_defaults_settings_to_false").is_autoconfigured());
}

#[test]
fn testEmptyDefaultsThrowsClearException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_empty_defaults.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Service \"_defaults\" key must be an array, \"null\" given in \"",
    );
    assert!(message(&e).ends_with("bad_empty_defaults.yml\"."));
}

#[test]
fn testEmptyInstanceofThrowsClearException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_empty_instanceof.yml").unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "Service \"_instanceof\" key must be an array, \"null\" given in \"",
    );
    assert!(message(&e).ends_with("bad_empty_instanceof.yml\"."));
}

#[test]
fn testUnsupportedKeywordThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_keyword.yml").unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "");
    assert!(
        message(&e)
            .starts_with("The configuration key \"private\" is unsupported for definition \"bar\"")
    );
}

#[test]
fn testUnsupportedKeywordInServiceAliasThrowsException() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "bad_alias.yml").unwrap_err();
    assert_error(&e, &di("InvalidArgumentException"), "");
    assert!(message(&e).starts_with(
        "The configuration key \"calls\" is unsupported for the service \"bar\" which is defined as an alias"
    ));
}

#[test]
fn testBindings() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_bindings.yml").unwrap();
    compile(&container).unwrap();

    let first_values = |bindings: PhpArray| {
        let mut values = PhpArray::new();
        for (k, v) in bindings.iter() {
            let bound = BoundArgument::from_value(v).unwrap();
            values.insert(k.clone(), bound.values().get(&Key::Int(0)).unwrap().clone());
        }
        values
    };

    let bar = f("Bar");
    let definition = definition(&container, "bar");
    assert_equals(
        php_array![
            "NonExistent" => Value::Null,
            f("BarInterface").as_str() => reference(&bar),
            "$foo" => php_array![Value::Null],
            "$quz" => "quz",
            "$factory" => "factory",
            "iterable $baz" => IteratorArgument::tagged("bar", None, false, Vec::new(), true),
        ],
        first_values(definition.bindings()),
        "",
    );
    assert_equals(
        php_array![
            "quz",
            Value::Null,
            reference(&bar),
            php_array![Value::Null],
            IteratorArgument::tagged("bar", None, false, Vec::new(), true),
        ],
        definition.arguments(),
        "",
    );

    let definition = self::definition(&container, &bar);
    assert_equals(
        php_array![Value::Null, "factory"],
        definition.arguments(),
        "",
    );
    assert_equals(
        php_array![
            "NonExistent" => Value::Null,
            "$quz" => "quz",
            "$factory" => "factory",
        ],
        first_values(definition.bindings()),
        "",
    );
}

#[test]
fn testProcessNotExistingActionParam() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_not_existing.yml").unwrap();
    let e = compile(&container).unwrap_err();
    assert!(e.instance_of(&di("RuntimeException")), "{}", e.class());
    assert!(
        e.message().contains("Cannot autowire service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\ConstructNotExists\": argument \"$notExist\" of method \"__construct()\" has type \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NotExist\" but this class was not found."),
        "{}",
        e.message()
    );
}

#[test]
fn testFqcnLazyProxy() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_lazy_fqcn.yml").unwrap();

    let definition = definition(&container, "foo");
    assert_eq!(
        php_array![php_array!["interface" => "SomeInterface"]],
        definition.tag("proxy")
    );
}

#[test]
fn testServiceWithSameNameAsInterfaceAndFactoryIsNotTagged() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "service_instanceof_factory.yml").unwrap();
    symfony_deprecation_contracts::collect(|| compile(&container).unwrap());

    let tagged = container
        .lock()
        .unwrap()
        .find_tagged_service_ids("bar", false)
        .unwrap();
    assert_eq!(1, tagged.len());
}

#[test]
fn testDefaultValueOfTagged() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "tagged_iterator_optional.yml").unwrap();

    let iterator_argument = definition(&container, "iterator_service")
        .argument(0)
        .unwrap();
    let iterator_argument = IteratorArgument::from_value(&iterator_argument)
        .filter(IteratorArgument::is_tagged)
        .expect("assertInstanceOf(TaggedIteratorArgument::class)");
    assert_eq!(None, iterator_argument.index_attribute());
}

#[test]
fn testEnumeration() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_with_enumeration.yml").unwrap();
    compile(&container).unwrap();

    let definition = definition(&container, &f("FooClassWithEnumAttribute"));
    let arguments = definition.arguments();
    assert_eq!(1, arguments.len());
    let Some(Value::Object(case)) = arguments.get(&Key::Int(0)) else {
        panic!("not an enum case: {arguments:?}");
    };
    let case = case.downcast_ref::<EnumCase>().unwrap();
    assert_eq!(
        (FOO_UNIT_ENUM, "BAR"),
        (PhpObject::class(case), case.name()),
        "assertSame([FooUnitEnum::BAR])"
    );
}

#[test]
fn testInvalidEnumeration() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = load(&loader, "services_with_invalid_enumeration.yml").unwrap_err();
    // `Yaml::dump(FooUnitEnum::BAR)` starts with '!php/enum' (symfony/yaml 8).
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The string \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FooUnitEnum::BAZ\" is not the name of a valid enum",
    );
}

#[test]
fn testReturnsClone() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "returns_clone.yaml").unwrap();

    let expected = php_array![
        php_array!["bar", php_array![1], true],
        php_array!["bar", php_array![2], true],
    ];
    assert_eq!(expected, definition(&container, "foo").method_calls());
}

#[test]
fn testSinglyImplementedInterfacesInMultipleResources() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(
        &loader,
        "singly_implemented_interface_in_multiple_resources.yml",
    )
    .unwrap();

    let alias = container
        .lock()
        .unwrap()
        .get_alias(&f(
            "Prototype\\SinglyImplementedInterface\\Port\\PortInterface",
        ))
        .unwrap();
    assert_eq!(
        f("Prototype\\SinglyImplementedInterface\\Adapter\\Adapter"),
        alias.to_string()
    );
}

#[test]
fn testNotSinglyImplementedInterfacesInMultipleResources() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(
        &loader,
        "not_singly_implemented_interface_in_multiple_resources.yml",
    )
    .unwrap();

    assert!(!container.lock().unwrap().has_alias(&f(
        "Prototype\\SinglyImplementedInterface\\Port\\PortInterface"
    )));
}

#[test]
fn testNotSinglyImplementedInterfacesInMultipleResourcesWithPreviouslyRegisteredAlias() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(
        &loader,
        "not_singly_implemented_interface_in_multiple_resources_with_previously_registered_alias.yml",
    )
    .unwrap();

    let alias = container
        .lock()
        .unwrap()
        .get_alias(&f(
            "Prototype\\SinglyImplementedInterface\\Port\\PortInterface",
        ))
        .unwrap();
    assert_eq!(
        f("Prototype\\SinglyImplementedInterface\\Adapter\\Adapter"),
        alias.to_string()
    );
}

#[test]
fn testNotSinglyImplementedInterfacesInMultipleResourcesWithPreviouslyRegisteredAlias2() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(
        &loader,
        "not_singly_implemented_interface_in_multiple_resources_with_previously_registered_alias2.yml",
    )
    .unwrap();

    let alias = container
        .lock()
        .unwrap()
        .get_alias(&f(
            "Prototype\\SinglyImplementedInterface\\Port\\PortInterface",
        ))
        .unwrap();
    assert_eq!(
        f("Prototype\\SinglyImplementedInterface\\Adapter\\Adapter"),
        alias.to_string()
    );
}

#[test]
fn testAlternativeMethodCalls() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "alt_call.yaml").unwrap();

    let expected = php_array![
        php_array!["foo", php_array![1, 2, 3]],
        php_array!["bar", php_array![1, 2, 3], true],
        php_array!["method", php_array!["url"]],
    ];
    assert_eq!(expected, definition(&container, "foo").method_calls());
}

#[test]
fn testWhenEnv() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = container();
    let loader = loader(&container, &format!("{fixtures}/yaml"), Some("some-env"));
    load(&loader, "when-env.yaml").unwrap();

    assert_eq!(
        php_array![
            "foo" => 234,
            ".container.known_envs" => php_array!["some-env", "some-other-env"],
            "bar" => 345,
        ],
        parameters(&container)
    );
}

#[test]
fn testClosure() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "closure.yml").unwrap();

    let definition = definition(&container, "closure_property")
        .properties()
        .get(&Key::from("foo"))
        .unwrap()
        .clone();
    let expected = Definition::with_class("Closure");
    expected
        .set_factory(php_array!["Closure", "fromCallable"])
        .add_argument(reference("bar"));
    assert_equals(expected.to_value(), definition, "");
}

#[test]
fn testFromCallable() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "from_callable.yml").unwrap();

    let definition = definition(&container, "from_callable");
    let expected = Definition::with_class("stdClass");
    expected
        .set_factory(php_array!["Closure", "fromCallable"])
        .add_argument(php_array![reference("bar"), "do"])
        .set_lazy(true);
    assert_equals(expected.to_value(), definition.to_value(), "");
}

#[test]
fn testStaticConstructor() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "static_constructor.yml").unwrap();

    let definition = definition(&container, "static_constructor");
    let expected = Definition::with_class("stdClass");
    expected.set_factory(php_array![Value::Null, "create"]);
    assert_equals(expected.to_value(), definition.to_value(), "");
}

#[test]
fn testFromCallableTriggersDeprecationOnForbiddenKeys() {
    let provide_forbidden_keys: [(&str, Value); 7] = [
        ("parent", Value::from("App\\SomeParent")),
        ("synthetic", Value::Bool(true)),
        ("file", Value::from("some_file.php")),
        ("arguments", Value::Array(PhpArray::new())),
        ("properties", Value::Array(php_array!["foo" => "bar"])),
        ("configurator", Value::from("some_configurator")),
        (
            "calls",
            Value::Array(php_array![
                php_array!["method" => "setFoo", "arguments" => php_array!["bar"]]
            ]),
        ),
    ];
    for (key, value) in provide_forbidden_keys {
        let loader =
            YamlFileLoader::new(container(), Arc::new(FileLocator::default()), None, false);
        let mut service = php_array!["from_callable" => "strlen"];
        service.insert(key, value);
        let (_, deprecations) = symfony_deprecation_contracts::collect(|| {
            loader.parse_definition(
                "my_service",
                &Value::Array(service),
                "config/services.yaml",
                &PhpArray::new(),
            )
        });
        let expected = format!(
            "Since symfony/dependency-injection 8.1: Configuring the \"{key}\" key for the service \"my_service\" when using \"from_callable\" is deprecated and will throw an \"InvalidArgumentException\" in 9.0."
        );
        assert!(
            deprecations
                .iter()
                .any(|d| d.message == expected.as_bytes()),
            "{key}: {deprecations:?}"
        );
    }
}

#[test]
fn testDecoratesTag() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    load(&loader, "services_decorates_tag.yml").unwrap();

    let mut c = container.lock().unwrap();
    TagDecoratorPass.process(&mut c).unwrap();
    DecoratorServicePass::new().process(&mut c).unwrap();

    assert!(!c.has("tag_decorator"));

    assert!(c.has_alias("foo"));
    assert!(c.has_alias("bar"));

    assert!(
        c.get_alias("foo")
            .unwrap()
            .to_string()
            .contains(".decorator.foo.")
    );
}
