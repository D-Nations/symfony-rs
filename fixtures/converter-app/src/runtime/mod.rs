//! Runtime pieces the fixture needs that the workspace crates do not have
//! yet. Each submodule names the upstream file it ports and covers only
//! what the fixture's recorded cases exercise; anything else is refused with
//! `NotPorted`, never approximated. These are the gaps M2-A must close in
//! `symfony-http-foundation`, `symfony-http-kernel` and a FrameworkBundle
//! crate (docs/milestones/m1-d-converter-fixture.md lists them); generated
//! code is not meant to carry them.
//!
//! Upstream: none

pub mod framework_bundle;
pub mod http_foundation;
pub mod listeners;
pub mod php;

/// Input the fixture's runtime does not port. Not an HTTP exception, so no
/// listener handles it and the kernel answers 500.
#[derive(Debug)]
pub struct NotPorted(pub &'static str);

impl std::fmt::Display for NotPorted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not ported in the converter fixture runtime: {}", self.0)
    }
}

impl std::error::Error for NotPorted {}

impl From<NotPorted> for symfony_http_kernel::Exception {
    fn from(error: NotPorted) -> Self {
        symfony_http_kernel::Exception::from_throwable(error)
    }
}
