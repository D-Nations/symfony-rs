//! Trial build-time DI compiler: reads the container Symfony compiled
//! (after its compiler passes, as `XmlDumper` writes it) and emits a Rust
//! container with one accessor per service, for `build.rs`.
//!
//! **This is not the M3 port.** It tests the design in PLAN.md (M3, and
//! converter contract 0.2 section 4) on symfony/demo's real container:
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
//! Classes are placeholders (`di-trial-runtime`): the emitted code builds
//! a recorded `Instance` where the real container would call the ported
//! class's constructor.
//!
//! Upstream: none

mod emit;
pub mod model;
mod scope;

use std::fmt;

pub use scope::Scope;

/// A refusal at build time, with the line in the container file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: u32,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

/// What the application tells the compiler that the container file does
/// not say.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Tags whose services keep state between uses (Symfony's own list
    /// for long-running workers is `kernel.reset`).
    pub request_scoped_tags: Vec<String>,
    /// Further service ids that are request-scoped.
    pub request_scoped_ids: Vec<String>,
}

/// The emitted code and what the compiler found.
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

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Compiles the container file into Rust source.
pub fn compile(xml: &[u8], config: &Config) -> Result<Output, Error> {
    // PhpDumper adds these when it writes the PHP container; they are not
    // in the ContainerBuilder. Symfony derives them from a hash of the
    // dumped code; here, from the input file.
    let hash = fnv1a(xml);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 32 bits name the build"
    )]
    let build_id = format!("{:08x}", hash as u32);
    let dumper_parameters = [
        (
            "container.build_hash".to_owned(),
            php_value::Value::string(format!("{hash:016x}")),
        ),
        (
            "container.build_id".to_owned(),
            php_value::Value::string(build_id),
        ),
    ];
    let graph = model::read(xml, &dumper_parameters)?;
    let analysis = scope::analyse(&graph, config)?;
    let code = emit::emit(&graph, &analysis)?;
    Ok(Output {
        code,
        report: analysis.report,
    })
}
