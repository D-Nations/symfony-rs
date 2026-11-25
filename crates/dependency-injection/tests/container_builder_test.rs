//! Tests/ContainerBuilderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's. Fixture classes are
//! class metadata; `ProjectExtension` is a Rust extension; stubbed passes
//! do nothing. `$_ENV`/`$_SERVER` are a map read by `EnvFixture`, which
//! stands for `EnvVarProcessor`'s plain-name lookup (`$_ENV`, then
//! `$_SERVER` except `HTTP_*`, then the `env(NAME)` default). Tests that
//! read the upstream fixture files skip, with a note, when `upstream/`
//! is not fetched.
//!
//! Pending, by reason:
//!
//! - needs PHP object creation (the Rust emitter replaces
//!   `ContainerBuilder::get()` for definitions): testGetThrowsCircularReferenceExceptionIfServiceHasReferenceToItself,
//!   testGetReturnsSameInstanceWhenServiceIsShared, testGetCreatesServiceBasedOnDefinition,
//!   testNonSharedServicesReturnsDifferentInstances, testGetEvictsSharedServiceWhenMethodCallFails,
//!   testGetEvictsSharedServiceWhenPropertyInjectionFails, testGetEvictsSharedServiceWhenConfiguratorFails,
//!   testGetEvictsSharedServiceWhenConfiguratorIsNotACallable, testGetEvictsWitherServiceWhenSetupFails,
//!   testCircularSetterInjectionRetriesAfterFailure, testAliases, testAliasesKeepInvalidBehavior,
//!   testCreateService, testCreateProxyWithRealServiceInstantiator, testCreateLazyProxy,
//!   testCreateLazyProxyForInlineDefinition, testCreateServiceClass, testCreateServiceArguments,
//!   testCreateServiceFactory, testCreateServiceMethodCalls, testCreateServiceMethodCallsWithEscapedParam,
//!   testCreateServiceProperties, testCreateServiceConfigurator, testCreateServiceWithIteratorArgument,
//!   testCreateServiceWithExpression, testResolveServices, testResolveServicesWithCustomDefinitionClass,
//!   testInlinedDefinitions, testNamedArgumentBeforeCompile;
//! - needs compiler passes that are not ported yet (`compile()` stops at
//!   the first `PendingPass`): testParameterDeprecationIsTrgiggeredWhenCompiled,
//!   testGetEvictsPrivateSharedServiceWhenSetupFails, testClosureProxy, testClosureProxyWithStaticMethod,
//!   testEnvExpressionFunction, testCreateServiceWithAbstractArgument, testMerge,
//!   testCompileWithResolveEnv, testCompileWithArrayResolveEnv, testCompileWithArrayAndAnotherResolveEnv,
//!   testCompileWithArrayInStringResolveEnv, testCompileWithResolveMissingEnv, testDynamicEnv,
//!   testFallbackEnv, testCastEnv, testEnvAreNullable, testEnvInId, testCircularDynamicEnv,
//!   testEnvVarProcessorsRegisteredAfterResolvingEnvPlaceholders, testMergeLogicException,
//!   testCompilesClassDefinitionsOfLazyServices, testRegisteredButNotLoadedExtension,
//!   testRegisteredAndLoadedExtension, testPrivateServiceUser,
//!   testThrowsExceptionWhenSetServiceOnACompiledContainer, testNoExceptionWhenAddServiceOnACompiledContainer,
//!   testNoExceptionWhenSetSyntheticServiceOnACompiledContainer,
//!   testThrowsExceptionWhenSetDefinitionOnACompiledContainer, testAbstractAlias,
//!   testLazyLoadedService, testInitializePropertiesBeforeMethodCalls, testAutowiring, testClassFromId,
//!   testNoClassFromGlobalNamespaceClassId, testNoClassFromGlobalNamespaceClassIdWithLeadingSlash,
//!   testNoClassFromNamespaceClassIdWithLeadingSlash, testNoClassFromNonClassId,
//!   testNoClassFromNsSeparatorId, testGetThrownServiceNotFoundExceptionWithCorrectServiceId,
//!   testUnusedServiceRemovedByPassAndServiceNotFoundExceptionWasNotThrown, testEnvClosureAutowire,
//!   testEnvClosureAutowireRefreshesAcrossEnvCacheReset, testServiceLocator, testUninitializedReference,
//!   testAlmostCircular, testCaseSensitivity, testParameterWithMixedCase,
//!   testArgumentsHaveHigherPriorityThanBindings, testUninitializedSyntheticReference,
//!   testServiceLocatorArgument, testDecoratedSelfReferenceInvolvingPrivateServices, testScalarService,
//!   testWither, testLazyWither, testWitherWithStaticReturnType, testAutoAliasing,
//!   testDirectlyAccessingDeprecatedPublicService, testReferencingDeprecatedPublicService,
//!   testExpressionInFactory, testNamedArgumentAfterCompile, testLazyClosure, testProxyAndInheritance
//!   (most of these also create services);
//! - needs a custom `ParameterBagInterface` (the bag is the crate's
//!   `ParameterBag`): testDeprecateParameterThrowsWhenParameterBagIsNotInternal;
//! - needs symfony-config's `ClassExistenceResource` and
//!   `ReflectionClassResource`: testGetReflectionClass.
//!
//! Upstream: symfony/dependency-injection Tests/ContainerBuilderTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::disallowed_names
)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use php_value::object::StdClass;
use php_value::{PhpArray, Value, array::Key};
use symfony_config::resource::{DirectoryResource, FileResource, ResourceInterface};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassKind, ClassMetadata, ClassRegistry, Reflector,
};
use symfony_dependency_injection::compiler::{CompilerPassInterface, Pass, PassType};
use symfony_dependency_injection::container_builder::{
    AttributeConfigurator, EnvFormat, EnvVarResolver, TrackContents,
};
use symfony_dependency_injection::extension::ExtensionInterface;
use symfony_dependency_injection::{
    Alias, ContainerBuilder, Definition, DefinitionError, Error, ErrorKind, InvalidBehavior,
    ParameterBag, Reference, Result,
};
use symfony_deprecation_contracts::collect;

