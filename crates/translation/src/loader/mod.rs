//! Symfony\Component\Translation\Loader: the XLIFF and Qt loaders.
//!
//! Upstream: none

mod local;
pub mod qt_file_loader;
pub mod xliff_file_loader;

pub use qt_file_loader::QtFileLoader;
pub use xliff_file_loader::{LoaderInterface, XliffFileLoader};
