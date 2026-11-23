//! PathTest.php at symfony/filesystem v8.1.9: the tests of the ported
//! methods, with the rows upstream gives when `DIRECTORY_SEPARATOR` is
//! `/`. setUp()'s `putenv('HOME=/home/webmozart')` is an [`Env`] passed to
//! the `_in` variants (Rust cannot change its environment safely).
//!
//! Upstream: symfony/filesystem Tests/PathTest.php
#![allow(non_snake_case)]

use symfony_filesystem::{Class, Env, Path};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn env() -> Env {
    Env {
        home: Some(b"/home/webmozart".to_vec()),
        homedrive: Some(Vec::new()),
        homepath: Some(Vec::new()),
    }
}

const CANONICALIZATION: &[(&str, &str)] = &[
    ("css/./style.css", "css/style.css"),
    ("css/../style.css", "style.css"),
    ("css/./../style.css", "style.css"),
    ("css/.././style.css", "style.css"),
    ("css/../../style.css", "../style.css"),
    ("./css/style.css", "css/style.css"),
    ("../css/style.css", "../css/style.css"),
    ("./../css/style.css", "../css/style.css"),
    (".././css/style.css", "../css/style.css"),
    ("../../css/style.css", "../../css/style.css"),
    ("", ""),
    (".", ""),
    ("..", ".."),
    ("./..", ".."),
    ("../.", ".."),
    ("../..", "../.."),
    ("/css/style.css", "/css/style.css"),
    ("/css/./style.css", "/css/style.css"),
    ("/css/../style.css", "/style.css"),
    ("/css/./../style.css", "/style.css"),
    ("/css/.././style.css", "/style.css"),
    ("/./css/style.css", "/css/style.css"),
    ("/../css/style.css", "/css/style.css"),
    ("/./../css/style.css", "/css/style.css"),
    ("/.././css/style.css", "/css/style.css"),
    ("/../../css/style.css", "/css/style.css"),
    ("phar:///css/style.css", "phar:///css/style.css"),
    ("phar:///css/./style.css", "phar:///css/style.css"),
    ("phar:///css/../style.css", "phar:///style.css"),
    ("phar:///css/./../style.css", "phar:///style.css"),
    ("phar:///css/.././style.css", "phar:///style.css"),
    ("phar:///./css/style.css", "phar:///css/style.css"),
    ("phar:///../css/style.css", "phar:///css/style.css"),
    ("phar:///./../css/style.css", "phar:///css/style.css"),
    ("phar:///.././css/style.css", "phar:///css/style.css"),
    ("phar:///../../css/style.css", "phar:///css/style.css"),
    ("~/css/style.css", "/home/webmozart/css/style.css"),
    ("~/css/./style.css", "/home/webmozart/css/style.css"),
    ("~/css/../style.css", "/home/webmozart/style.css"),
    ("~/css/./../style.css", "/home/webmozart/style.css"),
    ("~/css/.././style.css", "/home/webmozart/style.css"),
    ("~/./css/style.css", "/home/webmozart/css/style.css"),
    ("~/../css/style.css", "/home/css/style.css"),
    ("~/./../css/style.css", "/home/css/style.css"),
    ("~/.././css/style.css", "/home/css/style.css"),
    ("~/../../css/style.css", "/css/style.css"),
];

#[test]
fn testCanonicalize() -> TestResult {
    for (path, canonicalized) in CANONICALIZATION {
        assert_eq!(
            Path::canonicalize_in(path.as_bytes(), &env())?,
            canonicalized.as_bytes(),
            "{path}"
        );
    }
    Ok(())
}

const IS_ABSOLUTE: &[(&str, bool)] = &[
    ("/css/style.css", true),
    ("/", true),
    ("/var/lib", true),
    ("css/style.css", false),
    ("var/lib", false),
    ("../var/lib", false),
    ("", false),
    ("phar:///css/style.css", true),
    ("phar:///", true),
    ("http://example.com", true),
    ("ftp://user@server/path", true),
    ("vfs://root/file.txt", true),
];

#[test]
fn testIsAbsolute() {
    for (path, absolute) in IS_ABSOLUTE {
        assert_eq!(Path::is_absolute(path.as_bytes()), *absolute, "{path}");
    }
}

#[test]
fn testIsRelative() {
    for (path, absolute) in IS_ABSOLUTE {
        assert_eq!(Path::is_relative(path.as_bytes()), !*absolute, "{path}");
    }
}

#[test]
fn testGetRoot() {
    for (path, root) in [
        ("/css/style.css", "/"),
        ("/", "/"),
        ("css/style.css", ""),
        ("", ""),
        ("phar:///css/style.css", "phar:///"),
        ("phar:///", "phar:///"),
    ] {
        assert_eq!(Path::get_root(path.as_bytes()), root.as_bytes(), "{path}");
    }
}