const CONTAINER_INTERFACE: &str = "Symfony\\Component\\DependencyInjection\\ContainerInterface";

fn s(text: &str) -> Value {
    Value::from(text)
}

fn std_class() -> Value {
    StdClass::object(PhpArray::new())
}

fn upstream_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../upstream/symfony/dependency-injection/Tests");
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: upstream/ is not fetched (scripts/fetch-upstream.sh)");
        None
    }
}

fn keys(map: &symfony_dependency_injection::IdMap<impl Sized>) -> Vec<String> {
    map.keys().map(String::from).collect()
}

/// `$_ENV` and `$_SERVER`, read as `EnvVarProcessor` reads a plain name.
struct EnvFixture {
    env: HashMap<String, String>,
    server: HashMap<String, String>,
}

impl EnvVarResolver for EnvFixture {
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        parameters: &ParameterBag,
        _get_env: &mut dyn FnMut(&str) -> Result<Value>,
    ) -> Result<Value> {
        assert_eq!("", prefix, "the fixture reads plain names only");
        let found = self.env.get(name).cloned().or_else(|| {
            if name.starts_with("HTTP_") {
                None
            } else {
                self.server.get(name).cloned()
            }
        });
        if let Some(value) = found {
            return Ok(Value::from(value));
        }
        let default = format!("env({name})");
        if !parameters.has(&default) {
            return Err(Error::env_not_found(format!(
                "Environment variable not found: \"{name}\"."
            )));
        }
        parameters.get(&default)
    }
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
            .unwrap_or(s("foobar"));
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

/// `$this->createStub(CompilerPassInterface::class)`.
struct StubPass;

impl CompilerPassInterface for StubPass {
    fn class(&self) -> &'static str {
        "CompilerPassInterfaceStub"
    }

    fn process(&mut self, _container: &mut ContainerBuilder) -> Result<()> {
        Ok(())
    }
}

#[test]
fn testDefaultRegisteredDefinitions() {
    let builder = ContainerBuilder::new();

    assert_eq!(1, builder.get_definitions().len());
    assert!(builder.has_definition("service_container"));

    let definition = builder.get_definition("service_container").unwrap();
    assert!(
        !definition.is_child(),
        "assertInstanceOf(Definition::class)"
    );
    assert!(definition.is_synthetic());
    assert_eq!(Some(CONTAINER_INTERFACE.to_string()), definition.class());
}

#[test]
fn testDefinitions() {
    let mut builder = ContainerBuilder::new();
    let definitions = vec![
        ("foo".to_string(), Definition::with_class("Bar\\FooClass")),
        ("bar".to_string(), Definition::with_class("BarClass")),
    ];
    builder.set_definitions(definitions.clone()).unwrap();
    let got = builder.get_definitions();
    assert_eq!(
        vec!["foo", "bar"],
        keys(got),
        "->setDefinitions() sets the service definitions"
    );
    for (id, definition) in &definitions {
        assert!(definition.ptr_eq(got.get(id).unwrap()));
    }
    assert!(
        builder.has_definition("foo"),
        "->hasDefinition() returns true if a service definition exists"
    );
    assert!(
        !builder.has_definition("foobar"),
        "->hasDefinition() returns false if a service definition does not exist"
    );

    let foo = Definition::with_class("FooBarClass");
    builder.set_definition("foobar", foo.clone()).unwrap();
    assert!(
        foo.ptr_eq(&builder.get_definition("foobar").unwrap()),
        "->getDefinition() returns a service definition if defined"
    );
    let foo = Definition::with_class("FooBarClass");
    assert!(
        builder
            .set_definition("foobar", foo.clone())
            .unwrap()
            .ptr_eq(&foo),
        "->setDefinition() implements a fluid interface by returning the service reference"
    );

    let defs = vec![("foobar".to_string(), Definition::with_class("FooBarClass"))];
    builder.add_definitions(defs.clone()).unwrap();
    assert_eq!(
        vec!["foo", "bar", "foobar"],
        keys(builder.get_definitions()),
        "->addDefinitions() adds the service definitions"
    );
    assert!(
        defs[0]
            .1
            .ptr_eq(builder.get_definitions().get("foobar").unwrap())
    );

    let e = builder.get_definition("baz").unwrap_err();
    assert!(
        e.is(&ErrorKind::ServiceNotFound),
        "->getDefinition() throws a ServiceNotFoundException if the service definition does not exist"
    );
    assert_eq!(
        "You have requested a non-existent service \"baz\".",
        e.message(),
        "->getDefinition() throws a ServiceNotFoundException if the service definition does not exist"
    );
}

#[test]
fn testDeprecateParameter() {
    let mut builder = ContainerBuilder::new();
    builder.set_parameter("foo", "bar").unwrap();

    builder
        .deprecate_parameter("foo", "symfony/test", "6.3", None)
        .unwrap();

    let (result, deprecations) = collect(|| builder.get_parameter("foo"));
    result.unwrap();
    let messages: Vec<String> = deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect();
    assert_eq!(
        vec!["Since symfony/test 6.3: The parameter \"foo\" is deprecated."],
        messages
    );
}

#[test]
fn testDeprecateParameterThrowsWhenParameterIsUndefined() {
    let mut builder = ContainerBuilder::new();

    let e = builder
        .deprecate_parameter("foo", "symfony/test", "6.3", None)
        .unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"foo\".",
        e.message()
    );
}

#[test]
fn testRegister() {
    let mut builder = ContainerBuilder::new();
    builder.register("foo", Some("Bar\\FooClass")).unwrap();
    assert!(
        builder.has_definition("foo"),
        "->hasDefinition() returns true if a service definition exists"
    );
    assert!(
        !builder.get_definition("foo").unwrap().is_child(),
        "->getDefinition() returns an instance of Definition"
    );
}

#[test]
fn testRegisterChild() {
    let mut builder = ContainerBuilder::new();
    builder.register("foo", Some("Bar\\FooClass")).unwrap();
    builder.register_child("bar", "foo").unwrap();
    assert!(
        builder.has_definition("bar"),
        "->hasDefinition() returns true if a service definition exists"
    );
    let definition = builder.get_definition("bar").unwrap();
    assert!(
        definition.is_child(),
        "->getDefinition() returns an instance of Definition"
    );
    assert_eq!(
        Some("foo".to_string()),
        definition.parent(),
        "->getParent() returns the id of the parent service"
    );
}

