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
pub use tag_decorator_pass::TagDecoratorPass;
pub use validate_env_placeholders_pass::ValidateEnvPlaceholdersPass;
