//! Replays tests/data/qt.json, recorded from QtFileLoader and DOMXPath on
//! PHP 8.4.27 and libxml 2.9.14 (`scripts/record.sh translation-qt`,
//! oracle/recorders/translation_qt.php): `load()` of upstream's
//! resources.ts, of written documents and of other resources, and
//! `DOMXPath::evaluate()` over one document.
//!
//! For each load: the catalogue's locale, messages (with integer or string
//! keys) and resources, or the throwable chain (class, message, code), and
//! `libxml_use_internal_errors()` afterwards. For each evaluate(): the
//! nodes found, as sibling positions from the document, or `false`.
//!
//! The recorder wrote its files under /tmp/qt and ran there; this test
//! writes the same files to a temporary directory, runs there, and reads
//! the recorded paths as that directory. It is one test so that changing
//! the working directory races with nothing.
//!
//! Upstream: none

use php_dom::{
    Document, LoadOptions, NodeId, XPathError, libxml_use_internal_errors, load_xml, xpath,
};
use php_value::{Key, vectors::base64};
use serde_json::Value as Json;
use symfony_translation::{Error, LoaderInterface, Previous, QtFileLoader};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

const RECORDED_BASE: &[u8] = b"/tmp/qt";

fn replace(haystack: &[u8], search: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(search) {
            out.extend_from_slice(with);
            i += search.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

fn text(json: &Json) -> Result<&str, BoxError> {
    json.as_str()
        .ok_or_else(|| format!("expected a string, got {json}").into())
}

fn b64(json: &Json) -> Result<Vec<u8>, BoxError> {
    base64(text(json)?)
}

/// The throwable chain as the recorder writes it: class, message, code.
fn chain(e: &Error) -> Vec<(String, Vec<u8>, i64)> {
    let mut out = vec![(
        e.class().name().to_owned(),
        e.get_message().to_vec(),
        e.get_code(),
    )];
    let mut previous = e.get_previous().cloned();
    while let Some(p) = previous {
        previous = match p {
            Previous::Translation(t) => {
                out.push((
                    t.class().name().to_owned(),
                    t.get_message().to_vec(),
                    t.get_code(),
                ));
                t.get_previous().cloned()
            }
            Previous::Config(c) => {
                let mut c = Some(c);
                while let Some(e) = c {
                    out.push((
                        e.class().name().to_owned(),
                        e.get_message().to_vec(),
                        e.get_code(),
                    ));
                    c = e.get_previous().cloned();
                }
                None
            }
        };
    }
    out
}

/// A key as the recorder writes it: its type and its text.
fn key(k: &Key) -> (String, Vec<u8>) {
    match k {
        Key::Int(i) => ("int".to_owned(), i.to_string().into_bytes()),
        Key::Str(s) => ("string".to_owned(), s.as_bytes().to_vec()),
    }
}

type Messages = Vec<(Vec<u8>, Vec<(String, Vec<u8>, Vec<u8>)>)>;

/// The recorder's path of a node: 1-based sibling positions from the
/// document.
fn node_path(doc: &Document, mut n: NodeId) -> String {
    let mut parts = Vec::new();
    while let Some(parent) = doc.parent(n) {
        let position = doc
            .children(parent)
            .iter()
            .position(|&c| c == n)
            .expect("a child of its parent");
        parts.push((position + 1).to_string());
        n = parent;
    }
    parts.reverse();
    parts.join("/")
}

/// Loads that differ from PHP, stated in README.md: symfony/config's
/// `FileResource` does not read a `file://localhost/` URL as PHP's
/// `file_exists()` does, so it throws "does not exist" where PHP adds the
/// resource.
fn is_known(resource: &[u8]) -> bool {
    resource.starts_with(b"file://localhost/")
}

/// `evaluate()` cases outside php-dom's XPath subset (refused).
const REFUSED_EXPRESSIONS: [&str; 1] = ["//q:context"];

#[test]
fn recorded_qt() -> Result<(), BoxError> {
    let data: Json = serde_json::from_str(include_str!("data/qt.json"))?;
    let dir = std::env::temp_dir().join(format!("symfony-translation-qt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("dir"))?;
    for (name, content) in data["files"].as_object().ok_or("files")? {
        std::fs::write(dir.join(name), base64(text(content)?)?)?;
    }
    let base = dir.to_string_lossy().into_owned().into_bytes();
    // PHP's realpath(), as FileResource computes it (no `\\?\` prefix,
    // `/` separators on Windows)
    let real_base = symfony_finder::fs::realpath(&base).ok_or("realpath")?;
    let previous_cwd = std::env::current_dir()?;
    std::env::set_current_dir(&dir)?;

    let mut failures = Vec::new();
    let mut count = 0;
    let mut known = 0;
    for case in data["cases"].as_array().ok_or("cases")? {
        let resource = replace(&b64(&case[1])?, RECORDED_BASE, &base);
        let locale = text(&case[2])?.as_bytes();
        let domain = b64(&case[3])?;
        let outcome = &case[4];
        let label = format!(
            "{} {} (domain {})",
            text(&case[0])?,
            String::from_utf8_lossy(&resource),
            String::from_utf8_lossy(&domain)
        );
        let flags = &outcome[2];
        libxml_use_internal_errors(Some(flags[0].as_bool().ok_or("flag")?));
        let result = QtFileLoader.load(&resource, locale, &domain);
        let after = libxml_use_internal_errors(Some(false));
        count += 1;

        let before = failures.len();
        if Some(after) != flags[1].as_bool() {
            failures.push(format!(
                "{label}: internal errors {after}, expected {}",
                flags[1]
            ));
        }
        match (text(&outcome[0])?, &result) {
            ("ok", Ok(catalogue)) => {
                let expected = &outcome[1];
                if catalogue.get_locale().as_bytes() != text(&expected["locale"])?.as_bytes() {
                    failures.push(format!("{label}: locale"));
                }
                let mut want: Messages = Vec::new();
                for d in expected["messages"].as_array().ok_or("messages")? {
                    let mut pairs = Vec::new();
                    for p in d[1].as_array().ok_or("pairs")? {
                        pairs.push((text(&p[0])?.to_owned(), b64(&p[1])?, b64(&p[2])?));
                    }
                    want.push((b64(&d[0])?, pairs));
                }
                let (messages, _, _) = catalogue.raw();
                let mut got: Messages = Vec::new();
                for (d, pairs) in messages.iter() {
                    let mut list = Vec::new();
                    for (k, v) in pairs.as_array().ok_or("domain")?.iter() {
                        let (kind, k) = key(k);
                        list.push((kind, k, v.as_string().ok_or("value")?.as_bytes().to_vec()));
                    }
                    got.push((key(d).1, list));
                }
                if want != got {
                    failures.push(format!(
                        "{label}: messages\n  expected {want:?}\n  actual   {got:?}"
                    ));
                }
                let mut want = Vec::new();
                for r in expected["resources"].as_array().ok_or("resources")? {
                    want.push(replace(&b64(r)?, RECORDED_BASE, &real_base));
                }
                let got: Vec<Vec<u8>> = catalogue
                    .get_resources()
                    .iter()
                    .map(|r| r.to_string())
                    .collect();
                if want != got {
                    failures.push(format!("{label}: resources {got:?}, expected {want:?}"));
                }
            }
            ("throws", Err(e)) => {
                let mut want = Vec::new();
                for entry in outcome[1].as_array().ok_or("chain")? {
                    want.push((
                        text(&entry[0])?.to_owned(),
                        replace(&b64(&entry[1])?, RECORDED_BASE, &base),
                        entry[2].as_i64().ok_or("code")?,
                    ));
                }
                let got = chain(e);
                if want != got {
                    let show = |c: &[(String, Vec<u8>, i64)]| {
                        c.iter()
                            .map(|(class, m, code)| {
                                format!("{class} [{code}] {}", String::from_utf8_lossy(m))
                            })
                            .collect::<Vec<_>>()
                            .join(" <- ")
                    };
                    failures.push(format!(
                        "{label}:\n  expected {}\n  actual   {}",
                        show(&want),
                        show(&got)
                    ));
                }
            }
            (expected, _) => failures.push(format!("{label}: expected {expected}, got {result:?}")),
        }
        if is_known(&resource) {
            let refused =
                matches!(&result, Err(e) if e.get_message().ends_with(b"\" does not exist."));
            if failures.len() == before || !refused {
                failures.truncate(before);
                failures.push(format!(
                    "{label}: listed in is_known() but now differs otherwise or matches"
                ));
            } else {
                failures.truncate(before);
                known += 1;
            }
        }
    }
    std::env::set_current_dir(previous_cwd)?;

    let xml = b64(&data["xpath_document"])?;
    let doc = load_xml(&xml, LoadOptions::default())?
        .document
        .ok_or("document")?;
    let mut refused = 0;
    for case in data["evaluate"].as_array().ok_or("evaluate")? {
        let expression = b64(&case[0])?;
        let shown = String::from_utf8_lossy(&expression).into_owned();
        count += 1;
        let got = xpath::evaluate(&doc, &expression);
        if REFUSED_EXPRESSIONS.contains(&shown.as_str()) {
            if matches!(got, Err(XPathError::Refused(_))) {
                refused += 1;
            } else {
                failures.push(format!("evaluate({shown}): listed as refused, got {got:?}"));
            }
            continue;
        }
        let want = &case[1];
        let same = match (&got, want) {
            (Err(XPathError::Invalid), Json::Bool(false)) => true,
            (Ok(nodes), Json::Array(paths)) => {
                let got: Vec<String> = nodes.iter().map(|&n| node_path(&doc, n)).collect();
                let want: Vec<&str> = paths.iter().filter_map(Json::as_str).collect();
                got == want
            }
            _ => false,
        };
        if !same {
            failures.push(format!("evaluate({shown}): {got:?}, expected {want}"));
        }
    }

    eprintln!(
        "{known} recorded loads differ as listed in is_known(); {refused} evaluate() cases refused"
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {count} recorded cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}