#[test]
fn testAutowire() {
    let mut builder = ContainerBuilder::new();
    builder.autowire("foo", Some("Bar\\FooClass")).unwrap();

    assert!(
        builder.has_definition("foo"),
        "->autowire() registers a new service definition"
    );
    assert!(
        builder.get_definition("foo").unwrap().is_autowired(),
        "->autowire() creates autowired definitions"
    );
}

#[test]
fn testHas() {
    let mut builder = ContainerBuilder::new();
    assert!(
        !builder.has("foo"),
        "->has() returns false if the service does not exist"
    );
    builder.register("foo", Some("Bar\\FooClass")).unwrap();
    assert!(
        builder.has("foo"),
        "->has() returns true if a service definition exists"
    );
    builder.set("bar", Some(std_class())).unwrap();
    assert!(
        builder.has("bar"),
        "->has() returns true if a service exists"
    );
}

#[test]
fn testGetThrowsExceptionIfServiceDoesNotExist() {
    let e = ContainerBuilder::new()
        .get("foo", InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "You have requested a non-existent service \"foo\".",
        e.message()
    );
}

#[test]
fn testGetReturnsNullIfServiceDoesNotExistAndInvalidReferenceIsUsed() {
    let mut builder = ContainerBuilder::new();

    assert!(
        builder.get("foo", InvalidBehavior::Null).unwrap().is_none(),
        "->get() returns null if the service does not exist and NULL_ON_INVALID_REFERENCE is passed as a second argument"
    );
}

#[test]
fn testGetReturnsRegisteredService() {
    let mut builder = ContainerBuilder::new();
    let bar = std_class();
    builder.set("bar", Some(bar.clone())).unwrap();

    assert!(
        bar.identical(
            &builder
                .get("bar", InvalidBehavior::Exception)
                .unwrap()
                .unwrap()
        ),
        "->get() returns the service associated with the id"
    );
}

#[test]
fn testRegisterDoesNotOverrideExistingService() {
    let mut builder = ContainerBuilder::new();
    let bar = std_class();
    builder.set("bar", Some(bar.clone())).unwrap();
    builder.register("bar", Some("stdClass")).unwrap();

    assert!(
        bar.identical(
            &builder
                .get("bar", InvalidBehavior::Exception)
                .unwrap()
                .unwrap()
        ),
        "->get() returns the service associated with the id even if a definition has been defined"
    );
}

fn provide_bad_id() -> [&'static str; 6] {
    ["", "\0", "\r", "\n", "'", "ab\\"]
}

#[test]
fn testBadAliasId() {
    for id in provide_bad_id() {
        let e = ContainerBuilder::new().set_alias(id, "foo").unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument), "{id:?}");
    }
}

#[test]
fn testBadDefinitionId() {
    for id in provide_bad_id() {
        let e = ContainerBuilder::new()
            .set_definition(id, Definition::with_class("Foo"))
            .unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument), "{id:?}");
    }
}

#[test]
fn testGetUnsetLoadingServiceWhenCreateServiceThrowsAnException() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_synthetic(true);

    // we expect a RuntimeException here as foo is synthetic
    let e = builder.get("foo", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));

    // we must also have the same RuntimeException here
    let e = builder.get("foo", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "You have requested a synthetic service (\"foo\"). The DIC does not know how to construct this service.",
        e.message()
    );
}

#[test]
fn testGetServiceIds() {
    let mut builder = ContainerBuilder::new();
    builder.register("foo", Some("stdClass")).unwrap();
    builder.register("bar", Some("stdClass")).unwrap();
    assert_eq!(
        vec!["service_container", "foo", "bar"],
        builder.get_service_ids(),
        "->getServiceIds() returns all defined service ids"
    );
}

#[test]
fn testGetAliases() {
    let mut builder = ContainerBuilder::new();
    builder.set_alias("bar", "foo").unwrap().set_public(true);
    builder.set_alias("foobar", "foo").unwrap();
    builder.set_alias("moo", Alias::new("foo", false)).unwrap();

    let aliases = builder.get_aliases();
    assert_eq!("foo", aliases.get("bar").unwrap().to_string());
    assert!(aliases.get("bar").unwrap().is_public());
    assert_eq!("foo", aliases.get("foobar").unwrap().to_string());
    assert_eq!("foo", aliases.get("moo").unwrap().to_string());
    assert!(!aliases.get("moo").unwrap().is_public());

    builder.register("bar", Some("stdClass")).unwrap();
    assert!(!builder.has_alias("bar"));

    builder.set("foobar", Some(std_class())).unwrap();
    builder.set("moo", Some(std_class())).unwrap();
    assert_eq!(
        0,
        builder.get_aliases().len(),
        "->getAliases() does not return aliased services that have been overridden"
    );
}

#[test]
fn testSetAliases() {
    let mut builder = ContainerBuilder::new();
    builder
        .set_aliases([("bar".to_string(), "foo"), ("foobar".to_string(), "foo")])
        .unwrap();

    let aliases = builder.get_aliases();
    assert!(aliases.contains_key("bar"));
    assert!(aliases.contains_key("foobar"));
}

#[test]
fn testAddAliases() {
    let mut builder = ContainerBuilder::new();
    builder.set_aliases([("bar".to_string(), "foo")]).unwrap();
    builder
        .add_aliases([("foobar".to_string(), "foo")])
        .unwrap();

    let aliases = builder.get_aliases();
    assert!(aliases.contains_key("bar"));
    assert!(aliases.contains_key("foobar"));
}

#[test]
fn testSetReplacesAlias() {
    let mut builder = ContainerBuilder::new();
    builder.set_alias("alias", "aliased").unwrap();
    builder.set("aliased", Some(std_class())).unwrap();

    let foo = std_class();
    builder.set("alias", Some(foo.clone())).unwrap();
    assert!(
        foo.identical(
            &builder
                .get("alias", InvalidBehavior::Exception)
                .unwrap()
                .unwrap()
        ),
        "->set() replaces an existing alias"
    );
}

