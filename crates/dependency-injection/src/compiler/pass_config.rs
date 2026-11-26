//! Port of Symfony\Component\DependencyInjection\Compiler\PassConfig.php.
//!
//! Upstream's default passes, in upstream's order and with its
//! priorities. A pass that is not ported yet is a [`PendingPass`] named
//! after upstream's class (with the constructor arguments upstream
//! passes): it stays in its place in the list and refuses to run, so
//! compiling fails at the first missing pass instead of skipping it.
//!
//! Upstream: symfony/dependency-injection Compiler/PassConfig.php

use std::collections::BTreeMap;

use super::alias_deprecated_public_services_pass::AliasDeprecatedPublicServicesPass;
use super::analyze_service_references_pass::AnalyzeServiceReferencesPass;
use super::check_arguments_validity_pass::CheckArgumentsValidityPass;
use super::check_circular_references_pass::CheckCircularReferencesPass;
use super::check_definition_validity_pass::CheckDefinitionValidityPass;
use super::check_exception_on_invalid_reference_behavior_pass::CheckExceptionOnInvalidReferenceBehaviorPass;
use super::check_factory_builder_circular_reference_pass::CheckFactoryBuilderCircularReferencePass;
use super::check_reference_validity_pass::CheckReferenceValidityPass;
use super::compiler_pass_interface::{CompilerPassInterface, Pass};
use super::decorator_service_pass::DecoratorServicePass;
use super::definition_error_exception_pass::DefinitionErrorExceptionPass;
use super::inline_service_definitions_pass::InlineServiceDefinitionsPass;
use super::remove_abstract_definitions_pass::RemoveAbstractDefinitionsPass;
use super::remove_build_parameters_pass::RemoveBuildParametersPass;
use super::remove_private_aliases_pass::RemovePrivateAliasesPass;
use super::remove_unused_definitions_pass::RemoveUnusedDefinitionsPass;
use super::replace_alias_by_actual_definition_pass::ReplaceAliasByActualDefinitionPass;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Compiler\\";

/// `PassConfig::TYPE_*`, in the order of execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassType {
    /// `TYPE_BEFORE_OPTIMIZATION`.
    BeforeOptimization,
    /// `TYPE_OPTIMIZE`.
    Optimize,
    /// `TYPE_BEFORE_REMOVING`.
    BeforeRemoving,
    /// `TYPE_REMOVE`.
    Remove,
    /// `TYPE_AFTER_REMOVING`.
    AfterRemoving,
}

impl PassType {
    /// The constant's value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeOptimization => "beforeOptimization",
            Self::Optimize => "optimization",
            Self::BeforeRemoving => "beforeRemoving",
            Self::Remove => "removing",
            Self::AfterRemoving => "afterRemoving",
        }
    }

    /// The type named `name`, or `addPass()`'s error for another name.
    pub fn from_name(name: &str) -> Result<Self> {
        [
            Self::BeforeOptimization,
            Self::Optimize,
            Self::BeforeRemoving,
            Self::Remove,
            Self::AfterRemoving,
        ]
        .into_iter()
        .find(|t| t.as_str() == name)
        .ok_or_else(|| Error::invalid_argument(format!("Invalid type \"{name}\".")))
    }
}

/// A pass of upstream's default list that this port does not have yet.
/// Running it fails with its class name.
#[derive(Debug, Clone)]
pub struct PendingPass {
    class: String,
    arguments: &'static str,
}

impl PendingPass {
    /// `new <short>(<arguments>)` of the `Compiler` namespace.
    pub fn new(short: &str, arguments: &'static str) -> Self {
        Self {
            class: format!("{NS}{short}"),
            arguments,
        }
    }

    /// The constructor arguments upstream passes, as PHP source.
    pub fn arguments(&self) -> &str {
        self.arguments
    }
}

impl CompilerPassInterface for PendingPass {
    fn class(&self) -> &str {
        &self.class
    }

    fn process(&mut self, _container: &mut ContainerBuilder) -> Result<()> {
        Err(Error::logic(format!(
            "The compiler pass \"{}\" is not ported yet: this build cannot compile a container that needs it.",
            self.class
        )))
    }
}

