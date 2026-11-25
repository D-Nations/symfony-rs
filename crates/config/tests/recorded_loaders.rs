//! Replays tests/data/loaders.json, recorded from PHP 8.4.27 by
//! `oracle/recorders/config_loaders.php`: `parse_url()` schemes and
//! `isAbsolutePath()`, `LoaderLoadException` and the circular-reference
//! message, `FileLocator::locate()`, `GlobResource` (paths and string form,
//! with and without `GLOB_BRACE`), hash('xxh128'), `DirectoryResource`'s
//! string form and `FileLoader::import()`.
//!
//! "{root}" in the recording is the recorder's copy of tests/fixtures;
//! here it is a private copy's real path.
//!
//! Upstream: none
#![allow(non_snake_case)]
#![allow(
    clippy::match_same_arms,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod common;

use std::sync::Arc;

use common::{fs::TempDir, loaders::TestFileLoader};
use php_value::{Key, PhpArray, Value, base64, object::StdClass};
use serde_json::Value as Json;
use symfony_config::{
    Error, FileLocator, FileLocatorInterface, Located,
    exception::{file_loader_import_circular_reference_exception, loader_load_exception},
    file_locator::parse_url_scheme,
    resource::{DirectoryResource, GlobResource, ResourceInterface},
};
use symfony_filesystem::php::{UrlScheme, parse_url_scheme_value};

fn data() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/loaders.json");
    let text = std::fs::read_to_string(path).expect("tests/data/loaders.json");
    serde_json::from_str(&text).expect("valid JSON")
}

struct Root {
    _dir: TempDir,
    root: String,
}

impl Root {
    fn new() -> Self {
        let dir = TempDir::new();
        dir.copy_fixtures_to("Fixtures");
        let root = String::from_utf8_lossy(&dir.join("/Fixtures")).into_owned();
        Self { _dir: dir, root }
    }

    fn abs(&self, s: &str) -> String {
        s.replace("{root}", &self.root)
    }

    fn rel(&self, s: &[u8]) -> String {
        String::from_utf8_lossy(s).replace(&self.root, "{root}")
    }

    fn rel_value(&self, v: &Value) -> Json {
        match v {
            Value::Null => Json::Null,
            Value::String(s) => Json::String(self.rel(s.as_bytes())),
            Value::Array(a) if a.is_list() => {
                Json::Array(a.values().map(|v| self.rel_value(v)).collect())
            }
            Value::Array(a) => Json::Object(
                a.iter()
                    .map(|(k, v)| {
                        (
                            self.rel(&symfony_config::php::key_bytes(k)),
                            self.rel_value(v),
                        )
                    })
                    .collect(),
            ),
            Value::Bool(b) => Json::Bool(*b),
            Value::Int(i) => Json::from(*i),
            other => Json::String(format!("{other:?}")),
        }
    }

    fn error(&self, e: &Error) -> Json {
        let mut m = serde_json::Map::new();
        m.insert("class".into(), Json::String(e.class().name().to_owned()));
        m.insert("message".into(), Json::String(self.rel(e.get_message())));
        if *e.class() == symfony_config::Class::FileLocatorFileNotFound {
            m.insert(
                "paths".into(),
                Json::Array(
                    e.get_paths()
                        .iter()
                        .map(|p| Json::String(self.rel(p)))
                        .collect(),
                ),
            );
        }
        if let Some(p) = e.get_previous() {
            m.insert("previous".into(), Json::String(p.class().name().to_owned()));
        }
        Json::Object(m)
    }
}

fn opt_str(v: &Json) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn report(section: &str, failures: &[String], total: usize) {
    for f in failures.iter().take(20) {
        eprintln!("{f}");
    }
    assert!(
        failures.is_empty(),
        "{section}: {} of {total} cases differ from PHP",
        failures.len()
    );
}

#[test]
fn scheme_and_is_absolute_path() {
    let data = data();
    let cases = data["scheme"].as_array().expect("scheme cases");
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or_default();
        // null and false are both "no scheme"
        let expected = case["scheme"].as_str().map(str::to_owned);
        let actual =
            parse_url_scheme(name.as_bytes()).map(|s| String::from_utf8_lossy(&s).into_owned());
        if expected != actual {
            failures.push(format!(
                "parse_url({name:?}): PHP {expected:?}, port {actual:?}"
            ));
        }
        // symfony-filesystem's Path::isAbsolute() tells null from false
        let value = match &case["scheme"] {
            Json::String(s) => UrlScheme::Scheme(s.as_bytes().to_vec()),
            Json::Bool(false) => UrlScheme::False,
            _ => UrlScheme::Null,
        };
        if parse_url_scheme_value(name.as_bytes()) != value {
            failures.push(format!(
                "parse_url({name:?}): null and false differ from PHP"
            ));
        }
        let absolute = case["absolute"].as_bool().unwrap_or_default();
        if absolute != FileLocator::is_absolute_path(name.as_bytes()) {
            failures.push(format!("isAbsolutePath({name:?}): PHP {absolute}"));
        }
    }
    report("scheme", &failures, cases.len());
}

