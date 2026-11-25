//! Replays `tests/data/xml_utils.json`, recorded from `XmlUtils` on PHP 8.4.27
//! and libxml 2.9.14 (`scripts/record.sh config-xml-utils`,
//! `oracle/recorders/config_xml_utils.php`): `phpize()` over a value corpus,
//! `convertDomElementToArray()` over documents, and the outcome of
//! `parse()` and `loadFile()` (exception class, message, code and previous
//! exception) for malformed documents, document types, schema files and
//! their imports, and callables.
//!
//! The recorder wrote its files under /tmp/xml-utils and ran there; this
//! test writes the same files to a temporary directory, runs there, and
//! reads the recorded paths as that directory. It is one test so that
//! changing the working directory races with nothing.
//!
//! Upstream: none

use std::path::{Path, PathBuf};

use php_dom::{
    Document, LibxmlError, libxml_use_internal_errors, load_xml, schema_validate_source,
};
use php_value::{
    Value,
    vectors::{base64, decode},
};
use serde_json::Value as Json;
use symfony_config::{Class, Error, XmlUtils, util::Validation};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type TestResult = Result<(), BoxError>;

const RECORDED_BASE: &str = "/tmp/xml-utils";

/// Cases php-dom refuses (PLAN.md decision 14), with the start of the
/// `XmlParsingException` message the port gives instead of PHP's answer:
/// imports libxml fails to load (it then reports I/O warnings and skips
/// the import; php-dom has nothing it may read in their place).
const REFUSED: &[(&str, &str, &str)] = &[
    (
        "imp/rel.xsd",
        ".",
        "imported schema \"part.xsd\" is not loaded",
    ),
    (
        "imp/space.xsd",
        ".",
        "imported schema \"imp/a b.xsd\" is not loaded",
    ),
];

/// A string PHP calls instead of reading it as a path: `dir` is a
/// function, so `parse($c, 'dir')` calls `dir($dom, $internalErrors)`. Rust
/// callers choose `Validation::Schema` or `Validation::Callable`.
const PHP_CALLABLE_NAMES: &[&str] = &["dir"];

fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
        }
        _ => expected == actual,
    }
}

fn str_of(json: &Json) -> Result<&str, BoxError> {
    json.as_str()
        .ok_or_else(|| format!("expected a string, got {json}").into())
}

struct Paths {
    base: String,
    cwd_url: String,
}

impl Paths {
    /// A recorded message with the recorder's directories replaced by ours.
    fn expected(&self, recorded: &str) -> String {
        recorded
            .replace(
                &format!("(in {RECORDED_BASE}/ - "),
                &format!("(in {} - ", self.cwd_url),
            )
            .replace(RECORDED_BASE, &self.base)
    }
}

fn describe(result: &Result<Document, Error>) -> Json {
    match result {
        Ok(dom) => {
            let name = dom.document_element().map(|e| dom.node_name(e).to_string());
            serde_json::json!(["ok", name])
        }
        Err(e) => {
            let previous = e.get_previous().map(|p| {
                serde_json::json!([p.class().name(), String::from_utf8_lossy(p.get_message())])
            });
            serde_json::json!([
                "throws",
                e.class().name(),
                String::from_utf8_lossy(e.get_message()),
                e.get_code(),
                previous
            ])
        }
    }
}

fn expected_outcome(paths: &Paths, recorded: &Json) -> Json {
    match recorded[0].as_str() {
        Some("throws") => {
            let previous = if recorded[4].is_null() {
                Json::Null
            } else {
                serde_json::json!([
                    recorded[4][0],
                    paths.expected(recorded[4][1].as_str().unwrap_or(""))
                ])
            };
            serde_json::json!([
                "throws",
                recorded[1],
                paths.expected(recorded[2].as_str().unwrap_or("")),
                recorded[3],
                previous
            ])
        }
        _ => recorded.clone(),
    }
}

