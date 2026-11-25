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
#[expect(clippy::module_inception, reason = "one module per upstream file")]
mod compiler;
mod compiler_pass_interface;
mod deep_clone;
mod export_type;
mod pass_config;
mod service_reference_graph;

pub use abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState, recursive_process};
pub use compiler::Compiler;
pub use compiler_pass_interface::{CompilerPassInterface, Pass};
pub use pass_config::{PassConfig, PassType, PendingPass};
pub use service_reference_graph::{
    EdgeFlags, EdgeId, NodeId, ServiceReferenceGraph, ServiceReferenceGraphEdge,
    ServiceReferenceGraphNode,
};

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