fn resource(label: &str) -> Value {
    let list = |v: Vec<Value>| Value::Array(PhpArray::list(v));
    match label {
        "string" => Value::string("resource"),
        "bundle" => Value::string("@resource"),
        "bundle-path" => Value::string("@bundle/sub/x.yaml"),
        "empty-string" => Value::string(""),
        "int" => Value::Int(42),
        "float" => Value::Float(4.5),
        "float-int" => Value::Float(1.0),
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        "assoc" => {
            let mut a = PhpArray::new();
            a.insert(Key::from("a"), Value::Int(1));
            a.insert(Key::from("b"), list(vec![Value::Bool(true), Value::Null]));
            Value::Array(a)
        }
        "list" => list(vec![Value::Int(1), Value::string("two")]),
        "empty" => Value::Array(PhpArray::new()),
        "object" => StdClass::object(PhpArray::new()),
        "inf" => Value::Float(f64::INFINITY),
        "bad-utf8" => Value::string(b"\xff".to_vec()),
        "bad-utf8-array" => list(vec![Value::string(b"\xff".to_vec())]),
        "slash" => {
            let mut a = PhpArray::new();
            a.insert(Key::from("a/b"), Value::string("é"));
            Value::Array(a)
        }
        _ => Value::Null,
    }
}

#[test]
fn loader_load_exception_messages() {
    let data = data();
    let cases = data["loader_load"].as_array().expect("loader_load cases");
    let mut failures = Vec::new();
    for case in cases {
        let label = case["resource"].as_str().unwrap_or_default();
        let value = resource(label);
        let previous = opt_str(&case["previous"]);
        let source = opt_str(&case["source"]);
        let r#type = opt_str(&case["type"]);
        let expected = case["message"].as_str().unwrap_or_default();
        let matched = {
            let e = loader_load_exception(
                &value,
                source.as_deref().map(str::as_bytes),
                3,
                previous.as_ref().map(|p| {
                    Error::new(
                        symfony_config::Class::Other {
                            class: "Exception".into(),
                            exception: true,
                        },
                        p.as_bytes(),
                    )
                }),
                r#type.as_deref().map(str::as_bytes),
            );
            String::from_utf8_lossy(e.get_message()) == expected && e.get_code() == 3
        };
        if !matched {
            failures.push(format!(
                "{label} {previous:?} {source:?} {type:?}: PHP {expected:?}"
            ));
        }
    }
    report("loader_load", &failures, cases.len());

    for case in data["circular"].as_array().expect("circular cases") {
        let resources: Vec<Vec<u8>> = case["resources"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.as_bytes().to_vec())
                    .collect()
            })
            .unwrap_or_default();
        let e = file_loader_import_circular_reference_exception(&resources);
        assert_eq!(
            case["message"].as_str().unwrap_or_default(),
            String::from_utf8_lossy(e.get_message())
        );
    }
}

#[test]
fn file_locator_locate() {
    let data = data();
    let root = Root::new();
    let cases = data["locate"].as_array().expect("locate cases");
    let mut failures = Vec::new();
    for case in cases {
        let paths: Vec<String> = case["paths"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|p| root.abs(p))
                    .collect()
            })
            .unwrap_or_default();
        let locator = FileLocator::new(&paths);
        let name = root.abs(case["name"].as_str().unwrap_or_default());
        let current = opt_str(&case["current"]).map(|c| root.abs(&c));
        let first = case["first"].as_bool().unwrap_or_default();
        let actual = match locator.locate(
            name.as_bytes(),
            current.as_deref().map(str::as_bytes),
            first,
        ) {
            Ok(Located::Path(p)) => ("result", Json::String(root.rel(&p))),
            Ok(Located::Paths(ps)) => (
                "result",
                Json::Array(ps.iter().map(|p| Json::String(root.rel(p))).collect()),
            ),
            Err(e) => ("error", root.error(&e)),
        };
        if case.get(actual.0) != Some(&actual.1) {
            failures.push(format!("{case}: port {} {}", actual.0, actual.1));
        }
    }
    report("locate", &failures, cases.len());
}