#[test]
fn testAddGetCompilerPass() {
    let mut builder = ContainerBuilder::new();
    builder.set_resource_tracking(false);
    let default_passes = builder.get_compiler().get_pass_config().get_passes();
    let pass1 = Pass::new(StubPass);
    builder
        .add_compiler_pass(pass1.clone(), PassType::BeforeOptimization, -5)
        .unwrap();
    let pass2 = Pass::new(StubPass);
    builder
        .add_compiler_pass(pass2.clone(), PassType::BeforeOptimization, 10)
        .unwrap();

    let passes = builder.get_compiler().get_pass_config().get_passes();
    assert_eq!(passes.len() - 2, default_passes.len());
    // Pass 1 is executed later
    let at = |pass: &Pass| passes.iter().position(|p| p.ptr_eq(pass)).unwrap();
    assert!(at(&pass1) > at(&pass2));
}

#[test]
fn testCreateSyntheticService() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .set_synthetic(true);

    let e = builder.get("foo", InvalidBehavior::Exception).unwrap_err();
    assert!(e.instance_of("RuntimeException"));
}

#[test]
fn testGetEnvCountersWithEnum() {
    let bag = ParameterBag::env_placeholder(&PhpArray::new()).unwrap();
    let first = bag
        .get("env(enum:Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\StringBackedEnum:foo)")
        .unwrap();
    let second = bag.get("env(Bar)").unwrap();
    let mut config = ContainerBuilder::with_parameter_bag(bag);
    config
        .resolve_env_placeholders(
            Value::Array(PhpArray::list([first, second])),
            &EnvFormat::Placeholder,
            None,
        )
        .unwrap();

    let expected: Vec<(String, i64)> = vec![
        (
            "enum:Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\StringBackedEnum:foo"
                .to_string(),
            1,
        ),
        ("Bar".to_string(), 1),
    ];
    let counters: Vec<(String, i64)> = config
        .get_env_counters()
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    assert_eq!(expected, counters);
}

#[test]
fn testResolveServicesWithDecoratedDefinition() {
    let mut builder = ContainerBuilder::new();
    builder
        .set_definition("grandpa", Definition::with_class("stdClass"))
        .unwrap();
    builder
        .set_definition("parent", Definition::child("grandpa"))
        .unwrap();
    builder
        .set_definition("foo", Definition::child("parent"))
        .unwrap();

    let e = builder.get("foo", InvalidBehavior::Exception).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "Constructing service \"foo\" from a parent definition is not supported at build time.",
        e.message()
    );
}

#[test]
fn testMergeWithExcludedServices() {
    let mut container = ContainerBuilder::new();
    container.set_alias("bar", "foo").unwrap();
    container.register("foo", Some("Bar\\FooClass")).unwrap();
    let mut config = ContainerBuilder::new();
    config
        .register("bar", Some("Bar"))
        .unwrap()
        .add_tag("container.excluded", PhpArray::new());
    config
        .register("foo", Some("Bar"))
        .unwrap()
        .add_tag("container.excluded", PhpArray::new());
    config
        .register("baz", Some("Bar"))
        .unwrap()
        .add_tag("container.excluded", PhpArray::new());
    container.merge(&config).unwrap();
    assert_eq!(
        vec!["service_container", "foo", "baz"],
        keys(container.get_definitions())
    );
    assert!(
        !container
            .get_definition("foo")
            .unwrap()
            .has_tag("container.excluded")
    );
    assert!(
        container
            .get_definition("baz")
            .unwrap()
            .has_tag("container.excluded")
    );
}

#[test]
fn testMergeThrowsExceptionForDuplicateAutomaticInstanceofDefinitions() {
    let mut container = ContainerBuilder::new();
    let mut config = ContainerBuilder::new();
    container.register_for_autoconfiguration("AInterface");
    config.register_for_autoconfiguration("AInterface");

    let e = container.merge(&config).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "\"AInterface\" has already been autoconfigured and merge() does not support merging autoconfiguration for the same class/interface.",
        e.message()
    );
}

#[test]
fn testMergeAttributeAutoconfiguration() {
    const AS_TAGGED_ITEM: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsTaggedItem";
    let mut container = ContainerBuilder::new();
    let c1: AttributeConfigurator =
        Arc::new(|_: &Definition, _: &AttributeMetadata, _: &Reflector| Ok(()));
    container.register_attribute_for_autoconfiguration(AS_TAGGED_ITEM, c1.clone());
    let mut config = ContainerBuilder::new();
    let c2: AttributeConfigurator =
        Arc::new(|_: &Definition, _: &AttributeMetadata, _: &Reflector| Ok(()));
    config.register_attribute_for_autoconfiguration(AS_TAGGED_ITEM, c2.clone());

    container.merge(&config).unwrap();
    let configurators = container.get_attribute_autoconfigurators();
    assert_eq!(vec![AS_TAGGED_ITEM], keys(configurators));
    let list = configurators.get(AS_TAGGED_ITEM).unwrap();
    assert_eq!(2, list.len());
    assert!(Arc::ptr_eq(&c1, &list[0]));
    assert!(Arc::ptr_eq(&c2, &list[1]));
}

#[test]
fn testResolveEnvValues() {
    let mut container = ContainerBuilder::new();
    container.set_env_var_resolver(Arc::new(EnvFixture {
        env: HashMap::from([("DUMMY_ENV_VAR".to_string(), "du%%y".to_string())]),
        server: HashMap::from([
            ("DUMMY_SERVER_VAR".to_string(), "ABC".to_string()),
            ("HTTP_DUMMY_VAR".to_string(), "DEF".to_string()),
        ]),
    }));
    container
        .set_parameter(
            "bar",
            "%% %env(DUMMY_ENV_VAR)% %env(DUMMY_SERVER_VAR)% %env(HTTP_DUMMY_VAR)%",
        )
        .unwrap();
    container
        .set_parameter("env(HTTP_DUMMY_VAR)", "123")
        .unwrap();

    assert!(
        s("%% du%%%%y ABC 123").identical(
            &container
                .resolve_env_placeholders(s("%bar%"), &EnvFormat::Resolve, None)
                .unwrap()
        )
    );
}

