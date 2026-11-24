//! AbstractMimeTypeGuesserTestCase.php at symfony/mime
//! 8d1232052c9cdef170ed9608b3147bca4f61ff69: the tests every guesser's
//! test class inherits, as functions of the guesser. `markTestSkipped()`
//! is a return with a note on stderr, under upstream's own conditions.
//! The fixtures are upstream's `Tests/Fixtures`, copied.
//!
//! Upstream: symfony/mime Tests/AbstractMimeTypeGuesserTestCase.php
#![allow(dead_code)]

use symfony_mime::{Class, MimeTypeGuesser};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `__DIR__.'/Fixtures'`.
pub fn fixtures() -> String {
    format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"))
}

pub fn fixture(name: &str) -> Vec<u8> {
    format!("{}/{name}", fixtures()).into_bytes()
}

/// `if (!$guesser->isGuesserSupported()) $this->markTestSkipped(...)`.
pub fn skipped_unsupported(guesser: &dyn MimeTypeGuesser) -> bool {
    let unsupported = !guesser.is_guesser_supported();
    if unsupported {
        eprintln!("skipped: Guesser is not supported");
    }
    unsupported
}

pub fn guess_with_leading_dash(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    // chdir() is the process's: every test that calls this moves to the
    // same directory, and no other test reads a relative path, so the
    // directory is not restored (a restore could race another test here).
    std::env::set_current_dir(format!("{}/mimetypes", fixtures()))?;
    assert_eq!(
        Some("image/gif".to_string()),
        guesser.guess_mime_type(b"-test")?
    );
    Ok(())
}

pub fn guess_image_without_extension(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    assert_eq!(
        Some("image/gif".to_string()),
        guesser.guess_mime_type(&fixture("mimetypes/test"))?
    );
    Ok(())
}

pub fn guess_image_with_directory(guesser: &dyn MimeTypeGuesser) {
    if skipped_unsupported(guesser) {
        return;
    }
    let error = guesser
        .guess_mime_type(&fixture("mimetypes/directory"))
        .expect_err("InvalidArgumentException");
    assert!(error.is_a("InvalidArgumentException"), "{error}");
}

pub fn guess_image_with_known_extension(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    assert_eq!(
        Some("image/gif".to_string()),
        guesser.guess_mime_type(&fixture("mimetypes/test.gif"))?
    );
    Ok(())
}

pub fn guess_file_with_unknown_extension(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    assert_eq!(
        Some("application/octet-stream".to_string()),
        guesser.guess_mime_type(&fixture("mimetypes/.unknownextension"))?
    );
    Ok(())
}

pub fn guess_with_duplicated_file_type(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    assert_eq!(
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string()),
        guesser.guess_mime_type(&fixture("test.docx"))?
    );
    Ok(())
}

pub fn guess_with_incorrect_path(guesser: &dyn MimeTypeGuesser) {
    if skipped_unsupported(guesser) {
        return;
    }
    let error = guesser
        .guess_mime_type(&fixture("mimetypes/not_here"))
        .expect_err("InvalidArgumentException");
    assert!(error.is_a("InvalidArgumentException"), "{error}");
    assert_eq!(Class::InvalidArgument, error.class());
}

pub fn guess_with_non_readable_path(guesser: &dyn MimeTypeGuesser) -> TestResult {
    if skipped_unsupported(guesser) {
        return Ok(());
    }
    if cfg!(windows) {
        eprintln!("skipped: Cannot verify chmod operations on Windows");
        return Ok(());
    }
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() || user == "root" {
        eprintln!("skipped: This test will fail if run under superuser");
        return Ok(());
    }
    non_readable(guesser)
}

#[cfg(unix)]
fn non_readable(guesser: &dyn MimeTypeGuesser) -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    // one file per guesser type, so parallel tests do not share it
    let path = format!(
        "{}/to_delete_{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::write(&path, b"")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o333))?;
    let mode = std::fs::metadata(&path)?.permissions().mode();
    let result = if format!("{mode:o}").ends_with("0333") {
        let error = guesser
            .guess_mime_type(path.as_bytes())
            .expect_err("InvalidArgumentException");
        assert!(error.is_a("InvalidArgumentException"), "{error}");
        Ok(())
    } else {
        eprintln!("skipped: Cannot verify chmod operations, change of file permissions failed");
        Ok(())
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666));
    let _ = std::fs::remove_file(&path);
    result
}

#[cfg(not(unix))]
fn non_readable(_guesser: &dyn MimeTypeGuesser) -> TestResult {
    Ok(())
}
