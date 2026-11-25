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
mod extension_compiler_pass;
mod merge_extension_configuration_pass;
mod pass_config;
mod service_reference_graph;

pub use abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState, recursive_process};
pub use compiler::Compiler;
pub use compiler_pass_interface::{CompilerPassInterface, Pass};
pub use extension_compiler_pass::ExtensionCompilerPass;
pub use merge_extension_configuration_pass::MergeExtensionConfigurationPass;
pub use pass_config::{PassConfig, PassType, PendingPass};
pub use service_reference_graph::{
    EdgeFlags, EdgeId, NodeId, ServiceReferenceGraph, ServiceReferenceGraphEdge,
    ServiceReferenceGraphNode,
};
