//! XliffUtilsTest.php at symfony/translation v8.1.5. Names and assertions
//! are PHPUnit's; provider rows are upstream's, written as literals.
//! `getFileUrl()` is public here (upstream reaches it by reflection).
//!
//! Upstream: symfony/translation Tests/Util/XliffUtilsTest.php
#![allow(non_snake_case)]

use php_dom::{LoadOptions, load_xml};
use symfony_translation::XliffUtils;

/// `providePaths()`
const PATHS: &[(&str, &str, &str)] = &[
    (
        "plain POSIX path",
        "/tmp/symfony123",
        "file:////tmp/symfony123",
    ),
    (
        "POSIX path with spaces",
        "/tmp/dir with space/symfony123",
        "file:////tmp/dir%20with%20space/symfony123",
    ),
    (
        "POSIX path with non-ASCII characters",
        "/tmp/中文/symfony123",
        "file:////tmp/%E4%B8%AD%E6%96%87/symfony123",
    ),
];

#[test]
fn testGetFileUrlEncodesPathSegments() {
    for (name, path, expected) in PATHS {
        assert_eq!(
            String::from_utf8_lossy(&XliffUtils::get_file_url(path.as_bytes())),
            *expected,
            "{name}"
        );
    }
}

#[test]
fn testValidateSchemaDoesNotResolveExternalEntities() {
    // php-dom loads nothing from a file or the network (no entity loader
    // to observe): the document validates with no error and no load.
    let loaded = load_xml(
        concat!(
            r#"<?xml version="1.0"?>"#,
            r#"<!DOCTYPE xliff [<!ENTITY xxe SYSTEM "http://127.0.0.1:1/payload.dtd">]>"#,
            r#"<xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2">"#,
            r#"<file source-language="en" datatype="plaintext" original="file.ext"><body/></file>"#,
            r#"</xliff>"#
        )
        .as_bytes(),
        LoadOptions::default(),
    )
    .expect("loads");
    let dom = loaded.document.expect("a document");

    let result = XliffUtils::validate_schema(&dom);
    assert!(
        matches!(result, Ok(Ok(ref errors)) if errors.is_empty()),
        "{result:?}"
    );
}
