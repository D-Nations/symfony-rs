//! Replays tests/data/schema.json (scripts/record.sh php-dom-schema): for
//! every recorded document, what `DOMDocument::schemaValidateSource()`
//! answered in PHP 8.4.27 with libxml 2.9.14 after XmlUtils's
//! `loadXML()`: the result and every `libxml_get_errors()` entry.
//!
//! The cases this crate answers differently are listed in `KNOWN` with the
//! reason; the test fails if any other case differs, or if a listed case
//! stops differing (so the list stays exact).
//!
//! Upstream: none

use std::collections::{BTreeMap, HashMap};

use php_dom::{LoadOptions, Schema, SchemaError, load_xml};
use serde_json::{Value, json};

/// The document URL PHP gave documents loaded from a string: the
/// oracle's working directory.
const DOCUMENT_FILE: &str = "/work/";

/// Cases answered differently: (schema, case, why). Every one is a
/// refusal; the test checks that it is.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "p-misc",
        "xsi:type",
        "refused: xsi:type is not modelled (libxml resolves the QName in the document)",
    ),
    (
        "unsupported-complexcontent",
        "valid",
        "refused: complexContent is outside the schemas' feature set",
    ),
    (
        "unsupported-all",
        "valid",
        "refused: xs:all is outside the schemas' feature set",
    ),
    (
        "unsupported-counts",
        "too few",
        "refused: occurrence bounds other than 0/1 and 1/unbounded (libxml's counters not modelled)",
    ),
];

fn bytes(case: &Value) -> Vec<u8> {
    match (case["xml"].as_str(), case["xml64"].as_str()) {
        (Some(x), _) => x.as_bytes().to_vec(),
        (None, Some(b)) => php_value::base64::decode_strict(b.as_bytes()).expect("base64"),
        _ => panic!("case without input"),
    }
}

fn errors_json(errors: &[php_dom::LibxmlError]) -> Vec<Value> {
    errors
        .iter()
        .map(|e| {
            json!([
                e.level.as_int(),
                e.code,
                e.message,
                e.file.as_deref().unwrap_or(""),
                e.line,
                e.column
            ])
        })
        .collect()
}

#[test]
fn recorded_schema_validation() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/schema.json"
    ))
    .expect("tests/data/schema.json");
    let data: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(data["libxml"], "2.9.14");
    let imports: HashMap<String, Vec<u8>> = data["imports"]
        .as_object()
        .expect("imports")
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_str().unwrap_or_default().as_bytes().to_vec(),
            )
        })
        .collect();
    let mut schemas: BTreeMap<String, Result<Schema, SchemaError>> = BTreeMap::new();
    let known: HashMap<(&str, &str), &str> =
        KNOWN.iter().map(|(s, n, why)| ((*s, *n), *why)).collect();
    let mut failures = Vec::new();
    let mut matched = 0;
    let cases = data["cases"].as_array().expect("cases");
    for case in cases {
        let schema_id = case["schema"].as_str().expect("schema");
        let name = case["name"].as_str().expect("name");
        let schema = schemas.entry(schema_id.to_owned()).or_insert_with(|| {
            let src = data["schemas"][schema_id].as_str().expect("schema source");
            Schema::parse(src.as_bytes(), &mut |loc| imports.get(loc).cloned())
        });
        let loaded = load_xml(
            &bytes(case),
            LoadOptions {
                validate_on_parse: true,
            },
        )
        .expect("load");
        let doc = loaded.document.expect("document");
        let got = match schema {
            Ok(s) => match s.validate(&doc, Some(DOCUMENT_FILE)) {
                Ok(v) => json!({"ok": v.valid, "errors": errors_json(&v.errors)}),
                Err(r) => json!({"refused": format!("{} (line {})", r.reason, r.line)}),
            },
            Err(SchemaError::Invalid(errors)) => {
                json!({"ok": false, "errors": errors_json(errors)})
            }
            Err(SchemaError::Refused(r)) => {
                json!({"refused": format!("{} (line {})", r.reason, r.line)})
            }
        };
        let want = json!({"ok": case["ok"], "errors": case["errors"]});
        let same = got == want;
        match (same, known.get(&(schema_id, name))) {
            (true, None) => matched += 1,
            (false, Some(why)) => {
                if why.starts_with("refused") && got.get("refused").is_none() {
                    failures.push(format!(
                        "{schema_id} | {name}: listed as refused, got {got}"
                    ));
                }
            }
            (true, Some(_)) => {
                failures.push(format!("{schema_id} | {name}: listed in KNOWN but matches"))
            }
            (false, None) => {
                failures.push(format!("{schema_id} | {name}\n  want {want}\n  got  {got}"))
            }
        }
    }
    let listed: usize = cases
        .iter()
        .filter(|c| {
            known.contains_key(&(
                c["schema"].as_str().unwrap_or(""),
                c["name"].as_str().unwrap_or(""),
            ))
        })
        .count();
    assert_eq!(
        listed,
        KNOWN.len(),
        "every KNOWN entry names a recorded case"
    );
    assert!(
        failures.is_empty(),
        "{} of {} cases differ ({matched} match):\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

fn refusal(source: &str) -> php_dom::RefusalReason {
    match Schema::parse(source.as_bytes(), &mut |_| None) {
        Err(SchemaError::Refused(r)) => r.reason,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn imports_come_only_from_the_resolver() {
    let src = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:import namespace="http://www.w3.org/XML/1998/namespace" schemaLocation="file:///etc/passwd"/><xs:element name="r"/></xs:schema>"#;
    let mut asked = Vec::new();
    let r = Schema::parse(src.as_bytes(), &mut |loc| {
        asked.push(loc.to_owned());
        None
    });
    assert_eq!(asked, ["file:///etc/passwd"]);
    match r {
        Err(SchemaError::Refused(r)) => assert_eq!(
            r.reason,
            php_dom::RefusalReason::UnresolvedImport {
                location: "file:///etc/passwd".into()
            }
        ),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn unsupported_schema_constructs_are_refused() {
    let xs = |body: &str| {
        format!(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">{body}</xs:schema>"#)
    };
    for body in [
        r#"<xs:element name="r" type="xs:decimal"/>"#,
        r#"<xs:include schemaLocation="x.xsd"/>"#,
        r#"<xs:element name="r" nillable="true"/>"#,
        r#"<xs:element name="r" substitutionGroup="q"/>"#,
        r#"<xs:simpleType name="t"><xs:restriction base="xs:string"><xs:pattern value="\p{L}+"/></xs:restriction></xs:simpleType>"#,
        r#"<xs:simpleType name="t"><xs:restriction base="xs:string"><xs:length value="2"/></xs:restriction></xs:simpleType>"#,
        r#"<xs:element name="r"><xs:complexType><xs:sequence><xs:element name="a" maxOccurs="5"/></xs:sequence></xs:complexType></xs:element>"#,
        r#"<xs:element name="r"><xs:complexType><xs:choice maxOccurs="unbounded"><xs:sequence><xs:element name="a"/></xs:sequence></xs:choice></xs:complexType></xs:element>"#,
        r#"<xs:element name="r"><xs:complexType><xs:attribute name="a" use="prohibited"/></xs:complexType></xs:element>"#,
    ] {
        let reason = refusal(&xs(body));
        assert!(
            matches!(reason, php_dom::RefusalReason::UnsupportedSchema { .. }),
            "{body}: {reason:?}"
        );
    }
}
