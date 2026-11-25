//! Port of the symfony/mime component: the MIME type part that
//! `HttpFoundation`'s `File` and `UploadedFile` use (`MimeTypes`, its
//! guesser interface, `FileBinaryMimeTypeGuesser` and
//! `FileinfoMimeTypeGuesser`). See README.md for
//! the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod exception;
pub mod file_binary_mime_type_guesser;
pub mod fileinfo_mime_type_guesser;
mod map;
pub mod mime_types;

pub use exception::{Class, Error, Result};
pub use file_binary_mime_type_guesser::FileBinaryMimeTypeGuesser;
pub use fileinfo_mime_type_guesser::FileinfoMimeTypeGuesser;
pub use mime_types::{MimeTypeGuesser, MimeTypes};
