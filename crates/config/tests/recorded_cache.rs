//! Replays tests/data/cache.json (`oracle/recorders/config_cache.php`,
//! `scripts/record.sh config-cache`): what PHP's `ResourceCheckerConfigCache`,
//! `ConfigCache` and the Filesystem calls they make wrote and answered.
//!
//! Each test works in a directory with a path as long as the recorder's
//! (`/tmp/cc-00000000`), so the lengths in serialized strings stay valid
//! after the path is replaced. Temporary names are masked as the recorder
//! masks them (`<prefix>@TMP@`). File modes were recorded with umask 022;
//! a mode PHP derived from the umask is compared with this process's.
//!
//! Upstream: none
#![cfg(unix)]
#![allow(
    clippy::items_after_statements,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod common;

use std::{
    collections::BTreeMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, UNIX_EPOCH},
};

use common::cache::{LongNameResource, ResourceStub, classes};
use php_value::{PhpArray, Value, base64, vectors};
use serde_json::Value as Json;
use symfony_config::{
    ConfigCache, ConfigCacheInterface, Error, ResourceCheckerConfigCache, ResourceCheckerInterface,
    ResourceClasses,
    resource::{
        DirectoryResource, FileExistenceResource, FileResource, GlobResource, ResourceInterface,
        SelfCheckingResourceChecker, SkippingResourceChecker, file_resource,
    },
};
use symfony_filesystem::{Filesystem, php::umask};

const T: i64 = 1_000_000_000;
const RECORDED_BASE: &[u8] = b"/tmp/cc-00000000";

/// The checker's shared answers are process-wide.
static SHARED: Mutex<()> = Mutex::new(());

fn data() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/cache.json");
    let text = std::fs::read_to_string(path).expect("tests/data/cache.json");
    serde_json::from_str(&text).expect("valid JSON")
}

fn b64(json: &Json) -> Vec<u8> {
    base64::decode(json.as_str().unwrap_or_default().as_bytes())
}

fn os(path: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(path))
}

