//! The Definition\Dumper namespace: reference configurations of a tree.
//!
//! Upstream: none

pub mod xml_reference_dumper;
#[cfg(feature = "yaml")]
pub mod yaml_reference_dumper;

pub use xml_reference_dumper::XmlReferenceDumper;
#[cfg(feature = "yaml")]
pub use yaml_reference_dumper::YamlReferenceDumper;
