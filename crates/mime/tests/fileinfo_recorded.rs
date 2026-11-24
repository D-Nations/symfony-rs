//! FileinfoMimeTypeGuesser against PHP 8.4's own answers
//! (`tests/data/fileinfo.json`, `oracle/recorders/mime_fileinfo.php`,
//! `scripts/record.sh mime-fileinfo`).
//!
//! The port's libmagic is not PHP's, so some answers differ. Every
//! difference is listed in `tests/data/fileinfo_differences.tsv` (set,
//! path, PHP's answer, the port's answer). A test fails when a file
//! differs that is not listed, or a listed file no longer differs as
//! listed. With `SYMFONY_RS_WRITE_DIFFERENCES=1` the test writes the list
//! again instead.
//!
//! The `upstream` set reads the pinned checkouts under `upstream/symfony`
//! (`scripts/fetch-upstream.sh`); without them that set is skipped.
//!
//! Upstream: none

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;
use symfony_mime::{FileinfoMimeTypeGuesser, MimeTypeGuesser};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/fileinfo.json");
const DIFFERENCES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/fileinfo_differences.tsv"
);

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// A guess as the recorder writes it: the type, `null`, or the class
/// thrown.
fn answer(guesser: &FileinfoMimeTypeGuesser, path: &Path) -> String {
    match guesser.guess_mime_type(path.to_string_lossy().as_bytes()) {
        Ok(Some(t)) => t,
        Ok(None) => "null".to_string(),
        Err(e) => format!("throws {}", e.class().name()),
    }
}

fn recorded(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        other => format!("throws {}", other["throws"].as_str().unwrap_or("?")),
    }
}

/// (set, path) => (PHP, port).
type Differences = BTreeMap<(String, String), (String, String)>;

fn listed() -> Differences {
    let text = std::fs::read_to_string(DIFFERENCES).unwrap_or_default();
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(4, f.len(), "bad line {l:?}");
            ((f[0].into(), f[1].into()), (f[2].into(), f[3].into()))
        })
        .collect()
}

fn write(differences: &Differences, counts: &BTreeMap<String, (usize, usize)>) {
    let mut out = String::from(
        "# FileinfoMimeTypeGuesser: files where the port's answer is not PHP 8.4's.\n\
         # Written by `SYMFONY_RS_WRITE_DIFFERENCES=1 cargo test -p symfony-mime --test fileinfo_recorded`.\n\
         # set\tpath\tPHP finfo\tport\n",
    );
    for (set, (same, all)) in counts {
        out.push_str(&format!("# {set}: {same} of {all} files match\n"));
    }
    for ((set, path), (php, port)) in differences {
        out.push_str(&format!("{set}\t{path}\t{php}\t{port}\n"));
    }
    std::fs::write(DIFFERENCES, out).expect("write the differences");
}

#[test]
fn guesses_as_php_finfo() {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(DATA).expect("recorded answers"))
            .expect("JSON");
    let guesser = FileinfoMimeTypeGuesser::new();
    let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fileinfo");
    let have_upstream = root().join("upstream/symfony").is_dir();
    if !have_upstream {
        eprintln!("skipped: the upstream set (run scripts/fetch-upstream.sh)");
    }
    let mut differences = Differences::new();
    let mut counts = BTreeMap::new();
    for (set, base) in [("samples", samples.as_path()), ("upstream", root())] {
        if set == "upstream" && !have_upstream {
            continue;
        }
        let entries = data[set].as_object().expect("a set");
        let mut same = 0;
        for (name, php) in entries {
            let php = recorded(php);
            let port = answer(&guesser, &base.join(name));
            if php == port {
                same += 1;
            } else {
                differences.insert((set.into(), name.clone()), (php, port));
            }
        }
        counts.insert(set.to_string(), (same, entries.len()));
    }
    if std::env::var_os("SYMFONY_RS_WRITE_DIFFERENCES").is_some() {
        write(&differences, &counts);
        return;
    }
    let mut expected = listed();
    if !have_upstream {
        expected.retain(|(set, _), _| set != "upstream");
    }
    let unexpected: Vec<_> = differences
        .iter()
        .filter(|(k, v)| expected.get(*k) != Some(*v))
        .collect();
    let gone: Vec<_> = expected
        .iter()
        .filter(|(k, v)| differences.get(*k) != Some(*v))
        .collect();
    assert!(
        unexpected.is_empty() && gone.is_empty(),
        "not listed: {unexpected:#?}\nlisted, but not found: {gone:#?}"
    );
}