#[test]
fn testResolveEnvValuesWithArray() {
    let mut container = ContainerBuilder::new();
    container.set_env_var_resolver(Arc::new(EnvFixture {
        env: HashMap::from([("ANOTHER_DUMMY_ENV_VAR".to_string(), "dummy".to_string())]),
        server: HashMap::new(),
    }));

    let mut dummy_array = PhpArray::new();
    dummy_array.insert("1", s("one"));
    dummy_array.insert("2", s("two"));

    container
        .set_parameter("dummy", "%env(ANOTHER_DUMMY_ENV_VAR)%")
        .unwrap();
    container
        .set_parameter("dummy2", Value::Array(dummy_array.clone()))
        .unwrap();

    container
        .resolve_env_placeholders(s("%dummy%"), &EnvFormat::Resolve, None)
        .unwrap();
    container
        .resolve_env_placeholders(s("%dummy2%"), &EnvFormat::Resolve, None)
        .unwrap();

    let resolved = container
        .resolve_env_placeholders(s("%dummy2%"), &EnvFormat::Resolve, None)
        .unwrap();
    assert!(matches!(resolved, Value::Array(_)));

    for key in dummy_array.keys() {
        let resolved = container
            .resolve_env_placeholders(s("%dummy2%"), &EnvFormat::Resolve, None)
            .unwrap();
        assert!(resolved.as_array().unwrap().contains_key(key));
    }
}

#[test]
fn testFindTaggedServiceIds() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .set_abstract(true)
        .add_tag("foo", php_value::php_array!["foo" => "foo"])
        .add_tag("bar", php_value::php_array!["bar" => "bar"])
        .add_tag("foo", php_value::php_array!["foofoo" => "foofoo"]);
    builder
        .register("bar", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag("foo", PhpArray::new())
        .add_tag("container.excluded", PhpArray::new());

    let found = builder.find_tagged_service_ids("foo", false).unwrap();
    assert_eq!(
        vec!["foo"],
        keys(&found),
        "->findTaggedServiceIds() returns an array of service ids and its tag attributes"
    );
    assert_eq!(
        &PhpArray::list([
            Value::Array(php_value::php_array!["foo" => "foo"]),
            Value::Array(php_value::php_array!["foofoo" => "foofoo"]),
        ]),
        found.get("foo").unwrap()
    );
    assert!(
        builder
            .find_tagged_service_ids("foobar", false)
            .unwrap()
            .is_empty(),
        "->findTaggedServiceIds() returns an empty array if there is annotated services"
    );
}

#[test]
fn testFindTaggedServiceIdsThrowsWhenAbstract() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .set_abstract(true)
        .add_tag("foo", php_value::php_array!["foo" => "foo"]);

    let e = builder.find_tagged_service_ids("foo", true).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The service \"foo\" tagged \"foo\" must not be abstract.",
        e.message()
    );
}

/// `Bar\FooClass` (Fixtures/includes/foo.php) and the prototype fixtures.
fn tagged_resource_classes() -> Arc<ClassRegistry> {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class("Bar\\FooClass"));
    registry.add_class(ClassMetadata::interface(
        "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\FooInterface",
    ));
    registry.add_class(
        ClassMetadata::class(
            "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\AbstractClass",
        )
        .with_abstract(true),
    );
    Arc::new(registry)
}

