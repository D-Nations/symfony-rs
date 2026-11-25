//! Tests/Compiler/PassConfigTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; the stubbed
//! `CompilerPassInterface`s are a pass that does nothing.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/PassConfigTest.php
#![allow(non_snake_case)]

use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::compiler::{CompilerPassInterface, Pass, PassConfig, PassType};

struct Stub;

impl CompilerPassInterface for Stub {
    fn class(&self) -> &'static str {
        "CompilerPassInterfaceStub"
    }

    fn process(
        &mut self,
        _container: &mut ContainerBuilder,
    ) -> symfony_dependency_injection::Result<()> {
        Ok(())
    }
}

#[test]
fn testPassOrdering() {
    let mut config = PassConfig::new();
    config.set_before_optimization_passes(Vec::new());

    let pass1 = Pass::new(Stub);
    config.add_pass(pass1.clone(), PassType::BeforeOptimization, 10);

    let pass2 = Pass::new(Stub);
    config.add_pass(pass2.clone(), PassType::BeforeOptimization, 30);

    let passes = config.get_before_optimization_passes();
    assert!(pass2.ptr_eq(&passes[0]));
    assert!(pass1.ptr_eq(&passes[1]));
}

#[test]
fn testPassOrderingWithoutPasses() {
    let mut config = PassConfig::new();
    config.set_before_optimization_passes(Vec::new());
    config.set_after_removing_passes(Vec::new());
    config.set_before_removing_passes(Vec::new());
    config.set_optimization_passes(Vec::new());
    config.set_removing_passes(Vec::new());

    assert!(config.get_before_optimization_passes().is_empty());
    assert!(config.get_after_removing_passes().is_empty());
    assert!(config.get_before_removing_passes().is_empty());
    assert!(config.get_optimization_passes().is_empty());
    assert!(config.get_removing_passes().is_empty());
}

/// Upstream's default list: its order, and passes that are not ported
/// yet refusing to run.
#[test]
fn default_passes_in_upstream_order() {
    let config = PassConfig::new();
    let names: Vec<String> = config
        .get_passes()
        .iter()
        .map(|p| {
            p.class()
                .trim_start_matches("Symfony\\Component\\DependencyInjection\\Compiler\\")
                .to_string()
        })
        .collect();
    assert_eq!(
        vec![
            "MergeExtensionConfigurationPass",
            "ResolveClassPass",
            "RegisterAutoconfigureAttributesPass",
            "AutowireAsDecoratorPass",
            "AttributeAutoconfigurationPass",
            "ResolveInstanceofConditionalsPass",
            "RegisterEnvVarProcessorsPass",
            "ExtensionCompilerPass",
            "AutoAliasServicePass",
            "ValidateEnvPlaceholdersPass",
            "ResolveDecoratorStackPass",
            "ResolveAutowireInlineAttributesPass",
            "ResolveChildDefinitionsPass",
            "RegisterServiceSubscribersPass",
            "ResolveParameterPlaceHoldersPass",
            "ResolveFactoryClassPass",
            "ResolveNamedArgumentsPass",
            "AutowireRequiredMethodsPass",
            "AutowireRequiredPropertiesPass",
            "ResolveBindingsPass",
            "ServiceLocatorTagPass",
            "TagDecoratorPass",
            "DecoratorServicePass",
            "CheckDefinitionValidityPass",
            "AutowirePass",
            "ServiceLocatorTagPass",
            "ResolveTaggedIteratorArgumentPass",
            "ResolveServiceSubscribersPass",
            "ResolveReferencesToAliasesPass",
            "ResolveInvalidReferencesPass",
            "AnalyzeServiceReferencesPass",
            "CheckCircularReferencesPass",
            "CheckReferenceValidityPass",
            "CheckArgumentsValidityPass",
            "RemovePrivateAliasesPass",
            "ReplaceAliasByActualDefinitionPass",
            "RemoveAbstractDefinitionsPass",
            "RemoveUnusedDefinitionsPass",
            "AnalyzeServiceReferencesPass",
            "CheckExceptionOnInvalidReferenceBehaviorPass",
            "InlineServiceDefinitionsPass",
            "AnalyzeServiceReferencesPass",
            "CheckFactoryBuilderCircularReferencePass",
            "DefinitionErrorExceptionPass",
            "ResolveHotPathPass",
            "ResolveNoPreloadPass",
            "AliasDeprecatedPublicServicesPass",
            "RemoveBuildParametersPass",
        ],
        names
    );

    let mut container = ContainerBuilder::new();
    let e = container.compile(false).unwrap_err();
    assert_eq!(
        "The compiler pass \"Symfony\\Component\\DependencyInjection\\Compiler\\MergeExtensionConfigurationPass\" is not ported yet: this build cannot compile a container that needs it.",
        e.message()
    );
    assert!(!container.is_compiled());
}

#[test]
fn invalid_pass_type() {
    assert_eq!(
        "Invalid type \"foo\".",
        PassType::from_name("foo").unwrap_err().message()
    );
    assert_eq!(PassType::Remove, PassType::from_name("removing").unwrap());
}