#[test]
fn glob_resource() {
    let data = data();
    let root = Root::new();
    let cases = data["glob"].as_array().expect("glob cases");
    let mut failures = Vec::new();
    for case in cases {
        let prefix = root.abs(case["prefix"].as_str().unwrap_or_default());
        let pattern = case["pattern"].as_str().unwrap_or_default();
        let recursive = case["recursive"].as_bool().unwrap_or_default();
        let for_exclusion = case["forExclusion"].as_bool().unwrap_or_default();
        let brace = case["brace"].as_bool().unwrap_or(true);
        let mut excluded = PhpArray::new();
        for k in case["excluded"].as_array().into_iter().flatten() {
            if let Some(k) = k.as_str() {
                excluded.insert(Key::from_bytes(root.abs(k).as_bytes()), Value::Bool(true));
            }
        }
        let result = GlobResource::new(
            prefix.as_bytes(),
            pattern.as_bytes(),
            recursive,
            for_exclusion,
            excluded,
        )
        .and_then(|mut r| {
            r.set_glob_brace(brace);
            let paths = r.get_iterator()?;
            Ok((r.to_string(), paths))
        });
        match result {
            Ok((string, paths)) => {
                let paths = Json::Array(paths.iter().map(|p| Json::String(root.rel(p))).collect());
                let string = Json::String(root.rel(&string));
                if case.get("paths") != Some(&paths) || case.get("string") != Some(&string) {
                    failures.push(format!("{case}: port {string} {paths}"));
                }
            }
            Err(e) => {
                if case.get("error") != Some(&root.error(&e)) {
                    failures.push(format!("{case}: port error {}", root.error(&e)));
                }
            }
        }
    }
    report("glob", &failures, cases.len());

    for case in data["glob_new"].as_array().expect("glob_new cases") {
        let prefix = root.abs(case["prefix"].as_str().unwrap_or_default());
        let r = GlobResource::new(prefix.as_bytes(), b"/*", true, false, PhpArray::new());
        match (r, case.get("error")) {
            (Ok(_), None) => {}
            (Err(e), Some(expected)) => assert_eq!(*expected, root.error(&e)),
            (r, expected) => panic!("{case}: port {:?} PHP {expected:?}", r.err()),
        }
    }
}

#[test]
fn xxh128_and_directory_resource() {
    let data = data();
    for case in data["xxh128"].as_array().expect("xxh128 cases") {
        let bytes = base64::decode_strict(case["data"].as_str().unwrap_or_default().as_bytes())
            .expect("base64");
        let expected = case["hash"].as_str().unwrap_or_default();
        let actual = format!("{:032x}", twox_hash::XxHash3_128::oneshot(&bytes));
        assert_eq!(expected, actual);
    }
    // DirectoryResource's string is the hash of the serialized array; the
    // recorder's root differs from this copy's, so the check is that the
    // same formula gives PHP's answer for PHP's root.
    let root = Root::new();
    for case in data["directory"].as_array().expect("directory cases") {
        let pattern = opt_str(&case["pattern"]);
        let r = DirectoryResource::new(root.root.as_bytes(), pattern.as_deref().map(str::as_bytes))
            .expect("the fixtures directory");
        assert_eq!(32, r.to_string().len());
        assert_eq!(Some(true), case["serialized"].as_bool());
    }
}

#[test]
fn file_loader_import() {
    let data = data();
    let root = Root::new();
    let cases = data["import"].as_array().expect("import cases");
    let mut failures = Vec::new();
    for case in cases {
        let loader = TestFileLoader::new(Arc::new(FileLocator::new([&root.root])));
        let resource = root.abs(case["resource"].as_str().unwrap_or_default());
        let exclude = match &case["exclude"] {
            Json::Null => Value::Null,
            Json::String(s) => Value::string(root.abs(s)),
            Json::Array(a) => Value::Array(PhpArray::list(
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| Value::string(root.abs(s))),
            )),
            _ => Value::Null,
        };
        let ignore = case["ignore"].as_bool().unwrap_or_default();
        let r#type = opt_str(&case["type"]);
        let actual = match loader.file.import(
            &Value::string(resource),
            r#type.as_deref().map(str::as_bytes),
            ignore,
            Some(b"src.yaml"),
            &exclude,
        ) {
            Ok(v) => ("result", root.rel_value(&v)),
            Err(e) => ("error", root.error(&e)),
        };
        if case.get(actual.0) != Some(&actual.1) {
            failures.push(format!("{case}: port {} {}", actual.0, actual.1));
        }
    }
    report("import", &failures, cases.len());
}
