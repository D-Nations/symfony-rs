//! UploadedFileTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. `setUp()` skips when
//! `file_uploads` is off; this port reads PHP's default (on). The MIME
//! guess in `testFileUploadsWithNoMimeType` runs where a guesser does
//! (upstream: `extension_loaded('fileinfo')`).
//!
//! Upstream: symfony/http-foundation Tests/File/UploadedFileTest.php
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod files;

use files::{Scratch, TestResult, exists, fixture, fixtures, realpath, skipped_without_guesser};
use php_value::{Number, PhpString};
use symfony_http_foundation::{
    php::{
        UPLOAD_ERR_CANT_WRITE, UPLOAD_ERR_EXTENSION, UPLOAD_ERR_FORM_SIZE, UPLOAD_ERR_INI_SIZE,
        UPLOAD_ERR_NO_FILE, UPLOAD_ERR_NO_TMP_DIR, UPLOAD_ERR_OK, UPLOAD_ERR_PARTIAL,
    },
    uploaded_file::UploadedFile,
};

const NS: &str = "Symfony\\Component\\HttpFoundation\\File\\Exception\\";

fn mime(s: &str) -> Option<PhpString> {
    Some(PhpString::from(s))
}

fn directory() -> Vec<u8> {
    fixture("directory")
}

#[test]
fn testConstructWhenFileNotExists() {
    let error = UploadedFile::new(fixture("not_here"), "original.gif", None, None, false)
        .expect_err("FileNotFoundException");
    assert!(error.is_a(&format!("{NS}FileNotFoundException")), "{error}");
}

#[test]
fn testFileUploadsWithNoMimeType() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        None,
        Some(UPLOAD_ERR_OK),
        false,
    )?;
    assert_eq!(
        &PhpString::from("application/octet-stream"),
        file.get_client_mime_type()
    );
    if !skipped_without_guesser() {
        assert_eq!(Some("image/gif".to_string()), file.get_mime_type()?);
    }
    Ok(())
}

#[test]
fn testFileUploadsWithUnknownMimeType() -> TestResult {
    let file = UploadedFile::new(
        fixture(".unknownextension"),
        "original.gif",
        None,
        Some(UPLOAD_ERR_OK),
        false,
    )?;
    assert_eq!(
        &PhpString::from("application/octet-stream"),
        file.get_client_mime_type()
    );
    Ok(())
}

#[test]
fn testGuessClientExtension() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(Some("gif".to_string()), file.guess_client_extension());
    Ok(())
}

#[test]
fn testGuessClientExtensionWithIncorrectMimeType() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/png"),
        None,
        false,
    )?;
    assert_eq!(Some("png".to_string()), file.guess_client_extension());
    Ok(())
}

#[test]
fn testCaseSensitiveMimeType() -> TestResult {
    let file = UploadedFile::new(
        fixture("case-sensitive-mime-type.xlsm"),
        "test.xlsm",
        mime("application/vnd.ms-excel.sheet.macroEnabled.12"),
        None,
        false,
    )?;
    assert_eq!(Some("xlsm".to_string()), file.guess_client_extension());
    Ok(())
}

#[test]
fn testErrorIsOkByDefault() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(UPLOAD_ERR_OK, file.get_error());
    Ok(())
}

#[test]
fn testInvalidFile() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    let error = file.move_to(&directory(), None).expect_err("FileException");
    assert!(error.is_a(&format!("{NS}FileException")), "{error}");
    assert_eq!(
        "The file \"original.gif\" was not uploaded due to an unknown error.",
        error.message().to_string_lossy()
    );
    Ok(())
}

#[test]
fn testNoErrorMessageIfErrorIsUploadErrOk() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(PhpString::default(), file.get_error_message());
    Ok(())
}

#[test]
fn testGetClientOriginalName() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(
        &PhpString::from("original.gif"),
        file.get_client_original_name()
    );
    Ok(())
}

#[test]
fn testGetClientOriginalExtension() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(PhpString::from("gif"), file.get_client_original_extension());
    Ok(())
}

#[test]
fn testMoveLocalFileIsNotAllowed() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        Some(UPLOAD_ERR_OK),
        false,
    )?;
    let error = file.move_to(&directory(), None).expect_err("FileException");
    assert!(error.is_a(&format!("{NS}FileException")), "{error}");
    Ok(())
}

/// `failedUploadedFile()`.
fn failed_uploaded_file() -> Vec<UploadedFile> {
    [
        UPLOAD_ERR_INI_SIZE,
        UPLOAD_ERR_FORM_SIZE,
        UPLOAD_ERR_PARTIAL,
        UPLOAD_ERR_NO_FILE,
        UPLOAD_ERR_CANT_WRITE,
        UPLOAD_ERR_NO_TMP_DIR,
        UPLOAD_ERR_EXTENSION,
        -1,
    ]
    .into_iter()
    .map(|error| {
        UploadedFile::new(
            fixture("test.gif"),
            "original.gif",
            mime("image/gif"),
            Some(error),
            false,
        )
        .expect("an upload with an error is not checked")
    })
    .collect()
}