#[expect(
    dead_code,
    reason = "every default pass is ported; kept for passes added later"
)]
fn pending(short: &str) -> Pass {
    Pass::new(PendingPass::new(short, ""))
}

#[expect(
    dead_code,
    reason = "every default pass is ported; kept for passes added later"
)]
fn pending_with(short: &str, arguments: &'static str) -> Pass {
    Pass::new(PendingPass::new(short, arguments))
}

/// Passes by priority (`array<int, CompilerPassInterface[]>`).
type Prioritized = BTreeMap<i64, Vec<Pass>>;

/// `PassConfig`: the passes of each type, by priority.
#[derive(Debug, Clone)]
pub struct PassConfig {
    merge_pass: Pass,
    after_removing: Prioritized,
    before_optimization: Prioritized,
    before_removing: Prioritized,
    optimization: Prioritized,
    removing: Prioritized,
}

impl Default for PassConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PassConfig {
    /// `new PassConfig()`: upstream's default passes.
    pub fn new() -> Self {
        let before_optimization = BTreeMap::from([
            (
                100,
                vec![
                    Pass::new(super::ResolveClassPass::new()),
                    Pass::new(super::RegisterAutoconfigureAttributesPass),
                    Pass::new(super::AutowireAsDecoratorPass),
                    Pass::new(super::AttributeAutoconfigurationPass::new()),
                    Pass::new(super::ResolveInstanceofConditionalsPass::new()),
                    Pass::new(super::RegisterEnvVarProcessorsPass),
                ],
            ),
            (-1000, vec![Pass::new(super::ExtensionCompilerPass)]),
        ]);
        let optimization = BTreeMap::from([(
            0,
            vec![
                Pass::new(super::AutoAliasServicePass),
                Pass::new(super::ValidateEnvPlaceholdersPass::new()),
                Pass::new(super::ResolveDecoratorStackPass::new()),
                Pass::new(super::ResolveAutowireInlineAttributesPass::new()),
                Pass::new(super::ResolveChildDefinitionsPass::new()),
                Pass::new(super::RegisterServiceSubscribersPass::new()),
                Pass::new(super::ResolveParameterPlaceHoldersPass::new(false, false)),
                Pass::new(super::ResolveFactoryClassPass::new()),
                Pass::new(super::ResolveNamedArgumentsPass::new()),
                Pass::new(super::AutowireRequiredMethodsPass::new()),
                Pass::new(super::AutowireRequiredPropertiesPass::new()),
                Pass::new(super::ResolveBindingsPass::new()),
                Pass::new(super::ServiceLocatorTagPass::new()),
                Pass::new(super::TagDecoratorPass),
                Pass::new(DecoratorServicePass::new()),
                Pass::new(CheckDefinitionValidityPass),
                Pass::new(super::AutowirePass::new(false)),
                Pass::new(super::ServiceLocatorTagPass::new()),
                Pass::new(super::ResolveTaggedIteratorArgumentPass::new()),
                Pass::new(super::ResolveServiceSubscribersPass::new()),
                Pass::new(super::ResolveReferencesToAliasesPass::new()),
                Pass::new(super::ResolveInvalidReferencesPass::new()),
                Pass::new(AnalyzeServiceReferencesPass::new(true, true)),
                Pass::new(CheckCircularReferencesPass::new()),
                Pass::new(CheckReferenceValidityPass::new()),
                Pass::new(CheckArgumentsValidityPass::new(false)),
            ],
        )]);
        let removing = BTreeMap::from([(
            0,
            vec![
                Pass::new(RemovePrivateAliasesPass),
                Pass::new(ReplaceAliasByActualDefinitionPass::new()),
                Pass::new(RemoveAbstractDefinitionsPass),
                Pass::new(RemoveUnusedDefinitionsPass::new()),
                Pass::new(AnalyzeServiceReferencesPass::default()),
                Pass::new(CheckExceptionOnInvalidReferenceBehaviorPass::new()),
                Pass::new(InlineServiceDefinitionsPass::new(Some(
                    AnalyzeServiceReferencesPass::default(),
                ))),
                Pass::new(AnalyzeServiceReferencesPass::default()),
                Pass::new(CheckFactoryBuilderCircularReferencePass),
                Pass::new(DefinitionErrorExceptionPass::new()),
            ],
        )]);
        let after_removing = BTreeMap::from([
            (
                0,
                vec![
                    Pass::new(super::ResolveHotPathPass::new()),
                    Pass::new(super::ResolveNoPreloadPass::new()),
                    Pass::new(AliasDeprecatedPublicServicesPass::new()),
                ],
            ),
            // Let build parameters be available as late as possible
            // Don't remove array parameters since ResolveParameterPlaceHoldersPass doesn't resolve them
            (-2048, vec![Pass::new(RemoveBuildParametersPass::new(true))]),
        ]);
        Self {
            merge_pass: Pass::new(super::MergeExtensionConfigurationPass::new()),
            after_removing,
            before_optimization,
            before_removing: BTreeMap::new(),
            optimization,
            removing,
        }
    }