#[test]
fn testFindTaggedResourceIds() {
    let mut builder = ContainerBuilder::new();
    builder.set_class_registry(tagged_resource_classes());
    builder
        .register("myservice", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag("foo", php_value::php_array!["foo" => "foo"])
        .add_tag("bar", php_value::php_array!["bar" => "bar"])
        .add_tag("foo", php_value::php_array!["foofoo" => "foofoo"])
        .add_resource_tag("container.excluded", PhpArray::new());

    let found = builder.find_tagged_resource_ids("foo", true).unwrap();
    assert_eq!(vec!["myservice"], keys(&found));
    assert_eq!(
        &PhpArray::list([
            Value::Array(php_value::php_array!["foo" => "foo"]),
            Value::Array(php_value::php_array!["foofoo" => "foofoo"]),
        ]),
        found.get("myservice").unwrap()
    );
    assert!(
        builder
            .find_tagged_resource_ids("foofoo", true)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn testFindTaggedResourceIdsThrowsWhenNotExcluded() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("myservice", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag("foo", php_value::php_array!["foo" => "foo"])
        .add_tag("bar", php_value::php_array!["bar" => "bar"])
        .add_tag("foo", php_value::php_array!["foofoo" => "foofoo"]);

    let e = builder.find_tagged_resource_ids("foo", true).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The resource \"myservice\" tagged \"foo\" is missing the \"container.excluded\" tag; did you mean to use \"resource_tags\" instead of \"tags\"?",
        e.message()
    );
}

#[test]
fn testFindTaggedResourceIdsSkipsBaseTypes() {
    const FOO_INTERFACE: &str =
        "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\FooInterface";
    const ABSTRACT_CLASS: &str =
        "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\Prototype\\AbstractClass";
    let mut builder = ContainerBuilder::new();
    builder.set_class_registry(tagged_resource_classes());
    builder
        .register("concrete", Some("Bar\\FooClass"))
        .unwrap()
        .add_resource_tag("foo", php_value::php_array!["foo" => "concrete"]);
    builder
        .register(&format!(".abstract.{FOO_INTERFACE}"), Some(FOO_INTERFACE))
        .unwrap()
        .set_abstract(true)
        .add_resource_tag("foo", php_value::php_array!["foo" => "interface"]);
    builder
        .register(&format!(".abstract.{ABSTRACT_CLASS}"), Some(ABSTRACT_CLASS))
        .unwrap()
        .set_abstract(true)
        .add_resource_tag("foo", php_value::php_array!["foo" => "abstract"]);

    let found = builder.find_tagged_resource_ids("foo", true).unwrap();
    assert_eq!(vec!["concrete"], keys(&found));
    assert_eq!(
        &PhpArray::list([Value::Array(php_value::php_array!["foo" => "concrete"])]),
        found.get("concrete").unwrap()
    );

    let found = builder.find_tagged_resource_ids("foo", false).unwrap();
    let expected = [
        ("concrete".to_string(), "concrete"),
        (format!(".abstract.{FOO_INTERFACE}"), "interface"),
        (format!(".abstract.{ABSTRACT_CLASS}"), "abstract"),
    ];
    assert_eq!(
        expected.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
        keys(&found)
    );
    for (id, value) in expected {
        assert_eq!(
            &PhpArray::list([Value::Array(php_value::php_array!["foo" => value])]),
            found.get(&id).unwrap()
        );
    }
}

#[test]
fn testFindTaggedResourceIdsThrowsOnAbstractDefinitionWithConcreteClass() {
    let mut builder = ContainerBuilder::new();
    builder.set_class_registry(tagged_resource_classes());
    builder
        .register("myservice", Some("Bar\\FooClass"))
        .unwrap()
        .set_abstract(true)
        .add_resource_tag("foo", php_value::php_array!["foo" => "foo"]);

    let e = builder.find_tagged_resource_ids("foo", true).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The resource \"myservice\" tagged \"foo\" must have a class and not be abstract.",
        e.message()
    );
}

#[test]
fn testFindUnusedTags() {
    let mut builder = ContainerBuilder::new();
    builder
        .register("foo", Some("Bar\\FooClass"))
        .unwrap()
        .add_tag(
            "kernel.event_listener",
            php_value::php_array!["foo" => "foo"],
        )
        .add_tag(
            "kenrel.event_listener",
            php_value::php_array!["bar" => "bar"],
        );
    builder
        .find_tagged_service_ids("kernel.event_listener", false)
        .unwrap();
    assert_eq!(
        vec!["kenrel.event_listener"],
        builder.find_unused_tags(),
        "->findUnusedTags() returns an array with unused tags"
    );
}

#[test]
fn testFindDefinition() {
    let mut container = ContainerBuilder::new();
    let definition = Definition::with_class("Bar\\FooClass");
    container.set_definition("foo", definition.clone()).unwrap();
    container.set_alias("bar", "foo").unwrap();
    container.set_alias("foobar", "bar").unwrap();
    assert!(
        definition.ptr_eq(&container.find_definition("foobar").unwrap()),
        "->findDefinition() returns a Definition"
    );
}

#[test]
fn testAddObjectResource() {
    let Some(tests) = upstream_dir() else {
        return;
    };
    let classes = tests.join("Fixtures/includes/classes.php");
    let classes = classes.to_str().unwrap();
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class("BazClass").with_file(classes));
    registry.add_class(
        ClassMetadata::class("BarClass")
            .with_parent("BazClass")
            .with_file(classes),
    );

    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));

    container.set_resource_tracking(false);
    container.add_object_resource("BarClass").unwrap();

    assert!(
        container.get_resources().is_empty(),
        "No resources get registered without resource tracking"
    );

    container.set_resource_tracking(true);
    container.add_object_resource("BarClass").unwrap();

    let resources = container.get_resources();

    assert_eq!(1, resources.len());

    let resource = resources.last().unwrap();

    assert_eq!(
        "Symfony\\Component\\Config\\Resource\\FileResource",
        resource.class()
    );
    assert_eq!(
        std::fs::canonicalize(classes).unwrap(),
        std::fs::canonicalize(String::from_utf8(resource.to_string()).unwrap()).unwrap()
    );
}

#[test]
fn testGetReflectionClassOnInternalTypes() {
    let mut container = ContainerBuilder::new();

    for type_ in [
        "int", "float", "string", "bool", "resource", "object", "array", "null", "callable",
        "iterable", "mixed",
    ] {
        assert!(
            container
                .get_reflection_class(Some(type_), true)
                .unwrap()
                .is_none()
        );
    }
}

fn without_php(resources: &[Arc<dyn ResourceInterface>]) -> Vec<(String, Vec<u8>)> {
    resources
        .iter()
        .filter(|r| !String::from_utf8_lossy(&r.to_string()).contains(".php"))
        .map(|r| (r.class().to_string(), r.to_string()))
        .collect()
}

