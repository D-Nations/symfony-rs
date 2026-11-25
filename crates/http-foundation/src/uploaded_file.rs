//! Port of Symfony\Component\HttpFoundation\File\UploadedFile.php: a file
//! uploaded through a form, as `$_FILES` describes it.
//!
//! `isValid()` and `move()` outside test mode ask [`crate::php`]'s list of
//! uploaded files (`is_uploaded_file()`, `move_uploaded_file()`), which
//! the code that stores an upload fills. `getMaxFilesize()` reads PHP's
//! defaults (`post_max_size` 8M, `upload_max_filesize` 2M), as this crate
//! does elsewhere ([`crate::POST_MAX_SIZE`]); [`max_filesize`] gives the
//! answer for other ini texts. Checked against 108 recorded
//! `parseFilesize()` texts and 126 ini pairs.
//!
//! An `UploadedFile` is a PHP object ([`PhpObject`]), so a
//! [`crate::file_bag::FileBag`] holds it among its values; it has no
//! public properties.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation File/UploadedFile.php

use std::ops::Deref;

use php_value::{Number, PhpArray, PhpObject, PhpString, numeric::string_to_int};
use symfony_mime::MimeTypes;

use crate::{
    exception::{Class, Error, Result},
    file::{File, chmod_moved, get_name},
    php::{
        self, UPLOAD_ERR_CANT_WRITE, UPLOAD_ERR_EXTENSION, UPLOAD_ERR_FORM_SIZE,
        UPLOAD_ERR_INI_SIZE, UPLOAD_ERR_NO_FILE, UPLOAD_ERR_NO_TMP_DIR, UPLOAD_ERR_OK,
        UPLOAD_ERR_PARTIAL,
    },
};

pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\File\\UploadedFile";

/// PHP's default `post_max_size`.
pub const DEFAULT_POST_MAX_SIZE: &[u8] = b"8M";
/// PHP's default `upload_max_filesize`.
pub const DEFAULT_UPLOAD_MAX_FILESIZE: &[u8] = b"2M";

/// `new UploadedFile($path, $originalName, $mimeType, $error, $test)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    file: File,
    original_name: PhpString,
    mime_type: PhpString,
    error: i64,
    original_path: PhpString,
    test: bool,
}

impl Deref for UploadedFile {
    type Target = File;

    fn deref(&self) -> &File {
        &self.file
    }
}

impl PhpObject for UploadedFile {
    fn class(&self) -> &str {
        CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// `UploadedFile::parseFilesize($size)`: an ini quantity in bytes; it
/// becomes a float when the multiplication passes `PHP_INT_MAX`.
#[expect(clippy::cast_precision_loss, reason = "PHP's int to float overflow")]
pub fn parse_filesize(size: &[u8]) -> Number {
    if size.is_empty() {
        return Number::Int(0);
    }
    let size = size.to_ascii_lowercase();
    let start = size.iter().position(|&c| c != b'+').unwrap_or(size.len());
    let max = &size[start..];
    let max = if max.starts_with(b"0x") {
        php::intval_prefixed(max, 16)
    } else if max.starts_with(b"0") {
        php::intval_prefixed(max, 8)
    } else {
        string_to_int(max)
    };
    let times = match size.last() {
        Some(b't') => 4,
        Some(b'g') => 3,
        Some(b'm') => 2,
        Some(b'k') => 1,
        _ => 0,
    };
    let mut value = Number::Int(max);
    for _ in 0..times {
        value = match value {
            Number::Int(i) => i
                .checked_mul(1024)
                .map_or(Number::Float(i as f64 * 1024.0), Number::Int),
            Number::Float(f) => Number::Float(f * 1024.0),
        };
    }
    value
}

/// `$size ?: PHP_INT_MAX`.
fn or_max(size: Number) -> Number {
    match size {
        Number::Int(0) | Number::Float(0.0) => Number::Int(i64::MAX),
        other => other,
    }
}

/// `getMaxFilesize()` under these `post_max_size` and
/// `upload_max_filesize` texts.
pub fn max_filesize(post_max_size: &[u8], upload_max_filesize: &[u8]) -> Number {
    let post = or_max(parse_filesize(post_max_size));
    let upload = or_max(parse_filesize(upload_max_filesize));
    // min() keeps the first of two equal values; int and float compare
    // as floats
    let less = match (upload, post) {
        (Number::Int(a), Number::Int(b)) => a < b,
        (a, b) => a.as_f64() < b.as_f64(),
    };
    if less { upload } else { post }
}

/// `$number / 1024`, then `sprintf('%d', ...)`.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "PHP's int/float division and %d"
)]
fn kib(size: Number) -> i64 {
    let divided = match size {
        Number::Int(i) if i % 1024 == 0 => return i / 1024,
        Number::Int(i) => i as f64 / 1024.0,
        Number::Float(f) => f / 1024.0,
    };
    // the recorded limits stay in range; `as` truncates toward zero
    divided as i64
}

impl UploadedFile {
    /// `new UploadedFile($path, $originalName, $mimeType, $error, $test)`:
    /// a file whose error is `UPLOAD_ERR_OK` must exist
    /// (`FileNotFoundException`). `mime_type` `None`, `""` or `"0"` is
    /// `application/octet-stream`; `error` `None` or 0 is `UPLOAD_ERR_OK`.
    pub fn new(
        path: impl Into<Vec<u8>>,
        original_name: impl Into<PhpString>,
        mime_type: Option<PhpString>,
        error: Option<i64>,
        test: bool,
    ) -> Result<Self> {
        let original_name: PhpString = original_name.into();
        let name = get_name(original_name.as_bytes());
        let original_path: Vec<u8> = original_name
            .as_bytes()
            .iter()
            .map(|&c| if c == b'\\' { b'/' } else { c })
            .collect();
        let mime_type = match mime_type {
            Some(m) if !m.is_empty() && m.as_bytes() != b"0" => m,
            _ => PhpString::from("application/octet-stream"),
        };
        let error = error.unwrap_or(UPLOAD_ERR_OK);
        let file = File::new(path, error == UPLOAD_ERR_OK)?;
        Ok(Self {
            file,
            original_name: PhpString::from(name),
            mime_type,
            error,
            original_path: PhpString::from(original_path),
            test,
        })
    }