const MAKE_ABSOLUTE: &[(&str, &str, &str)] = &[
    (
        "css/style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/css/style.css",
    ),
    (
        "../css/style.css",
        "/webmozart/symfony",
        "/webmozart/css/style.css",
    ),
    (
        "../../css/style.css",
        "/webmozart/symfony",
        "/css/style.css",
    ),
    ("css/style.css", "/", "/css/style.css"),
    (
        "../../symfony/css/style.css",
        "/webmozart/css",
        "/symfony/css/style.css",
    ),
    ("", "/webmozart/symfony", "/webmozart/symfony"),
    ("..", "/webmozart/symfony", "/webmozart"),
    (
        "css/./style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/css/style.css",
    ),
    (
        "css/../style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/style.css",
    ),
    (
        "css/./../style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/style.css",
    ),
    (
        "css/.././style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/style.css",
    ),
    (
        "./css/style.css",
        "/webmozart/symfony",
        "/webmozart/symfony/css/style.css",
    ),
    ("./css/style.css", "/", "/css/style.css"),
    ("../css/style.css", "/", "/css/style.css"),
    ("../css/./style.css", "/", "/css/style.css"),
    ("../css/../style.css", "/", "/style.css"),
    ("../css/./../style.css", "/", "/style.css"),
    ("../css/.././style.css", "/", "/style.css"),
    ("./css/style.css", "phar:///", "phar:///css/style.css"),
    ("../css/style.css", "phar:///", "phar:///css/style.css"),
    ("../css/./style.css", "phar:///", "phar:///css/style.css"),
    ("../css/../style.css", "phar:///", "phar:///style.css"),
    ("../css/./../style.css", "phar:///", "phar:///style.css"),
    ("../css/.././style.css", "phar:///", "phar:///style.css"),
    ("/css/style.css", "/webmozart/symfony", "/css/style.css"),
];

#[test]
fn testMakeAbsolute() -> TestResult {
    for (relative, base, absolute) in MAKE_ABSOLUTE {
        assert_eq!(
            Path::make_absolute_in(relative.as_bytes(), base.as_bytes(), &env())?,
            absolute.as_bytes(),
            "{relative} against {base}"
        );
    }
    Ok(())
}

#[test]
fn testMakeAbsoluteFailsIfBasePathNotAbsolute() {
    let e = Path::make_absolute_in(b"css/style.css", b"webmozart/symfony", &env())
        .expect_err("not absolute");
    assert_eq!(e.class(), Class::InvalidArgument);
    assert!(
        String::from_utf8_lossy(e.message())
            .contains("The base path \"webmozart/symfony\" is not an absolute path.")
    );
}

#[test]
fn testMakeAbsoluteFailsIfBasePathEmpty() {
    let e = Path::make_absolute_in(b"css/style.css", b"", &env()).expect_err("empty");
    assert_eq!(e.class(), Class::InvalidArgument);
    assert!(
        String::from_utf8_lossy(e.message())
            .contains("The base path must be a non-empty string. Got: \"\"")
    );
}

#[test]
fn testMakeAbsoluteDoesNotFailIfDifferentRoot() -> TestResult {
    for (base, absolute) in [
        ("phar:///css/style.css", "/webmozart/symfony"),
        ("/css/style.css", "phar:///webmozart/symfony"),
    ] {
        assert_eq!(
            Path::make_absolute_in(absolute.as_bytes(), base.as_bytes(), &env())?,
            Path::canonicalize_in(absolute.as_bytes(), &env())?
        );
    }
    Ok(())
}

#[test]
fn testGetHomeDirectoryFailsIfNotSupportedOperatingSystem() {
    let env = Env {
        home: Some(Vec::new()),
        ..env()
    };
    let e = Path::get_home_directory_in(&env).expect_err("no home");
    assert_eq!(e.class(), Class::Runtime);
    assert!(
        String::from_utf8_lossy(e.message())
            .contains("Your environment or operating system isn't supported")
    );
}

#[test]
fn testGetHomeDirectoryForUnix() -> TestResult {
    assert_eq!(Path::get_home_directory_in(&env())?, b"/home/webmozart");
    Ok(())
}

#[test]
fn testGetHomeDirectoryForWindows() -> TestResult {
    let env = Env {
        home: Some(Vec::new()),
        homedrive: Some(b"C:".to_vec()),
        homepath: Some(b"/users/webmozart".to_vec()),
    };
    assert_eq!(Path::get_home_directory_in(&env)?, b"C:/users/webmozart");
    Ok(())
}

#[test]
fn testNormalize() {
    assert_eq!(Path::normalize(b"C:\\Foo\\Bar/test"), b"C:\\Foo\\Bar/test");
}