fn replace(subject: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < subject.len() {
        if !from.is_empty() && subject[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// The recorder's mask: a name's 19 random characters become `@TMP@`.
fn masked(s: &[u8]) -> Vec<u8> {
    let lower = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if i + 19 <= s.len()
            && s[i..i + 13].iter().all(|&c| lower(c))
            && s[i + 13..i + 19].iter().all(u8::is_ascii_alphanumeric)
            && (i + 19 == s.len() || matches!(s[i + 19], b'"' | b',' | b')' | b'/'))
        {
            out.extend_from_slice(b"@TMP@");
            i += 19;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// A directory as long as the recorder's, its files as `reset_base()`
/// leaves them.
struct Base {
    path: Vec<u8>,
}

impl Base {
    fn new(tag: u8) -> Self {
        let pid = std::process::id() % 0x0100_0000;
        let path = format!("/tmp/cc-{tag:02x}{pid:06x}").into_bytes();
        assert_eq!(path.len(), RECORDED_BASE.len());
        Self { path }
    }

    fn join(&self, rest: &str) -> Vec<u8> {
        let mut p = self.path.clone();
        p.push(b'/');
        p.extend_from_slice(rest.as_bytes());
        p
    }

    fn here(&self, recorded: &[u8]) -> Vec<u8> {
        replace(recorded, RECORDED_BASE, &self.path)
    }

    fn reset(&self) {
        let root = os(&self.path);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).expect("mkdir sub");
        std::fs::create_dir(root.join("empty")).expect("mkdir empty");
        for (f, c) in [("a.txt", "a"), ("b.txt", "b"), ("sub/c.txt", "c")] {
            std::fs::write(root.join(f), c).expect("write a file");
            set_mtime(&root.join(f), T);
        }
        for d in ["sub", "empty", ""] {
            set_mtime(&root.join(d), T);
        }
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(os(&self.path));
    }
}

fn set_mtime(path: &Path, t: i64) {
    let when = UNIX_EPOCH + Duration::from_secs(t.unsigned_abs());
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.set_modified(when);
    }
}

/// `touch($path, $t)`.
fn touch(path: &[u8], t: i64) {
    let p = os(path);
    if !p.exists() {
        std::fs::write(&p, "").expect("create a file");
    }
    set_mtime(&p, t);
}

/// The recorder's Probe resource: `__serialize()` returns its array.
#[derive(Debug)]
struct ProbeResource(PhpArray);

impl ResourceInterface for ProbeResource {
    fn class(&self) -> &'static str {
        "ProbeResource"
    }

    fn to_string(&self) -> Vec<u8> {
        b"probe".to_vec()
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        Ok(self.0.clone())
    }
}

/// The recorder's `TestChecker`.
struct TestChecker {
    class: Option<&'static str>,
    fresh: bool,
}

impl ResourceCheckerInterface for TestChecker {
    fn class(&self) -> &'static str {
        "TestChecker"
    }

    fn supports(&self, metadata: &dyn ResourceInterface) -> bool {
        self.class.is_none_or(|c| metadata.class() == c)
    }

    fn is_fresh(&self, _resource: &dyn ResourceInterface, _timestamp: i64) -> Result<bool, Error> {
        Ok(self.fresh)
    }
}

fn all_classes() -> ResourceClasses {
    let mut classes = classes();
    classes.register(
        "ProbeResource",
        Arc::new(|data: &PhpArray| Ok(Arc::new(ProbeResource(data.clone())) as _)),
    );
    classes
}

fn arg(spec: &Json, i: usize) -> Vec<u8> {
    b64(&spec[i])
}

fn resource(base: &Base, spec: &Json) -> Result<Arc<dyn ResourceInterface>, Error> {
    let kind = spec[0].as_str().unwrap_or_default();
    let path = |i: usize| {
        let mut p = base.path.clone();
        p.push(b'/');
        p.extend_from_slice(&arg(spec, i));
        p
    };
    Ok(match kind {
        "file" => Arc::new(FileResource::new(&path(1))?),
        "existence" => Arc::new(FileExistenceResource::new(&path(1))),
        "directory" => {
            let pattern = (!spec[2].is_null()).then(|| arg(spec, 2));
            Arc::new(DirectoryResource::new(&path(1), pattern.as_deref())?)
        }
        "glob" => Arc::new(GlobResource::new(
            &path(1),
            &arg(spec, 2),
            spec[3].as_bool().unwrap_or_default(),
            spec[4].as_bool().unwrap_or_default(),
            PhpArray::new(),
        )?),
        "stub" => {
            let stub = ResourceStub::new();
            stub.set_fresh(spec[1].as_bool().unwrap_or_default());
            Arc::new(stub)
        }
        "long" => Arc::new(LongNameResource::new(&path(1))),
        "probe" => match vectors::decode(&spec[1]) {
            Ok(Value::Array(a)) => Arc::new(ProbeResource(a)),
            other => panic!("probe data {other:?}"),
        },
        other => panic!("unknown resource {other}"),
    })
}

/// What `file_state()` records, with this directory in place of the
/// recorder's and modes PHP derived from umask 022 made this process's.
fn state(base: &Base, path: &[u8]) -> Json {
    let p = os(path);
    let Ok(meta) = std::fs::symlink_metadata(&p) else {
        return Json::Null;
    };
    let stat = std::fs::metadata(&p).ok();
    let content = stat
        .as_ref()
        .filter(|m| m.is_file())
        .and_then(|_| std::fs::read(&p).ok())
        .map(|c| replace(&c, &base.path, RECORDED_BASE));
    let mode = stat.map_or(meta.permissions().mode(), |m| m.permissions().mode()) & 0o7777;
    let link = meta
        .file_type()
        .is_symlink()
        .then(|| std::fs::read_link(&p).ok())
        .flatten()
        .map(|l| {
            use std::os::unix::ffi::OsStrExt;
            let l = replace(l.as_os_str().as_bytes(), &base.path, RECORDED_BASE);
            String::from_utf8_lossy(&replace(&l, &base.path[5..], &RECORDED_BASE[5..])).into_owned()
        });
    serde_json::json!({
        "content": content.map(|c| base64::encode(&c)),
        "mode": format!("{mode:o}"),
        "link": link,
    })
}

/// The recorded state with umask-derived modes turned into this
/// process's: 644 (666 & ~022) and 755 (777 & ~022).
fn expected_state(recorded: &Json) -> Json {
    let mut recorded = recorded.clone();
    if let Some(mode) = recorded.get("mode").and_then(Json::as_str) {
        let here = match mode {
            "644" => format!("{:o}", 0o666 & !umask()),
            "755" => format!("{:o}", 0o777 & !umask()),
            other => other.to_owned(),
        };
        recorded["mode"] = Json::String(here);
    }
    recorded
}

fn throwable(base: &Base, e: &Error) -> Json {
    let message = replace(e.get_message(), &base.path, RECORDED_BASE);
    serde_json::json!([
        "throws",
        e.class().name(),
        base64::encode(&masked(&message))
    ])
}

/// `GlobResource`'s hash covers the absolute paths it finds, which differ
/// here: its 32 hex digits are compared as `@HASH@`.
fn unhashed(state: &Json) -> Json {
    let mut state = state.clone();
    if let Some(content) = state.get("content").and_then(Json::as_str) {
        let bytes = base64::decode(content.as_bytes());
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let hex = bytes.get(i..i + 32).is_some_and(|h| {
                h.iter()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
            });
            let after_quote = i > 0 && bytes[i - 1] == b'"';
            if hex && after_quote && bytes.get(i + 32) == Some(&b'"') {
                out.extend_from_slice(b"@HASH@");
                i += 32;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        state["content"] = Json::String(base64::encode(&out));
    }
    state
}

fn report(failures: &[String], total: usize, what: &str) {
    assert!(
        failures.is_empty(),
        "{} of {total} {what} differ from PHP:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The written meta files PHP and this port cannot agree on, listed in
/// README.md: PHP writes `r:` for an object met twice, php-value's
/// `serialize()` writes it again.
fn listed_write(case: &Json) -> bool {
    case["metadata"][0][1].to_string().contains("\"Hidden\"")
}

#[test]
fn write_files_as_php() {
    let _shared = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = Base::new(1);
    let data = data();
    let cases = data["write"].as_array().expect("write cases");
    let mut failures = Vec::new();
    for (n, case) in cases.iter().enumerate() {
        base.reset();
        let content = b64(&case["content"]);
        let metadata = case["metadata"].as_array().map(|specs| {
            specs
                .iter()
                .map(|s| resource(&base, s).expect("a resource"))
                .collect::<Vec<_>>()
        });
        let mut cache = ResourceCheckerConfigCache::new(&base.join("cache.php"), [], None)
            .with_classes(all_classes());
        let result = match cache.write(&content, metadata.as_deref()) {
            Ok(()) => Json::Null,
            Err(e) => throwable(&base, &e),
        };
        let actual = serde_json::json!({
            "result": result,
            "cache": state(&base, &base.join("cache.php")),
            "meta": unhashed(&state(&base, &base.join("cache.php.meta"))),
            "json": unhashed(&state(&base, &base.join("cache.php.meta.json"))),
        });
        let expected = serde_json::json!({
            "result": case["result"],
            "cache": expected_state(&case["cache"]),
            "meta": unhashed(&expected_state(&case["meta"])),
            "json": unhashed(&expected_state(&case["json"])),
        });
        if actual != expected && !listed_write(case) {
            failures.push(format!(
                "write #{n} {}:\n  PHP  {expected}\n  port {actual}",
                case["metadata"]
            ));
        }
    }
    report(&failures, cases.len(), "writes");
}

fn make(set: &str, file: &[u8]) -> Box<dyn ConfigCacheInterface> {
    let checkers: Vec<Arc<dyn ResourceCheckerInterface>> = match set {
        "none" => vec![],
        "self" => vec![Arc::new(SelfCheckingResourceChecker::new())],
        "skip-all" => vec![Arc::new(SkippingResourceChecker::new::<&str>(&[]))],
        "skip-file" => vec![
            Arc::new(SkippingResourceChecker::new(&[file_resource::CLASS])),
            Arc::new(SelfCheckingResourceChecker::new()),
        ],
        "test-other-then-self" => vec![
            Arc::new(TestChecker {
                class: Some("Nope"),
                fresh: false,
            }),
            Arc::new(SelfCheckingResourceChecker::new()),
        ],
        "test-stale" => vec![
            Arc::new(TestChecker {
                class: None,
                fresh: false,
            }),
            Arc::new(SelfCheckingResourceChecker::new()),
        ],
        "test-fresh" => vec![
            Arc::new(TestChecker {
                class: None,
                fresh: true,
            }),
            Arc::new(SelfCheckingResourceChecker::new()),
        ],
        "config-debug" => {
            return Box::new(ConfigCache::new(file, true, None, None).with_classes(all_classes()));
        }
        "config-prod" => {
            return Box::new(ConfigCache::new(file, false, None, None).with_classes(all_classes()));
        }
        "config-debug-skip-file" => {
            return Box::new(
                ConfigCache::new(file, true, None, Some(&[file_resource::CLASS]))
                    .with_classes(all_classes()),
            );
        }
        other => panic!("unknown checker set {other}"),
    };
    Box::new(ResourceCheckerConfigCache::new(file, checkers, None).with_classes(all_classes()))
}

fn change(base: &Base, name: &str) {
    let p = |f: &str| base.join(f);
    match name {
        "none" => {}
        "cache older" => touch(&p("cache.php"), T - 10),
        "cache same time" => touch(&p("cache.php"), T),
        "touch a.txt" => touch(&p("a.txt"), T + 100),
        "remove a.txt" => std::fs::remove_file(os(&p("a.txt"))).expect("unlink"),
        "create missing" => touch(&p("missing"), T),
        "add sub/d.txt" => {
            std::fs::write(os(&p("sub/d.txt")), "d").expect("write");
            touch(&p("sub/d.txt"), T);
            touch(&p("sub"), T);
        }
        "remove cache" => std::fs::remove_file(os(&p("cache.php"))).expect("unlink"),
        "remove meta" => std::fs::remove_file(os(&p("cache.php.meta"))).expect("unlink"),
        "meta is a directory" => {
            std::fs::remove_file(os(&p("cache.php.meta"))).expect("unlink");
            std::fs::create_dir(os(&p("cache.php.meta"))).expect("mkdir");
        }
        other => panic!("unknown change {other}"),
    }
}

/// The `isFresh()` answers listed in README.md: `GlobResource`'s restore
/// (symfony-config's resource code) refuses any object as a Stringable,
/// where PHP refuses only a `\Stringable` and then fails the typed
/// property (a `TypeError`).
fn listed_fresh(case: &Json) -> bool {
    String::from_utf8_lossy(&b64(&case["meta"]))
        .contains("GlobResource\":1:{s:6:\"prefix\";O:8:\"stdClass\"")
}

#[test]
fn is_fresh_as_php() {
    let _shared = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = Base::new(2);
    let data = data();
    let cases = data["fresh"].as_array().expect("fresh cases");
    let mut failures = Vec::new();
    for (n, case) in cases.iter().enumerate() {
        base.reset();
        let set = case["checkers"].as_str().unwrap_or_default();
        let file = base.join("cache.php");
        let resources: Vec<_> = case["resources"]
            .as_array()
            .expect("resources")
            .iter()
            .map(|s| resource(&base, s).expect("a resource"))
            .collect();
        SelfCheckingResourceChecker::clear_cache();
        make(set, &file)
            .write(b"content", Some(&resources))
            .expect("write");
        touch(&file, T);
        for f in ["a.txt", "b.txt", "sub/c.txt", "sub"] {
            touch(&base.join(f), T);
        }
        if !case["meta"].is_null() {
            let len = base.join("a.txt").len().to_string();
            let lenm = base.join("missing").len().to_string();
            let meta = b64(&case["meta"]);
            let meta = replace(&meta, b"@LEN@", len.as_bytes());
            let meta = replace(&meta, b"@LENM@", lenm.as_bytes());
            let meta = replace(&meta, b"@BASE@", &base.path);
            std::fs::write(os(&base.join("cache.php.meta")), meta).expect("write the meta file");
        }
        change(&base, case["change"].as_str().unwrap_or_default());
        let actual = match make(set, &file).is_fresh() {
            Ok(b) => serde_json::json!(["bool", b]),
            Err(e) => throwable(&base, &e),
        };
        if actual != case["result"] && !listed_fresh(case) {
            let meta = String::from_utf8_lossy(&b64(&case["meta"])).into_owned();
            failures.push(format!(
                "fresh #{n} {set} {} {:?} meta {meta:?}: PHP {}, port {actual}",
                case["resources"], case["change"], case["result"]
            ));
        }
    }
    report(&failures, cases.len(), "isFresh() calls");
}

/// Every file under the directory, as the recorder's `listing()`.
fn listing(base: &Base) -> Json {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<Vec<u8>, PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(&path);
            use std::os::unix::ffi::OsStrExt;
            out.insert(masked(rel.as_os_str().as_bytes()), path.clone());
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                walk(&path, root, out);
            }
        }
    }
    let root = os(&base.path);
    let mut all = BTreeMap::new();
    walk(&root, &root, &mut all);
    let mut map = serde_json::Map::new();
    for (name, path) in all {
        use std::os::unix::ffi::OsStrExt;
        map.insert(
            String::from_utf8_lossy(&name).into_owned(),
            state(base, path.as_os_str().as_bytes()),
        );
    }
    Json::Object(map)
}

fn expected_listing(recorded: &Json) -> Json {
    match recorded {
        Json::Object(map) => Json::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), expected_state(v)))
                .collect(),
        ),
        // PHP's json_encode() of an empty array
        _ => Json::Object(serde_json::Map::new()),
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
fn filesystem_as_php() {
    let base = Base::new(3);
    let data = data();
    let cases = data["filesystem"].as_array().expect("filesystem cases");
    let filesystem = Filesystem::new();
    let mut failures = Vec::new();
    for (n, case) in cases.iter().enumerate() {
        base.reset();
        std::env::set_current_dir(os(&base.path)).expect("chdir");
        let call = &case["call"];
        let op = call[0].as_str().unwrap_or_default();
        let raw = base.here(
            call[1]
                .as_str()
                .unwrap_or_default()
                .replace("@BASE@", "/tmp/cc-00000000")
                .as_bytes(),
        );
        let kind = call[2].as_str();
        let mut path = if raw.windows(3).any(|w| w == b"://") || kind == Some("relative") {
            raw.clone()
        } else {
            let mut p = base.path.clone();
            p.push(b'/');
            p.extend_from_slice(&raw);
            p
        };
        let own_name = base.path[5..].to_vec();
        let target = |i: usize| {
            let t = call[i]
                .as_str()
                .unwrap_or_default()
                .replace("@BASE@", "/tmp/cc-00000000");
            replace(&base.here(t.as_bytes()), &RECORDED_BASE[5..], &own_name)
        };
        match kind {
            Some("link" | "link-in-sub") => {
                std::os::unix::fs::symlink(os(&target(3)), os(&path)).expect("symlink");
            }
            Some("relative-link") => {
                let mut abs = base.path.clone();
                abs.push(b'/');
                abs.extend_from_slice(&raw);
                std::os::unix::fs::symlink(os(&target(3)), os(&abs)).expect("symlink");
                path = raw.clone();
            }
            Some("chain") => {
                std::os::unix::fs::symlink(os(&base.join("a.txt")), os(&base.join("link-a")))
                    .expect("symlink");
                std::os::unix::fs::symlink("link-a", os(&path)).expect("symlink");
            }
            Some("mode") => {
                std::fs::write(os(&path), "old").expect("write");
                let mode = u32::from_str_radix(call[3].as_str().unwrap_or("644"), 8).expect("mode");
                std::fs::set_permissions(os(&path), std::fs::Permissions::from_mode(mode))
                    .expect("chmod");
            }
            _ => {}
        }
        let to_json = |r: Result<Json, symfony_filesystem::Error>| match r {
            Ok(v) => serde_json::json!(["ok", v]),
            Err(e) => {
                let message = replace(e.message(), &base.path, RECORDED_BASE);
                serde_json::json!([
                    "throws",
                    e.class().name(),
                    base64::encode(&masked(&message))
                ])
            }
        };
        let result = to_json(match op {
            "dumpFile" => filesystem
                .dump_file(&path, b"new content")
                .map(|()| Json::Null),
            "readFile" => filesystem
                .read_file(&path)
                .map(|c| Json::String(base64::encode(&c))),
            "chmod" => filesystem
                .chmod(
                    &[&path],
                    u32::try_from(call[2].as_u64().unwrap_or_default()).unwrap_or_default(),
                    u32::try_from(call[3].as_u64().unwrap_or_default()).unwrap_or_default(),
                )
                .map(|()| Json::Null),
            "tempnam" => {
                let prefix = call[2].as_str().unwrap_or_default();
                filesystem.tempnam(&raw, prefix.as_bytes()).map(|name| {
                    let shown = replace(&name, &base.path, RECORDED_BASE);
                    if !name.starts_with(&base.path) {
                        // the system's temporary directory
                        let _ = std::fs::remove_file(os(&name));
                    }
                    Json::String(String::from_utf8_lossy(&masked(&shown)).into_owned())
                })
            }
            other => panic!("unknown call {other}"),
        });
        std::env::set_current_dir("/").expect("chdir /");
        let actual = serde_json::json!({"result": result, "files": listing(&base)});
        let expected = serde_json::json!({"result": case["result"], "files": expected_listing(&case["files"])});
        if actual != expected {
            failures.push(format!(
                "filesystem #{n} {call}:\n  PHP  {expected}\n  port {actual}"
            ));
        }
    }
    report(&failures, cases.len(), "filesystem calls");
}