    /// The file as a plain `File`.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// `getClientOriginalName()`: the client's name, without directories.
    pub fn get_client_original_name(&self) -> &PhpString {
        &self.original_name
    }

    /// `getClientOriginalExtension()`.
    pub fn get_client_original_extension(&self) -> PhpString {
        PhpString::from(php::pathinfo_extension(self.original_name.as_bytes()))
    }

    /// `getClientOriginalPath()`: the client's name with `/` separators
    /// (a `webkitdirectory` upload's relative path).
    pub fn get_client_original_path(&self) -> &PhpString {
        &self.original_path
    }

    /// `getClientMimeType()`.
    pub fn get_client_mime_type(&self) -> &PhpString {
        &self.mime_type
    }

    /// `guessClientExtension()`: the first extension of the client's MIME
    /// type.
    pub fn guess_client_extension(&self) -> Option<String> {
        MimeTypes::get_default()
            .get_extensions(&self.mime_type.to_string_lossy())
            .into_iter()
            .next()
    }

    /// `getError()`.
    pub fn get_error(&self) -> i64 {
        self.error
    }

    /// `isValid()`: no error, and uploaded through HTTP unless in test
    /// mode.
    pub fn is_valid(&self) -> bool {
        let is_ok = self.error == UPLOAD_ERR_OK;
        if self.test {
            is_ok
        } else {
            is_ok && php::is_uploaded_file(self.file.get_pathname())
        }
    }

    /// `move($directory, $name)`.
    pub fn move_to(&self, directory: &[u8], name: Option<&[u8]>) -> Result<File> {
        if self.is_valid() {
            if self.test {
                return self.file.move_to(directory, name);
            }
            let target = self.file.get_target_file(directory, name)?;
            let (moved, warning) =
                php::move_uploaded_file(self.file.get_pathname(), target.get_pathname());
            if !moved {
                // the warning is already escaped as a web SAPI escapes it
                return Err(Error::new(
                    Class::File,
                    format!(
                        "Could not move the file \"{}\" to \"{}\" ({}).",
                        String::from_utf8_lossy(self.file.get_pathname()),
                        String::from_utf8_lossy(target.get_pathname()),
                        warning.unwrap_or_default()
                    ),
                ));
            }
            chmod_moved(&target);
            return Ok(target);
        }
        let class = match self.error {
            UPLOAD_ERR_INI_SIZE => Class::IniSizeFile,
            UPLOAD_ERR_FORM_SIZE => Class::FormSizeFile,
            UPLOAD_ERR_PARTIAL => Class::PartialFile,
            UPLOAD_ERR_NO_FILE => Class::NoFile,
            UPLOAD_ERR_CANT_WRITE => Class::CannotWriteFile,
            UPLOAD_ERR_NO_TMP_DIR => Class::NoTmpDirFile,
            UPLOAD_ERR_EXTENSION => Class::ExtensionFile,
            _ => Class::File,
        };
        Err(Error::new(class, self.exception_message()))
    }

    /// `getErrorMessage()`: `""` without an error.
    pub fn get_error_message(&self) -> PhpString {
        if self.error == UPLOAD_ERR_OK {
            PhpString::default()
        } else {
            self.exception_message()
        }
    }

    /// `UploadedFile::getMaxFilesize()` with PHP's default ini values.
    pub fn get_max_filesize() -> Number {
        max_filesize(DEFAULT_POST_MAX_SIZE, DEFAULT_UPLOAD_MAX_FILESIZE)
    }

    /// `getExceptionMessage()`.
    fn exception_message(&self) -> PhpString {
        exception_message(self.error, &self.original_name, Self::get_max_filesize())
    }
}

/// `getExceptionMessage()` for an error code, a client name and the
/// `getMaxFilesize()` answer.
pub fn exception_message(error: i64, name: &PhpString, max_filesize: Number) -> PhpString {
    let name = name.as_bytes();
    let quoted = |before: &str, after: &str| {
        let mut text = before.as_bytes().to_vec();
        text.extend_from_slice(name);
        text.extend_from_slice(after.as_bytes());
        PhpString::from(text)
    };
    match error {
        UPLOAD_ERR_INI_SIZE => quoted(
            "The file \"",
            &format!(
                "\" exceeds your upload_max_filesize ini directive (limit is {} KiB).",
                kib(max_filesize)
            ),
        ),
        UPLOAD_ERR_FORM_SIZE => quoted(
            "The file \"",
            "\" exceeds the upload limit defined in your form.",
        ),
        UPLOAD_ERR_PARTIAL => quoted("The file \"", "\" was only partially uploaded."),
        UPLOAD_ERR_NO_FILE => PhpString::from("No file was uploaded."),
        UPLOAD_ERR_CANT_WRITE => quoted("The file \"", "\" could not be written on disk."),
        UPLOAD_ERR_NO_TMP_DIR => {
            PhpString::from("File could not be uploaded: missing temporary directory.")
        }
        UPLOAD_ERR_EXTENSION => PhpString::from("File upload was stopped by a PHP extension."),
        _ => quoted(
            "The file \"",
            "\" was not uploaded due to an unknown error.",
        ),
    }
}