/// The callables the recorder passes, by name.
fn callable(
    name: &str,
    base: &Path,
) -> impl FnMut(&Document, bool, &mut Vec<LibxmlError>) -> Result<Value, Error> + use<> {
    let name = name.to_string();
    let schema = std::fs::read(base.join("schema.xsd")).unwrap_or_default();
    move |dom: &Document, _internal: bool, errors: &mut Vec<LibxmlError>| {
        let validate = |errors: &mut Vec<LibxmlError>| -> Result<bool, Error> {
            let cwd = std::env::current_dir().ok();
            let url = cwd.map(|c| format!("{}/", c.display()));
            let result = schema_validate_source(dom, &schema, &mut |_| None, url.as_deref())
                .map_err(|r| Error::new(Class::Runtime, r.to_string()))?;
            errors.extend(result.errors);
            Ok(result.valid)
        };
        let other = |class: &str, exception: bool| Class::Other {
            class: class.into(),
            exception,
        };
        match name.as_str() {
            "false" => Ok(Value::Bool(false)),
            "true" => Ok(Value::Bool(true)),
            "null" => Ok(Value::Null),
            "zero" => Ok(Value::Int(0)),
            "zero-string" => Ok(Value::string("0")),
            "one" => Ok(Value::Int(1)),
            "empty-array" => Ok(Value::Array(php_value::PhpArray::default())),
            "array" => {
                let mut a = php_value::PhpArray::new();
                a.insert(0, Value::Int(0));
                Ok(Value::Array(a))
            }
            "string" => Ok(Value::string("yes")),
            "exception" => Err(Error::new(Class::Runtime, "boom")),
            "logic-exception" => Err(Error::new(other("LogicException", true), "logic")),
            "error" => Err(Error::new(Class::Error, "bad")),
            "schema-false" => validate(errors).map(Value::Bool),
            "schema-true" => validate(errors).map(|_| Value::Bool(true)),
            "schema-throw" => {
                validate(errors)?;
                Err(Error::new(Class::Runtime, "after errors"))
            }
            other => Err(Error::new(
                Class::Error,
                format!("unknown callable {other}"),
            )),
        }
    }
}