    /// `getPasses()`: every pass in the order of execution.
    pub fn get_passes(&self) -> Vec<Pass> {
        let mut passes = vec![self.merge_pass.clone()];
        passes.extend(self.get_before_optimization_passes());
        passes.extend(self.get_optimization_passes());
        passes.extend(self.get_before_removing_passes());
        passes.extend(self.get_removing_passes());
        passes.extend(self.get_after_removing_passes());
        passes
    }

    fn list_mut(&mut self, type_: PassType) -> &mut Prioritized {
        match type_ {
            PassType::BeforeOptimization => &mut self.before_optimization,
            PassType::Optimize => &mut self.optimization,
            PassType::BeforeRemoving => &mut self.before_removing,
            PassType::Remove => &mut self.removing,
            PassType::AfterRemoving => &mut self.after_removing,
        }
    }

    /// `addPass($pass, $type, $priority)`.
    pub fn add_pass(&mut self, pass: Pass, type_: PassType, priority: i64) {
        self.list_mut(type_).entry(priority).or_default().push(pass);
    }

    pub fn get_after_removing_passes(&self) -> Vec<Pass> {
        sort_passes(&self.after_removing)
    }

    pub fn get_before_optimization_passes(&self) -> Vec<Pass> {
        sort_passes(&self.before_optimization)
    }

    pub fn get_before_removing_passes(&self) -> Vec<Pass> {
        sort_passes(&self.before_removing)
    }

    pub fn get_optimization_passes(&self) -> Vec<Pass> {
        sort_passes(&self.optimization)
    }

    pub fn get_removing_passes(&self) -> Vec<Pass> {
        sort_passes(&self.removing)
    }

    pub fn get_merge_pass(&self) -> Pass {
        self.merge_pass.clone()
    }

    pub fn set_merge_pass(&mut self, pass: Pass) {
        self.merge_pass = pass;
    }

    /// `setAfterRemovingPasses($passes)`: the list, at priority 0.
    pub fn set_after_removing_passes(&mut self, passes: Vec<Pass>) {
        self.after_removing = BTreeMap::from([(0, passes)]);
    }

    pub fn set_before_optimization_passes(&mut self, passes: Vec<Pass>) {
        self.before_optimization = BTreeMap::from([(0, passes)]);
    }

    pub fn set_before_removing_passes(&mut self, passes: Vec<Pass>) {
        self.before_removing = BTreeMap::from([(0, passes)]);
    }

    pub fn set_optimization_passes(&mut self, passes: Vec<Pass>) {
        self.optimization = BTreeMap::from([(0, passes)]);
    }

    pub fn set_removing_passes(&mut self, passes: Vec<Pass>) {
        self.removing = BTreeMap::from([(0, passes)]);
    }
}

/// `sortPasses()`: `krsort()`, then flattened.
fn sort_passes(passes: &Prioritized) -> Vec<Pass> {
    passes.values().rev().flatten().cloned().collect()
}