#[test]
fn testMoveFailed() {
    for file in failed_uploaded_file() {
        let class = match file.get_error() {
            UPLOAD_ERR_INI_SIZE => "IniSizeFileException",
            UPLOAD_ERR_FORM_SIZE => "FormSizeFileException",
            UPLOAD_ERR_PARTIAL => "PartialFileException",
            UPLOAD_ERR_NO_FILE => "NoFileException",
            UPLOAD_ERR_CANT_WRITE => "CannotWriteFileException",
            UPLOAD_ERR_NO_TMP_DIR => "NoTmpDirFileException",
            UPLOAD_ERR_EXTENSION => "ExtensionFileException",
            _ => "FileException",
        };
        let error = file.move_to(&directory(), None).expect_err(class);
        assert!(
            error.is_a(&format!("{NS}{class}")),
            "{}: {error}",
            file.get_error()
        );
    }
}

#[test]
fn testMoveLocalFileIsAllowedInTestMode() -> TestResult {
    let dir = Scratch::new("uploaded-test-mode")?;
    let path = dir.path("test.copy.gif");
    let target_path = dir.path("directory/test.copy.gif");
    std::fs::copy(dir.path("test.gif"), &path)?;

    let file = UploadedFile::new(
        path.clone(),
        "original.gif",
        mime("image/gif"),
        Some(UPLOAD_ERR_OK),
        true,
    )?;
    let moved = file.move_to(dir.path("directory").as_bytes(), None)?;

    assert!(exists(&target_path));
    assert!(!exists(&path));
    assert_eq!(realpath(&target_path), moved.get_real_path());
    Ok(())
}

#[test]
fn testGetClientOriginalNameSanitizeFilename() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "../../original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(
        &PhpString::from("original.gif"),
        file.get_client_original_name()
    );
    Ok(())
}

#[test]
fn testGetSize() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    let size = std::fs::metadata(format!("{}/test.gif", fixtures()))?.len();
    assert_eq!(i64::try_from(size)?, file.get_size()?);

    let file = UploadedFile::new(
        fixture("test"),
        "original.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    let size = std::fs::metadata(format!("{}/test", fixtures()))?.len();
    assert_eq!(i64::try_from(size)?, file.get_size()?);
    Ok(())
}

#[test]
fn testGetExtension() -> TestResult {
    let file = UploadedFile::new(fixture("test.gif"), "original.gif", None, None, false)?;
    assert_eq!(b"gif", file.get_extension());
    Ok(())
}

#[test]
fn testIsValid() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        None,
        Some(UPLOAD_ERR_OK),
        true,
    )?;
    assert!(file.is_valid());
    Ok(())
}

/// `uploadedFileErrorProvider()`.
const UPLOADED_FILE_ERRORS: [i64; 5] = [
    UPLOAD_ERR_INI_SIZE,
    UPLOAD_ERR_FORM_SIZE,
    UPLOAD_ERR_PARTIAL,
    UPLOAD_ERR_NO_TMP_DIR,
    UPLOAD_ERR_EXTENSION,
];

#[test]
fn testIsInvalidOnUploadError() -> TestResult {
    for error in UPLOADED_FILE_ERRORS {
        let file = UploadedFile::new(
            fixture("test.gif"),
            "original.gif",
            None,
            Some(error),
            false,
        )?;
        assert!(!file.is_valid(), "{error}");
    }
    Ok(())
}

#[test]
fn testIsInvalidIfNotHttpUpload() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "original.gif",
        None,
        Some(UPLOAD_ERR_OK),
        false,
    )?;
    assert!(!file.is_valid());
    Ok(())
}

#[test]
fn testGetMaxFilesize() {
    let size = UploadedFile::get_max_filesize();
    // PHP's defaults (8M, 2M) fit an int; neither is 0
    let Number::Int(size) = size else {
        panic!("not an int: {size:?}");
    };
    assert!(size > 0);
}

#[test]
fn testgetClientOriginalPath() -> TestResult {
    let file = UploadedFile::new(
        fixture("test.gif"),
        "test.gif",
        mime("image/gif"),
        None,
        false,
    )?;
    assert_eq!(
        &PhpString::from("test.gif"),
        file.get_client_original_path()
    );
    Ok(())
}

#[test]
fn testgetClientOriginalPathWebkitDirectory() -> TestResult {
    let file = UploadedFile::new(
        fixture("webkitdirectory/test.txt"),
        "webkitdirectory/test.txt",
        mime("text/plain"),
        None,
        false,
    )?;
    assert_eq!(
        &PhpString::from("webkitdirectory/test.txt"),
        file.get_client_original_path()
    );
    Ok(())
}
