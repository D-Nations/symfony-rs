//! symfony/demo's compiled container, emitted as Rust by
//! `di-trial-compiler` in `build.rs`. Every class is a placeholder that
//! records how PHP would have built it; see the trial's README.
//!
//! Upstream: none

/// The emitted container (OUT_DIR/container.rs).
#[allow(
    clippy::all,
    clippy::pedantic,
    non_camel_case_types,
    unused_imports,
    unused_mut,
    unused_variables
)]
pub mod container {
    include!(concat!(env!("OUT_DIR"), "/container.rs"));
}

/// What the compiler found (OUT_DIR/report.txt).
pub const REPORT: &str = include_str!(concat!(env!("OUT_DIR"), "/report.txt"));
