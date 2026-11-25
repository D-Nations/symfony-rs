//! Runtime pieces the fixture needs that the workspace crates do not have
//! yet. Each submodule names what it ports and covers only what the
//! fixture's recorded cases exercise; anything else is refused with
//! `NotPorted`, never approximated. `framework_bundle` belongs in a
//! `FrameworkBundle` crate (M3) and `php` in `php-value`
//! (docs/milestones/m2-a-converter-fixture-kernel.md); generated code is not
//! meant to carry them.
//!
//! Upstream: none

pub mod framework_bundle;
pub mod php;

/// Input the fixture's runtime does not port: an `\Exception` no listener
/// handles, so the kernel answers 500.
#[derive(Debug)]
pub struct NotPorted(pub &'static str);

impl std::fmt::Display for NotPorted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not ported in the converter fixture runtime: {}", self.0)
    }
}

impl std::error::Error for NotPorted {}
