//! Replays tests/data/parse.json (scripts/record.sh php-dom): for every
//! recorded document, what `DOMDocument::loadXML()` answered in PHP 8.4.27
//! with libxml 2.9.14, once with XmlUtils's settings (`validateOnParse`)
//! and once plain: whether it loaded, `libxml_get_errors()`, and the tree.
//!
//! The cases this crate answers differently are listed in `KNOWN` with the
//! reason; the test fails if any other case differs, or if a listed case
//! stops differing (so the list stays exact).
//!
//! Upstream: none

use std::collections::BTreeMap;

use php_dom::{Document, LoadError, LoadOptions, NodeId, NodeType, load_xml};
use serde_json::{Value, json};

/// Cases answered differently, and why. `refused`: this crate refuses
/// the document (no file, network or iconv); the others are named
/// diagnostics libxml produces from state this crate does not model.
const KNOWN: &[(&str, &str)] = &[
    (
        "doctype system",
        "refused: external DTD subset (PHP reads it from its working directory)",
    ),
    (
        "doctype public",
        "refused: external DTD subset (PHP's NONET blocks the fetch)",
    ),
    (
        "external dtd file",
        "refused: external DTD subset (PHP reads the local file)",
    ),
    ("external dtd http", "refused: external DTD subset"),
    (
        "external entity file",
        "refused: external entity reference in content (PHP reads the file when validating)",
    ),
    (
        "external entity http",
        "refused: external entity reference in content",
    ),
    (
        "external entity php filter",
        "refused: external entity reference in content",
    ),
    (
        "external parameter entity",
        "refused: external parameter entity (PHP reads the file when validating)",
    ),
    (
        "unknown encoding",
        "refused: encodings beyond UTF-8/16, ISO-8859-1 and ASCII are iconv's in PHP",
    ),
    ("windows-1252", "refused: iconv encoding"),
    ("windows-1252 undefined byte", "refused: iconv encoding"),
    (
        "declaration encoding iso-8859-15",
        "refused: iconv encoding",
    ),
    ("utf-16le bom lone surrogate", "refused: invalid UTF-16"),
    (
        "billion laughs",
        "libxml's entity amplification check (counts of consumed bytes) is not modelled",
    ),
    (
        "billion laughs attribute",
        "libxml's entity amplification check is not modelled",
    ),
];

fn input(case: &Value) -> Vec<u8> {
    match case["input"].as_str() {
        Some(b64) => php_value::base64::decode_strict(b64.as_bytes()).expect("base64"),
        None => match case["name"].as_str() {
            Some("attribute value 20 MB") => {
                let mut v = b"<a x=\"".to_vec();
                v.extend(std::iter::repeat_n(b'v', 20_000_000));
                v.extend_from_slice(b"\"/>");
                v
            }
            other => panic!("no recipe for {other:?}"),
        },
    }
}

fn opt(s: Option<&str>) -> Value {
    s.map_or(Value::Null, |s| json!(s))
}

fn dump(doc: &Document) -> Vec<Value> {
    let mut out = Vec::new();
    dump_node(doc, doc.root(), 0, &mut out);
    out
}

fn dump_node(doc: &Document, id: NodeId, depth: usize, out: &mut Vec<Value>) {
    let row = match doc.node_type(id) {
        NodeType::Document => json!([
            depth,
            "Doc",
            opt(doc.encoding()),
            opt(doc.encoding()),
            doc.xml_version(),
            doc.xml_standalone()
        ]),
        NodeType::Element => {
            let attrs: Vec<Value> = doc
                .attributes(id)
                .iter()
                .map(|a| {
                    json!([
                        a.name,
                        a.local_name,
                        a.prefix.as_deref().unwrap_or(""),
                        opt(a.namespace_uri.as_deref()),
                        a.value
                    ])
                })
                .collect();
            json!([
                depth,
                "E",
                doc.node_name(id),
                opt(doc.local_name(id)),
                doc.prefix(id).unwrap_or(""),
                opt(doc.namespace_uri(id)),
                doc.line(id),
                attrs
            ])
        }
        NodeType::Text => json!([depth, "T", opt(doc.node_value(id))]),
        NodeType::CData => json!([depth, "C", opt(doc.node_value(id))]),
        NodeType::Comment => json!([depth, "#", opt(doc.node_value(id))]),
        NodeType::ProcessingInstruction => {
            let (t, d) = doc.processing_instruction(id).unwrap_or_default();
            json!([depth, "P", t, d])
        }
        NodeType::EntityReference => json!([depth, "R", doc.node_name(id)]),
        NodeType::DocumentType => {
            let (n, p, s) = doc.document_type(id).unwrap_or_default();
            json!([depth, "D", n, p, s])
        }
    };
    out.push(row);
    if !matches!(
        doc.node_type(id),
        NodeType::EntityReference | NodeType::DocumentType
    ) {
        for &c in doc.children(id) {
            dump_node(doc, c, depth + 1, out);
        }
    }
}

