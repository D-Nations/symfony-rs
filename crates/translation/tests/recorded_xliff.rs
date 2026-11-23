//! Replays tests/data/xliff.json, recorded from XliffFileLoader and
//! XliffUtils on PHP 8.4.27 and libxml 2.9.14 (`scripts/record.sh
//! translation-xliff`, oracle/recorders/translation_xliff.php): `load()` of
//! upstream's fixtures and of written documents, as files and strings, and
//! of resources that are not XML; `getFileUrl()`; `getVersionNumber()`;
//! `validateSchema()` on a document declaring an external entity.
//!
//! For each load: the catalogue's locale, messages, metadata and catalogue
//! metadata, or the exception chain (class, message, code), and
//! `libxml_use_internal_errors()` afterwards. Resources are compared as
//! their `__toString()` (the FileResource of a file).
//!
//! The recorder wrote its files under /tmp/xliff and ran there; this test
//! writes the same files to a temporary directory, runs there, and reads
//! the recorded paths as that directory. It is one test so that changing
//! the working directory races with nothing.
//!
//! Upstream: none

use php_dom::{LoadOptions, libxml_use_internal_errors, load_xml};
use php_value::{
    Value,
    vectors::{base64, decode},
};
use serde_json::Value as Json;
use symfony_translation::{Error, LoaderInterface, Previous, XliffFileLoader, XliffUtils};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

const RECORDED_BASE: &[u8] = b"/tmp/xliff";

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

/// The exception chain as the recorder writes it: class, message, code.
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

fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
        }
        _ => expected == actual,
    }
}

/// Loads that differ from PHP, stated in README.md: symfony/config's
/// `FileResource` does not resolve these two names as PHP's `realpath()` and
/// `file_exists()` do, so it throws "does not exist" where PHP adds the
/// resource. `foo://bar` is the relative path `foo:/bar` for `realpath()`;
/// `file_exists()` reads a `file://localhost/` URL.
fn is_known(kind: &str, resource: &[u8]) -> bool {
    kind == "raw" && (resource == b"foo://bar" || resource.starts_with(b"file://localhost/"))
}