#[test]
fn testResources() {
    let Some(tests) = upstream_dir() else {
        return;
    };
    let mut container = ContainerBuilder::new();
    let a = FileResource::new(
        tests
            .join("Fixtures/xml/services1.xml")
            .to_str()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let b = FileResource::new(
        tests
            .join("Fixtures/xml/services2.xml")
            .to_str()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let expected = vec![
        (a.class().to_string(), a.to_string()),
        (b.class().to_string(), b.to_string()),
    ];
    container.add_resource(Arc::new(a));
    container.add_resource(Arc::new(b));
    assert_eq!(
        expected,
        without_php(&container.get_resources()),
        "->getResources() returns an array of resources read for the current configuration"
    );
    let returned: *const ContainerBuilder =
        container.set_resources(symfony_dependency_injection::IdMap::new());
    assert!(std::ptr::eq(returned, &raw const container));
    assert!(container.get_resources().is_empty());
}

#[test]
fn testFileExists() {
    let Some(tests) = upstream_dir() else {
        return;
    };
    let mut container = ContainerBuilder::new();
    let a = FileResource::new(
        tests
            .join("Fixtures/xml/services1.xml")
            .to_str()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let b = FileResource::new(
        tests
            .join("Fixtures/xml/services2.xml")
            .to_str()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let dir = php_value::path::dirname(&b.to_string());
    let c = DirectoryResource::new(&dir, None).unwrap();

    let dir = String::from_utf8(dir).unwrap();
    assert!(
        container
            .file_exists(
                &String::from_utf8(a.to_string()).unwrap(),
                &TrackContents::Yes
            )
            .unwrap()
            && container
                .file_exists(
                    &String::from_utf8(b.to_string()).unwrap(),
                    &TrackContents::Yes
                )
                .unwrap()
            && container.file_exists(&dir, &TrackContents::Yes).unwrap()
    );

    let expected = vec![
        (a.class().to_string(), a.to_string()),
        (b.class().to_string(), b.to_string()),
        (c.class().to_string(), c.to_string()),
    ];
    assert_eq!(
        expected,
        without_php(&container.get_resources()),
        "->getResources() returns an array of resources read for the current configuration"
    );
}

#[test]
fn testExtension() {
    let mut container = ContainerBuilder::new();
    container.set_resource_tracking(false);

    let extension: Arc<dyn ExtensionInterface> = Arc::new(ProjectExtension);
    container.register_extension(extension.clone());
    assert!(
        Arc::ptr_eq(&container.get_extension("project").unwrap(), &extension),
        "->registerExtension() registers an extension"
    );

    let e = container.get_extension("no_registered").err().unwrap();
    assert!(e.instance_of("LogicException"));
}

#[test]
fn testExtensionConfig() {
    let mut container = ContainerBuilder::new();

    let configs = container.get_extension_config("foo");
    assert!(configs.is_empty());

    let first = php_value::php_array!["foo" => "bar"];
    container.prepend_extension_config("foo", first.clone());
    let configs = container.get_extension_config("foo");
    assert_eq!(vec![first.clone()], configs);

    let second = php_value::php_array!["ding" => "dong"];
    container.prepend_extension_config("foo", second.clone());
    let configs = container.get_extension_config("foo");
    assert_eq!(vec![second, first], configs);
}

#[test]
fn testThrowsCircularExceptionForCircularAliases() {
    let mut builder = ContainerBuilder::new();

    builder
        .set_aliases([
            ("foo".to_string(), Alias::new("app.test_class", false)),
            (
                "app.test_class".to_string(),
                Alias::new("App\\TestClass", false),
            ),
            (
                "App\\TestClass".to_string(),
                Alias::new("app.test_class", false),
            ),
        ])
        .unwrap();

    let e = builder.find_definition("foo").unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert_eq!(
        "Circular reference detected for service \"app.test_class\", path: \"app.test_class -> App\\TestClass -> app.test_class\".",
        e.message()
    );
}

#[test]
fn testRegisterForAutoconfiguration() {
    let mut container = ContainerBuilder::new();
    let child_def_a = container.register_for_autoconfiguration("AInterface");
    let child_def_b = container.register_for_autoconfiguration("BInterface");
    let instanceof = container.get_autoconfigured_instanceof();
    assert_eq!(vec!["AInterface", "BInterface"], keys(instanceof));
    assert!(child_def_a.ptr_eq(instanceof.get("AInterface").unwrap()));
    assert!(child_def_b.ptr_eq(instanceof.get("BInterface").unwrap()));

    // when called multiple times, the same instance is returned
    assert!(child_def_a.ptr_eq(&container.register_for_autoconfiguration("AInterface")));
}

fn assert_alias(expected_id: &str, alias: &Alias) {
    // assertEquals(new Alias($id), $alias)
    assert_eq!(expected_id, alias.id());
    assert!(!alias.is_public());
    assert!(!alias.is_deprecated());
}

#[test]
fn testRegisterAliasForArgument() {
    let mut container = ContainerBuilder::new();

    container
        .register_alias_for_argument("Foo.bar_baz", "Some\\FooInterface", None, None)
        .unwrap();
    assert_alias(
        "Foo.bar_baz",
        &container
            .get_alias("Some\\FooInterface $fooBarBaz")
            .unwrap(),
    );
    assert_alias(
        "Some\\FooInterface $fooBarBaz",
        &container
            .get_alias(".Some\\FooInterface $Foo.bar_baz")
            .unwrap(),
    );

    container
        .register_alias_for_argument(
            "Foo.bar_baz",
            "Some\\FooInterface",
            Some("Bar_baz.foo"),
            None,
        )
        .unwrap();
    assert_alias(
        "Foo.bar_baz",
        &container
            .get_alias("Some\\FooInterface $barBazFoo")
            .unwrap(),
    );
    assert_alias(
        "Some\\FooInterface $barBazFoo",
        &container
            .get_alias(".Some\\FooInterface $Bar_baz.foo")
            .unwrap(),
    );

    container
        .register_alias_for_argument(
            "Foo.bar_baz",
            "Some\\FooInterface",
            Some("Bar_baz.foo"),
            Some("foo"),
        )
        .unwrap();
    assert_alias(
        "Foo.bar_baz",
        &container
            .get_alias("Some\\FooInterface $barBazFoo")
            .unwrap(),
    );
    assert_alias(
        "Some\\FooInterface $barBazFoo",
        &container.get_alias(".Some\\FooInterface $foo").unwrap(),
    );
}

#[test]
fn testIdCanBeAnObjectAsLongAsItCanBeCastToString() {
    let id = Reference::new("another_service");
    let alias_id = Reference::new("alias_id");

    let mut container = ContainerBuilder::new();
    container.set(&id.to_string(), Some(std_class())).unwrap();
    container
        .set_alias(&alias_id.to_string(), "another_service")
        .unwrap();

    assert!(container.has("another_service"));
    assert!(container.has(&id.to_string()));
    assert!(container.has_alias("alias_id"));
    assert!(container.has_alias(&alias_id.to_string()));

    container.remove_alias(&alias_id.to_string());
    container.remove_definition(&id.to_string());
}

#[test]
fn testErroredDefinition() {
    let mut container = ContainerBuilder::new();

    container
        .register("errored_definition", Some("stdClass"))
        .unwrap()
        .add_error(DefinitionError::Text(
            "Service \"errored_definition\" is broken.".to_string(),
        ))
        .set_public(true);

    let e = container
        .get("errored_definition", InvalidBehavior::Exception)
        .unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!("Service \"errored_definition\" is broken.", e.message());
}

#[test]
fn testFindTags() {
    let mut container = ContainerBuilder::new();
    container
        .register("Symfony\\Component\\DependencyInjection\\Tests\\A", None)
        .unwrap()
        .add_tag("tag1", PhpArray::new())
        .add_tag("tag2", PhpArray::new())
        .add_tag("tag3", PhpArray::new());

    assert_eq!(vec!["tag1", "tag2", "tag3"], container.find_tags());
}

// ----- port checks beyond upstream's tests -----

/// The parts of `testMerge` that need no compiled container.
#[test]
fn merge_without_compiling() {
    let mut container = ContainerBuilder::with_parameter_bag(
        ParameterBag::new(&php_value::php_array!["bar" => "foo"]).unwrap(),
    );
    container.set_resource_tracking(false);
    let config = ContainerBuilder::with_parameter_bag(
        ParameterBag::new(&php_value::php_array!["foo" => "bar"]).unwrap(),
    );
    container.merge(&config).unwrap();
    assert_eq!(
        &php_value::php_array!["bar" => "foo", "foo" => "bar"],
        container.get_parameter_bag().all()
    );

    let mut container = ContainerBuilder::new();
    container.set_resource_tracking(false);
    container.register("foo", Some("Bar\\FooClass")).unwrap();
    container.register("bar", Some("BarClass")).unwrap();
    let mut config = ContainerBuilder::new();
    config
        .set_definition("baz", Definition::with_class("BazClass"))
        .unwrap();
    config.set_alias("alias_for_foo", "foo").unwrap();
    container.merge(&config).unwrap();
    assert_eq!(
        vec!["service_container", "foo", "bar", "baz"],
        keys(container.get_definitions())
    );
    assert_eq!(
        "foo",
        container.get_aliases().get("alias_for_foo").unwrap().id()
    );

    let mut container = ContainerBuilder::new();
    container.set_resource_tracking(false);
    container.register("foo", Some("Bar\\FooClass")).unwrap();
    config
        .set_definition("foo", Definition::with_class("BazClass"))
        .unwrap();
    container.merge(&config).unwrap();
    assert_eq!(
        Some("BazClass".to_string()),
        container.get_definition("foo").unwrap().class()
    );

    let mut container = ContainerBuilder::new();
    let bag = ParameterBag::env_placeholder(&PhpArray::new()).unwrap();
    bag.get("env(Foo)").unwrap();
    let bar = bag.get("env(Bar)").unwrap();
    let mut config = ContainerBuilder::with_parameter_bag(bag);
    let resolved = config
        .resolve_env_placeholders(
            Value::Array(PhpArray::list([bar])),
            &EnvFormat::Placeholder,
            None,
        )
        .unwrap();
    assert_eq!(
        &PhpArray::list([s("%env(Bar)%")]),
        resolved.as_array().unwrap()
    );
    container.merge(&config).unwrap();
    let counters: Vec<(String, i64)> = container
        .get_env_counters()
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    assert_eq!(
        vec![("Bar".to_string(), 1), ("Foo".to_string(), 0)],
        counters
    );

    let mut container = ContainerBuilder::new();
    let mut config = ContainerBuilder::new();
    let a = container.register_for_autoconfiguration("AInterface");
    let b = config.register_for_autoconfiguration("BInterface");
    container.merge(&config).unwrap();
    let instanceof = container.get_autoconfigured_instanceof();
    assert_eq!(vec!["AInterface", "BInterface"], keys(instanceof));
    assert!(a.ptr_eq(instanceof.get("AInterface").unwrap()));
    assert!(b.ptr_eq(instanceof.get("BInterface").unwrap()));
}

/// A definition that needs a PHP object is refused, not created.
#[test]
fn creating_a_service_is_refused() {
    let mut builder = ContainerBuilder::new();
    builder.register("foo", Some("stdClass")).unwrap();
    let e = builder.get("foo", InvalidBehavior::Exception).unwrap_err();
    assert_eq!(
        "Creating the service \"foo\" needs PHP object creation, which this build-time port does not do; the Rust emitter replaces it.",
        e.message()
    );
    // A missing reference resolves to null with NULL_ON_INVALID_REFERENCE.
    let value = builder
        .resolve_services(&Value::Array(PhpArray::list([
            Value::from(Reference::with_behavior("missing", InvalidBehavior::Null)),
            s("plain"),
        ])))
        .unwrap();
    assert_eq!(
        &PhpArray::list([Value::Null, s("plain")]),
        value.as_array().unwrap()
    );
}

/// Removed ids skip dotted ones; `get()` of an alias reaches the aliased
/// service.
#[test]
fn removed_ids_and_aliases() {
    let mut builder = ContainerBuilder::new();
    builder.register("foo", Some("stdClass")).unwrap();
    builder.remove_definition("foo");
    builder.register(".hidden", Some("stdClass")).unwrap();
    builder.remove_definition(".hidden");
    assert_eq!(vec!["foo"], keys(builder.get_removed_ids()));
    builder.set_alias("alias", "bar").unwrap();
    builder.set("bar", Some(std_class())).unwrap();
    assert!(
        builder
            .get("alias", InvalidBehavior::Exception)
            .unwrap()
            .is_some()
    );
}

/// `ContainerBuilder::hash()` and the class kinds of the metadata.
#[test]
fn hash_is_seven_characters() {
    let hash = ContainerBuilder::hash(&s("foo"));
    assert_eq!(7, hash.len());
    assert!(!hash.contains(['/', '+']));
    assert_eq!(ClassKind::Interface, ClassMetadata::interface("I").kind);
}

/// `Compiler::compile()` turns env placeholders in a pass's message back
/// into `%env()%` and wraps the error in an `EnvParameterException`.
#[test]
fn compile_failure_with_env_placeholder() {
    struct Failing(String);
    impl CompilerPassInterface for Failing {
        fn class(&self) -> &'static str {
            "Failing"
        }
        fn process(&mut self, _container: &mut ContainerBuilder) -> Result<()> {
            Err(Error::runtime(format!("bad value {}", self.0)))
        }
    }
    let mut container = ContainerBuilder::new();
    let placeholder = container.get_parameter("env(FOO)").unwrap();
    let placeholder = placeholder
        .as_string()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let config = container.get_compiler().get_pass_config_mut();
    config.set_merge_pass(Pass::new(Failing(placeholder)));
    let e = container.compile(false).unwrap_err();
    assert!(e.is(&ErrorKind::EnvParameter));
    assert_eq!(
        "Incompatible use of dynamic environment variables \"FOO\" found in parameters.",
        e.message()
    );
    assert_eq!("bad value %env(FOO)%", e.previous().unwrap().message());
}