/// The recorded tree with what this crate does not claim removed: a
/// document type's `internalSubset` (libxml's re-serialisation).
fn expected_tree(tree: &Value) -> Vec<Value> {
    tree.as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    let mut r = r.as_array().cloned().unwrap_or_default();
                    if r.get(1).and_then(Value::as_str) == Some("D") {
                        r.truncate(5);
                    }
                    Value::Array(r)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Validity errors (5xx) and I/O warnings (15xx) for documents with a
/// document type: DTD validation is not modelled (XmlUtils rejects every
/// document type anyway).
fn claimed_errors(errors: &Value, has_doctype: bool) -> Vec<Value> {
    errors
        .as_array()
        .map(|es| {
            es.iter()
                .filter(|e| {
                    let code = e[1].as_u64().unwrap_or(0);
                    !(has_doctype && ((500..600).contains(&code) || (1500..1600).contains(&code)))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn compare(
    name: &str,
    mode: &str,
    bytes: &[u8],
    validate: bool,
    rec: &Value,
    utils_tree: &Value,
) -> Result<(), String> {
    let has_doctype = utils_tree
        .as_array()
        .is_some_and(|t| t.iter().any(|r| r[1] == "D"))
        || bytes.windows(9).any(|w| w == b"<!DOCTYPE");
    let got = load_xml(
        bytes,
        LoadOptions {
            validate_on_parse: validate,
        },
    );
    if let Some(t) = rec.get("throws") {
        return match got {
            Err(LoadError::Empty) if t[1] == LoadError::EMPTY_MESSAGE => Ok(()),
            other => Err(format!("{name} [{mode}]: expected {t}, got {other:?}")),
        };
    }
    let loaded = match got {
        Ok(l) => l,
        Err(e) => return Err(format!("{name} [{mode}]: refused: {e}")),
    };
    let errors: Vec<Value> = loaded
        .errors
        .iter()
        .map(|e| json!([e.level.as_int(), e.code, e.message, e.line, e.column]))
        .collect();
    let want_errors = claimed_errors(&rec["errors"], has_doctype);
    let got_errors = claimed_errors(&Value::Array(errors), has_doctype);
    let mut diffs = Vec::new();
    if rec["ok"].as_bool() != Some(loaded.document.is_some()) {
        diffs.push(format!(
            "ok: want {}, got {}",
            rec["ok"],
            loaded.document.is_some()
        ));
    }
    if want_errors != got_errors {
        diffs.push(format!(
            "errors:\n      want {}\n      got  {}",
            Value::Array(want_errors),
            Value::Array(got_errors)
        ));
    }
    if let Some(doc) = &loaded.document {
        let tree = if rec["tree"] == "same" {
            utils_tree
        } else {
            &rec["tree"]
        };
        if !tree.is_null() {
            let want = expected_tree(tree);
            let got = dump(doc);
            if want != got {
                let at = want
                    .iter()
                    .zip(&got)
                    .position(|(a, b)| a != b)
                    .unwrap_or(want.len().min(got.len()));
                diffs.push(format!(
                    "tree differs at row {at}:\n      want {}\n      got  {}",
                    want.get(at).map_or("(end)".into(), Value::to_string),
                    got.get(at).map_or("(end)".into(), Value::to_string)
                ));
            }
        }
    }
    if diffs.is_empty() {
        Ok(())
    } else {
        Err(format!("{name} [{mode}]: {}", diffs.join("\n    ")))
    }
}

#[test]
fn load_xml_matches_recorded_php() {
    // The recorded trees nest deeply. When the workspace builds serde_json
    // with `unbounded_depth` (yaml and http-foundation enable it), reading
    // and comparing them needs more than the 2 MiB test-thread stack.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(load_xml_matches_recorded_php_body)
        .expect("spawn")
        .join()
        .expect("load_xml_matches_recorded_php");
}

fn load_xml_matches_recorded_php_body() {
    let data: Value = serde_json::from_str(include_str!("data/parse.json")).expect("parse.json");
    assert_eq!(data["libxml"], "2.9.14");
    let known: BTreeMap<&str, &str> = KNOWN.iter().copied().collect();
    let cases = data["cases"].as_array().expect("cases");
    let mut failures = Vec::new();
    let mut matched = 0;
    let mut listed_differ = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or_default();
        let bytes = input(case);
        let utils_tree = case["utils"].get("tree").cloned().unwrap_or(Value::Null);
        let results = [
            compare(name, "utils", &bytes, true, &case["utils"], &utils_tree),
            compare(name, "plain", &bytes, false, &case["plain"], &utils_tree),
        ];
        let errs: Vec<String> = results.into_iter().filter_map(Result::err).collect();
        match (errs.is_empty(), known.contains_key(name)) {
            (true, false) => matched += 1,
            (true, true) => failures.push(format!("{name}: listed in KNOWN but now matches")),
            (false, true) => listed_differ.push(name),
            (false, false) => failures.extend(errs),
        }
    }
    for k in known.keys() {
        if !cases.iter().any(|c| c["name"] == *k) {
            failures.push(format!("{k}: listed in KNOWN but not recorded"));
        }
    }
    eprintln!(
        "{matched} of {} recorded documents match; {} listed differences",
        cases.len(),
        listed_differ.len()
    );
    assert!(
        failures.is_empty(),
        "{} mismatches:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