#[test]
fn recorded_xliff() -> Result<(), BoxError> {
    let data: Json = serde_json::from_str(include_str!("data/xliff.json"))?;
    let dir =
        std::env::temp_dir().join(format!("symfony-translation-xliff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["dir", "sub", "foo:"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
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
    let mut known = 0;
    let mut count = 0;
    for case in data["cases"].as_array().ok_or("cases")? {
        let kind = text(&case[0])?;
        let resource: Vec<u8> = match kind {
            "file" => text(&case[1])?.as_bytes().to_vec(),
            "absolute" => {
                let mut r = base.clone();
                r.push(b'/');
                r.extend_from_slice(text(&case[1])?.as_bytes());
                r
            }
            "string" => base64(text(&case[1])?)?,
            _ => replace(&base64(text(&case[1])?)?, RECORDED_BASE, &base),
        };
        let (locale, domain) = (text(&case[2])?.as_bytes(), text(&case[3])?.as_bytes());
        let outcome = &case[4];
        let label = format!(
            "{kind} {}",
            String::from_utf8_lossy(&resource[..resource.len().min(120)])
        );

        let flags = &outcome[2];
        libxml_use_internal_errors(Some(flags[0].as_bool().ok_or("flag")?));
        let result = XliffFileLoader.load(&resource, locale, domain);
        let after = libxml_use_internal_errors(None);
        count += 1;

        if Some(after) != flags[1].as_bool() {
            failures.push(format!(
                "{label}: internal errors {after}, expected {}",
                flags[1]
            ));
        }

        let before = failures.len();
        match (text(&outcome[0])?, &result) {
            ("ok", Ok(catalogue)) => {
                let expected = &outcome[1];
                let (messages, metadata, catalogue_metadata) = catalogue.raw();
                if catalogue.get_locale().as_bytes() != text(&expected["locale"])?.as_bytes() {
                    failures.push(format!("{label}: locale"));
                }
                for (field, actual) in [
                    ("messages", messages),
                    ("metadata", metadata),
                    ("catalogue_metadata", catalogue_metadata),
                ] {
                    let want = decode(&expected[field])?;
                    if !same(&want, &Value::Array(actual.clone())) {
                        failures.push(format!(
                            "{label}: {field}\n  expected {want:?}\n  actual   {actual:?}"
                        ));
                    }
                }
                let mut want = Vec::new();
                for r in expected["resources"].as_array().ok_or("resources")? {
                    want.push(replace(text(r)?.as_bytes(), RECORDED_BASE, &real_base));
                }
                let actual: Vec<Vec<u8>> = catalogue
                    .get_resources()
                    .iter()
                    .map(|r| r.to_string())
                    .collect();
                if want != actual {
                    let show = |v: &[Vec<u8>]| {
                        v.iter()
                            .map(|r| String::from_utf8_lossy(r).into_owned())
                            .collect::<Vec<_>>()
                    };
                    failures.push(format!(
                        "{label}: resources\n  expected {:?}\n  actual   {:?}",
                        show(&want),
                        show(&actual)
                    ));
                }
            }
            ("throws", Err(e)) => {
                let mut want = Vec::new();
                for entry in outcome[1].as_array().ok_or("chain")? {
                    want.push((
                        text(&entry[0])?.to_owned(),
                        replace(&base64(text(&entry[1])?)?, RECORDED_BASE, &base),
                        entry[2].as_i64().ok_or("code")?,
                    ));
                }
                let actual = chain(e);
                if want != actual {
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
                        show(&actual)
                    ));
                }
            }
            (expected, _) => failures.push(format!("{label}: expected {expected}, got {result:?}")),
        }
        if is_known(kind, &resource) {
            let refused = matches!(&result, Err(e) if e.class().name() == "InvalidArgumentException"
                && e.get_message().ends_with(b"\" does not exist."));
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
    eprintln!("{known} recorded loads differ as listed in is_known()");

    std::env::set_current_dir(previous_cwd)?;
    let _ = std::fs::remove_dir_all(&dir);

    for pair in data["file_urls"].as_array().ok_or("file_urls")? {
        let (path, url) = (base64(text(&pair[0])?)?, base64(text(&pair[1])?)?);
        count += 1;
        if XliffUtils::get_file_url(&path) != url {
            failures.push(format!("getFileUrl({})", String::from_utf8_lossy(&path)));
        }
    }

    for pair in data["versions"].as_array().ok_or("versions")? {
        let xml = base64(text(&pair[0])?)?;
        let dom = load_xml(&xml, LoadOptions::default())?
            .document
            .ok_or("document")?;
        count += 1;
        let want = decode(&pair[1])?;
        let got = Value::string(XliffUtils::get_version_number(&dom).as_str());
        if want != got {
            failures.push(format!(
                "getVersionNumber({}): {got:?}, expected {want:?}",
                String::from_utf8_lossy(&xml)
            ));
        }
    }

    for case in data["tag_names"].as_array().ok_or("tag_names")? {
        let xml = base64(text(&case[0])?)?;
        let dom = load_xml(&xml, LoadOptions::default())?
            .document
            .ok_or("document")?;
        let name = text(&case[1])?;
        let want: Vec<&str> = case[2]
            .as_array()
            .ok_or("names")?
            .iter()
            .filter_map(Json::as_str)
            .collect();
        let got: Vec<&str> = dom
            .get_elements_by_tag_name(dom.root(), name)
            .into_iter()
            .map(|e| dom.node_name(e))
            .collect();
        count += 1;
        if want != got {
            failures.push(format!(
                "getElementsByTagName({name}): {got:?}, expected {want:?}"
            ));
        }
    }

    let external = &data["external_entity"];
    let xml = b"<?xml version=\"1.0\"?><!DOCTYPE xliff [<!ENTITY xxe SYSTEM \"http://127.0.0.1:1/payload.dtd\">]><xliff version=\"1.2\" xmlns=\"urn:oasis:names:tc:xliff:document:1.2\"><file source-language=\"en\" datatype=\"plaintext\" original=\"file.ext\"><body/></file></xliff>";
    let dom = load_xml(xml, LoadOptions::default())?
        .document
        .ok_or("document")?;
    count += 1;
    match XliffUtils::validate_schema(&dom) {
        Ok(Ok(errors))
            if errors.is_empty() && decode(external)? == Value::Array(Default::default()) => {}
        other => failures.push(format!(
            "validateSchema() with an external entity: {other:?}, expected {external}"
        )),
    }

    assert!(
        failures.is_empty(),
        "{} of {count} recorded cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}
