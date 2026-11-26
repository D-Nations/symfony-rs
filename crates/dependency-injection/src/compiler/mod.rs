//! The compiler of Symfony\Component\DependencyInjection\Compiler: the
//! pass interface, the pass list, the compiler, the recursive value
//! walker and the service reference graph.
//!
//! The passes themselves are ported in later stages. Until then, each
//! pass of upstream's default list is a [`PendingPass`] that refuses to
//! run, so compiling a container fails with the name of the first pass
//! that is missing rather than skipping it.
//!
//! Upstream: none

mod abstract_recursive_pass;
mod add_behavior_describing_tags_pass;
mod attribute_autoconfiguration_pass;
mod auto_alias_service_pass;
mod autowire_as_decorator_pass;
mod autowire_required_methods_pass;
mod autowire_required_properties_pass;
#[expect(clippy::module_inception, reason = "one module per upstream file")]
mod compiler;
mod compiler_pass_interface;
mod deep_clone;
mod export_type;
mod extension_compiler_pass;
mod merge_extension_configuration_pass;
mod pass_config;
mod priority_tagged_service_trait;
mod register_autoconfigure_attributes_pass;
mod register_env_var_processors_pass;
mod register_reverse_container_pass;
mod register_service_subscribers_pass;
mod resettable_service_pass;
mod resolve_tagged_iterator_argument_pass;
mod service_locator_tag_pass;
mod service_reference_graph;
mod tag_decorator_pass;
mod validate_env_placeholders_pass;

pub use abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState, recursive_process};
pub use add_behavior_describing_tags_pass::AddBehaviorDescribingTagsPass;
pub use attribute_autoconfiguration_pass::AttributeAutoconfigurationPass;
pub use auto_alias_service_pass::AutoAliasServicePass;
pub use autowire_as_decorator_pass::AutowireAsDecoratorPass;
pub use autowire_required_methods_pass::AutowireRequiredMethodsPass;
pub use autowire_required_properties_pass::AutowireRequiredPropertiesPass;
pub use compiler::Compiler;
pub use compiler_pass_interface::{CompilerPassInterface, Pass};
pub use extension_compiler_pass::ExtensionCompilerPass;
pub use merge_extension_configuration_pass::MergeExtensionConfigurationPass;
pub use pass_config::{PassConfig, PassType, PendingPass};
pub use priority_tagged_service_trait::{
    AS_TAGGED_ITEM, TagName, attribute_argument, class_attributes, find_and_sort_tagged_services,
    get_default,
};
pub use register_autoconfigure_attributes_pass::RegisterAutoconfigureAttributesPass;
pub use register_env_var_processors_pass::RegisterEnvVarProcessorsPass;
pub use register_reverse_container_pass::RegisterReverseContainerPass;
pub use register_service_subscribers_pass::RegisterServiceSubscribersPass;
pub use resettable_service_pass::ResettableServicePass;
pub use resolve_tagged_iterator_argument_pass::ResolveTaggedIteratorArgumentPass;
pub use service_locator_tag_pass::ServiceLocatorTagPass;
pub use service_reference_graph::{
    EdgeFlags, EdgeId, NodeId, ServiceReferenceGraph, ServiceReferenceGraphEdge,
    ServiceReferenceGraphNode,
};

mod alias_deprecated_public_services_pass;
pub use alias_deprecated_public_services_pass::AliasDeprecatedPublicServicesPass;

mod analyze_service_references_pass;
pub use analyze_service_references_pass::AnalyzeServiceReferencesPass;

mod check_alias_validity_pass;
pub use check_alias_validity_pass::CheckAliasValidityPass;

mod check_arguments_validity_pass;
pub use check_arguments_validity_pass::CheckArgumentsValidityPass;

mod check_circular_references_pass;
pub use check_circular_references_pass::CheckCircularReferencesPass;

mod check_definition_validity_pass;
pub use check_definition_validity_pass::CheckDefinitionValidityPass;

mod check_exception_on_invalid_reference_behavior_pass;
pub use check_exception_on_invalid_reference_behavior_pass::CheckExceptionOnInvalidReferenceBehaviorPass;

mod check_factory_builder_circular_reference_pass;
pub use check_factory_builder_circular_reference_pass::CheckFactoryBuilderCircularReferencePass;

mod check_reference_validity_pass;
pub use check_reference_validity_pass::CheckReferenceValidityPass;

mod check_type_declarations_pass;
pub use check_type_declarations_pass::CheckTypeDeclarationsPass;

mod decorator_service_pass;
pub use decorator_service_pass::DecoratorServicePass;

mod definition_error_exception_pass;
pub use definition_error_exception_pass::DefinitionErrorExceptionPass;

mod inline_service_definitions_pass;
pub use inline_service_definitions_pass::InlineServiceDefinitionsPass;

mod remove_abstract_definitions_pass;
pub use remove_abstract_definitions_pass::RemoveAbstractDefinitionsPass;

mod remove_build_parameters_pass;
pub use remove_build_parameters_pass::RemoveBuildParametersPass;

mod remove_private_aliases_pass;
pub use remove_private_aliases_pass::RemovePrivateAliasesPass;

mod remove_unused_definitions_pass;
pub use remove_unused_definitions_pass::RemoveUnusedDefinitionsPass;

mod replace_alias_by_actual_definition_pass;
pub use replace_alias_by_actual_definition_pass::ReplaceAliasByActualDefinitionPass;

mod resolve_bindings_pass;
pub use resolve_bindings_pass::ResolveBindingsPass;

mod resolve_child_definitions_pass;
pub use resolve_child_definitions_pass::ResolveChildDefinitionsPass;

mod resolve_class_pass;
pub use resolve_class_pass::ResolveClassPass;

mod resolve_decorator_stack_pass;
pub use resolve_decorator_stack_pass::ResolveDecoratorStackPass;

mod resolve_env_placeholders_pass;
pub use resolve_env_placeholders_pass::ResolveEnvPlaceholdersPass;

mod resolve_factory_class_pass;
pub use resolve_factory_class_pass::ResolveFactoryClassPass;

mod resolve_hot_path_pass;
pub use resolve_hot_path_pass::ResolveHotPathPass;

mod resolve_instanceof_conditionals_pass;
pub use resolve_instanceof_conditionals_pass::ResolveInstanceofConditionalsPass;

mod resolve_invalid_references_pass;
pub use resolve_invalid_references_pass::ResolveInvalidReferencesPass;

mod resolve_named_arguments_pass;
pub use resolve_named_arguments_pass::ResolveNamedArgumentsPass;

mod resolve_no_preload_pass;
pub use resolve_no_preload_pass::ResolveNoPreloadPass;

mod resolve_parameter_place_holders_pass;
pub use resolve_parameter_place_holders_pass::ResolveParameterPlaceHoldersPass;

mod resolve_references_to_aliases_pass;
pub use resolve_references_to_aliases_pass::ResolveReferencesToAliasesPass;

mod resolve_service_subscribers_pass;
pub use resolve_service_subscribers_pass::ResolveServiceSubscribersPass;

pub use tag_decorator_pass::TagDecoratorPass;
pub use validate_env_placeholders_pass::ValidateEnvPlaceholdersPass;
