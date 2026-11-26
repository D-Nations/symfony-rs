//! Build-time emitter of a Rust container from a compiled Symfony service
//! graph, for `build.rs`: the part of PLAN.md's M3 that replaces
//! `PhpDumper`.
//!
//! The graph ([`model::Graph`]) is read from a compiled
//! `ContainerBuilder` of `symfony-dependency-injection`
//! ([`Graph::from_container_builder`]), or from the `XmlDumper` file of a
//! container compiled by PHP ([`Graph::from_xml`]), and gives the same
//! graph either way. [`compile`] checks it and emits Rust with one
//! accessor per service:
//!
//! - every reference is checked at build time (a missing service is a
//!   build error, with Symfony's message);
//! - shared services live in cells, non-shared and inline services are
//!   built at each use, as `PhpDumper` does;
//! - each service is application-scoped or request-scoped, and a service
//!   that reaches a request-scoped one (eagerly or through a closure) is
//!   request-scoped too;
//! - circular references through method calls work as in PHP (the
//!   service is stored before its calls run); constructor cycles are
//!   refused.
//!
//! Classes are still the DI trial's placeholders (`di-trial-runtime`):
//! the emitted code builds a recorded `Instance` where the real container
//! would call the ported class's constructor.
//!
//! Upstream: none

mod builder;
mod emit;
pub mod model;
mod scope;
mod xml;

use std::fmt;

use symfony_dependency_injection::ContainerBuilder;

pub use model::{DumperParameters, Graph};
pub use scope::Scope;

/// Where a refusal points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// Nothing more precise is known.
    Unknown,
    /// A line of the container file.
    Line(u32),
    /// A service (or alias) id of a `ContainerBuilder`; for an inline
    /// definition, the named service that holds it.
    Service(String),
    /// A parameter of a `ContainerBuilder`.
    Parameter(String),
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => Ok(()),
            Self::Line(line) => write!(f, "line {line}"),
            Self::Service(id) => write!(f, "service \"{id}\""),
            Self::Parameter(name) => write!(f, "parameter \"{name}\""),
        }
    }
}

/// A refusal at build time, with where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub location: Location,
    pub message: String,
}

impl Error {
    pub fn new(location: Location, message: impl Into<String>) -> Self {
        Self {
            location,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Location::Unknown => f.write_str(&self.message),
            _ => write!(f, "{}: {}", self.location, self.message),
        }
    }
}

impl std::error::Error for Error {}

impl Graph {
    /// The graph of a compiled `ContainerBuilder`.
    pub fn from_container_builder(
        container: &ContainerBuilder,
        dumper_parameters: &DumperParameters,
    ) -> Result<Self, Error> {
        builder::read(container, dumper_parameters)
    }

    /// The graph of a compiled container as `XmlDumper` writes it (for
    /// containers recorded from PHP).
    pub fn from_xml(xml: &[u8], dumper_parameters: &DumperParameters) -> Result<Self, Error> {
        xml::read(xml, dumper_parameters)
    }
}

/// What the application tells the emitter that the graph does not say.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Tags whose services keep state between uses (Symfony's own list
    /// for long-running workers is `kernel.reset`).
    pub request_scoped_tags: Vec<String>,
    /// Further service ids that are request-scoped.
    pub request_scoped_ids: Vec<String>,
}

/// The emitted code and what the emitter found.
#[derive(Debug)]
pub struct Output {
    pub code: String,
    pub report: Report,
}

#[derive(Debug, Default, Clone)]
pub struct Report {
    /// Named definitions (aliases and inline definitions not counted).
    pub services: usize,
    pub inline: usize,
    pub aliases: usize,
    pub classes: usize,
    pub app_scoped: usize,
    pub request_scoped: usize,
    /// Request-scoped because of the configuration (before propagation).
    pub request_seeds: usize,
    /// Cycles through method calls or properties (allowed, as in PHP),
    /// as service id paths.
    pub call_cycles: Vec<Vec<String>>,
    /// Distinct `%env()%` expressions, evaluated at run time.
    pub env: Vec<String>,
    /// References to missing services that resolve to null or nothing
    /// (`on-invalid` other than `exception`).
    pub missing_optional: Vec<String>,
}

/// Checks the graph and emits it as Rust source.
pub fn compile(graph: &Graph, config: &Config) -> Result<Output, Error> {
    let analysis = scope::analyse(graph, config)?;
    let code = emit::emit(graph, &analysis)?;
    Ok(Output {
        code,
        report: analysis.report,
    })
}

/// Reads an `XmlDumper` file and compiles it, with the dumper parameters
/// derived from the file.
pub fn compile_xml(xml: &[u8], config: &Config) -> Result<Output, Error> {
    let graph = Graph::from_xml(xml, &DumperParameters::from_seed(xml))?;
    compile(&graph, config)
}