fn run_validation(
    paths: &Paths,
    base: &Path,
    spec: &Json,
    run: &mut dyn FnMut(Option<Validation<'_>>) -> Result<Document, Error>,
) -> Result<Result<Document, Error>, BoxError> {
    let _ = paths;
    Ok(match spec[0].as_str() {
        None => run(None),
        Some("schema") => {
            let schema = PathBuf::from(str_of(&spec[1])?.replace(RECORDED_BASE, &paths.base));
            run(Some(Validation::Schema(&schema)))
        }
        Some("callable") => {
            let mut f = callable(str_of(&spec[1])?, base);
            run(Some(Validation::Callable(&mut f)))
        }
        Some(other) => return Err(format!("unknown validation {other}").into()),
    })
}

#[test]
fn recorded_xml_utils() -> TestResult {
    let data: Json = serde_json::from_slice(&std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xml_utils.json"),
    )?)?;

    // phpize()
    let mut count = 0;
    for case in data["phpize"].as_array().ok_or("phpize")? {
        let input = base64(str_of(&case[0])?)?;
        let expected = decode(&case[1])?;
        let actual = XmlUtils::phpize(&input);
        assert!(
            same(&expected, &actual),
            "phpize({:?}): expected {expected:?}, got {actual:?}",
            String::from_utf8_lossy(&input)
        );
        count += 1;
    }
    assert!(count > 3000, "{count} phpize cases");

    // convertDomElementToArray()
    for case in data["convert"].as_array().ok_or("convert")? {
        let source = base64(str_of(&case[0])?)?;
        let check_prefix = case[1].as_bool().ok_or("check")?;
        let dom = load_xml(&source, php_dom::LoadOptions::default())?
            .document
            .ok_or("loadXML() failed")?;
        let element = dom.document_element().ok_or("no document element")?;
        let actual = XmlUtils::convert_dom_element_to_array(&dom, element, check_prefix);
        let shown = String::from_utf8_lossy(&source).into_owned();
        match (case[2][0].as_str(), actual) {
            (Some("throws"), Err(e)) => {
                assert_eq!(str_of(&case[2][1])?, e.class().name(), "{shown}");
                assert_eq!(
                    str_of(&case[2][2])?,
                    String::from_utf8_lossy(e.get_message()),
                    "{shown}"
                );
            }
            (Some("throws"), Ok(v)) => panic!("{shown}: expected {}, got {v:?}", case[2]),
            (_, Ok(actual)) => {
                let expected = decode(&case[2])?;
                assert!(
                    same(&expected, &actual),
                    "{shown}: expected {expected:?}, got {actual:?}"
                );
            }
            (_, Err(e)) => panic!("{shown}: expected {}, got {e}", case[2]),
        }
    }

    // the files, in a directory of our own
    let base =
        std::env::temp_dir().join(format!("symfony-config-xml-utils-{}", std::process::id()));
    std::fs::create_dir_all(base.join("imp"))?;
    std::fs::create_dir_all(base.join("dir"))?;
    let base_string = base.to_string_lossy().into_owned();
    // the schemas that import by absolute path or file: URL name our directory
    let url_path = base_string.replace('\\', "/");
    let url_path = if url_path.starts_with('/') {
        url_path
    } else {
        format!("/{url_path}")
    };
    for (name, content) in data["files"].as_object().ok_or("files")? {
        let bytes = base64(str_of(content)?)?;
        let bytes = String::from_utf8(bytes)?
            .replace(
                &format!("file://{RECORDED_BASE}"),
                &format!("file://{url_path}"),
            )
            .replace(RECORDED_BASE, &base_string);
        std::fs::write(base.join(name), bytes)?;
    }
    let start = std::env::current_dir()?;
    std::env::set_current_dir(&base)?;
    let paths = Paths {
        base: base_string.clone(),
        cwd_url: format!("{}/", std::env::current_dir()?.display()),
    };

    let mut failures = Vec::new();
    let mut refused_seen = 0;

    // parse()
    for case in data["parse"].as_array().ok_or("parse")? {
        let content = base64(str_of(&case[0])?)?;
        let spec = &case[1];
        let dir = spec.get(2).and_then(Json::as_str).unwrap_or(".");
        if spec[0] == "schema" && PHP_CALLABLE_NAMES.contains(&str_of(&spec[1])?) {
            continue;
        }
        std::env::set_current_dir(base.join(dir))?;
        let cwd_url = format!("{}/", std::env::current_dir()?.display());
        let case_paths = Paths {
            base: base_string.clone(),
            cwd_url,
        };
        let result = run_validation(&case_paths, &base, spec, &mut |v| {
            XmlUtils::parse(&content, v)
        })?;
        std::env::set_current_dir(&base)?;

        if spec[0] == "schema" {
            let schema = str_of(&spec[1])?;
            if let Some((_, _, text)) = REFUSED.iter().find(|(s, d, _)| *s == schema && *d == dir) {
                refused_seen += 1;
                match &result {
                    Err(e)
                        if *e.class() == Class::XmlParsing
                            && String::from_utf8_lossy(e.get_message()).starts_with(text) => {}
                    other => failures.push(format!(
                        "{schema} in {dir}: expected a refusal, got {}",
                        describe(other)
                    )),
                }
                continue;
            }
        }

        let expected = expected_outcome(&case_paths, &case[2]);
        let actual = describe(&result);
        if expected != actual {
            failures.push(format!(
                "parse({:?}, {spec}):\n  expected {expected}\n  actual   {actual}",
                String::from_utf8_lossy(&content)
            ));
        }
    }
    assert_eq!(
        REFUSED.len(),
        refused_seen,
        "every listed refusal is recorded"
    );

    // the internal-errors setting a callable receives, and restoration
    for flag in data["flags"].as_array().ok_or("flags")? {
        let setting = flag[0].as_bool().ok_or("setting")?;
        let before = libxml_use_internal_errors(Some(setting));
        let mut seen = None;
        let mut f = |_: &Document, internal: bool, _: &mut Vec<LibxmlError>| {
            seen = Some(internal);
            Ok(Value::Bool(true))
        };
        XmlUtils::parse(b"<a/>", Some(Validation::Callable(&mut f)))?;
        let after = libxml_use_internal_errors(Some(before));
        assert_eq!(
            flag[2].as_bool(),
            seen,
            "flag passed with setting {setting}"
        );
        assert_eq!(
            flag[3].as_bool(),
            Some(after),
            "setting restored after parse()"
        );
    }

    // parse('') throws loadXML()'s ValueError and leaves the setting on
    libxml_use_internal_errors(Some(false));
    let empty = describe(&XmlUtils::parse(b"", None));
    let leaked = libxml_use_internal_errors(Some(false));
    assert_eq!(expected_outcome(&paths, &data["empty"][0]), empty);
    assert_eq!(data["empty"][1].as_bool(), Some(leaked));

    // loadFile()
    for case in data["load"].as_array().ok_or("load")? {
        // the path as the recorder passed it: "$base/$file", or relative
        let file = str_of(&case[0])?;
        let path = PathBuf::from(file.replace(RECORDED_BASE, &base_string));
        let result = run_validation(&paths, &base, &case[1], &mut |v| {
            XmlUtils::load_file(&path, v)
        })?;
        let expected = expected_outcome(&paths, &case[2]);
        let actual = describe(&result);
        if expected != actual {
            failures.push(format!(
                "loadFile({file}, {}):\n  expected {expected}\n  actual   {actual}",
                case[1]
            ));
        }
    }

    std::env::set_current_dir(start)?;
    let _ = std::fs::remove_dir_all(&base);

    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}
