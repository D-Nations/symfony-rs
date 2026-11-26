//! `sapi` against PHP-FPM 8.4.27 (the oracle's FPM image, no php.ini),
//! recorded by `oracle/recorders/runtime_fpm.php` (`scripts/record.sh
//! runtime-fpm`): `FastCGI` requests a web server would send for a front
//! controller `index.php` (methods, query strings, cookies, headers,
//! `Authorization`, forms, multipart bodies with files of every outcome,
//! bodies past `post_max_size`), with what the script saw (`$_SERVER`,
//! `$_GET`, `$_POST`, `$_COOKIE`, `$_REQUEST`, `$_FILES` and each uploaded
//! file, `php://input`, `request_parse_body()`); and scripts of output
//! calls (`header()`, `header_remove()`, `http_response_code()`, `echo`,
//! `ini_set()`, `setcookie()`, output buffers, `fastcgi_finish_request()`),
//! named and generated, for GET and HEAD, with their answers, warnings,
//! `headers_list()` and the CGI response PHP-FPM sent, byte for byte.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::path::PathBuf;

use php_value::{Key, PhpArray, PhpString, Value, path::from_path, vectors};
use serde_json::Value as Json;
use symfony_http_foundation::{
    php::is_uploaded_file,
    sapi::{self, CgiRequest, Ini, Sapi, SetCookie},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/sapi.json");

fn b64(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(json.as_str().ok_or("expected base64")?)
}

/// Bytes as base64, or `[before, byte, count, after]`.
fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    match json {
        Json::String(_) => b64(json),
        Json::Array(parts) if parts.len() == 4 => {
            let mut out = b64(&parts[0])?;
            let byte = u8::try_from(parts[1].as_u64().ok_or("byte")?)?;
            let count = usize::try_from(parts[2].as_u64().ok_or("count")?)?;
            out.extend(std::iter::repeat_n(byte, count));
            out.extend(b64(&parts[3])?);
            Ok(out)
        }
        other => Err(format!("not bytes: {other}").into()),
    }
}

/// The front controller's value encoding.
fn value(json: &Json) -> Result<Value, TestError> {
    Ok(match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().ok_or("number")?),
        },
        Json::String(s) => {
            let text = s.strip_prefix("s:").ok_or("string without s:")?;
            Value::String(PhpString::from(vectors::base64(text)?))
        }
        Json::Object(o) => Value::String(PhpString::from(bytes(&o["long"])?)),
        Json::Array(pairs) => {
            let mut array = PhpArray::new();
            for pair in pairs {
                let key = match &pair[0] {
                    Json::Number(n) => Key::Int(n.as_i64().ok_or("key")?),
                    Json::String(s) => {
                        Key::from_bytes(&vectors::base64(s.strip_prefix("s:").ok_or("key")?)?)
                    }
                    other => return Err(format!("key {other}").into()),
                };
                array.insert(key, value(&pair[1])?);
            }
            Value::Array(array)
        }
    })
}

fn array(json: &Json) -> Result<PhpArray, TestError> {
    match value(json)? {
        Value::Array(a) => Ok(a),
        other => Err(format!("not an array: {other:?}").into()),
    }
}

/// `===` on arrays, order included.
fn identical(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && identical(va, vb))
        }
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

/// Uploaded files as the front controller records them: each `tmp_name`
/// replaced by `#n`, the file's content kept.
fn mark_files(value: &mut Value, dir: &[u8], files: &mut Vec<(bool, Vec<u8>)>) {
    match value {
        Value::Array(array) => {
            let keys: Vec<Key> = array.keys().cloned().collect();
            for key in keys {
                if let Some(v) = array.get_mut(&key) {
                    mark_files(v, dir, files);
                }
            }
        }
        Value::String(s) if s.as_bytes().starts_with(dir) => {
            let path = php_value::path::to_path(s.as_bytes()).unwrap();
            let content = std::fs::read(&path).unwrap_or_default();
            files.push((is_uploaded_file(s.as_bytes()), content));
            *s = PhpString::from(format!("#{}", files.len() - 1));
        }
        _ => {}
    }
}

fn upload_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapi-recorded-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The environment the recording's PHP-FPM passed on: what precedes the
/// first parameter in the first case's `$_SERVER`.
fn environment(case: &Json) -> Result<Vec<(PhpString, PhpString)>, TestError> {
    let params = params(case)?;
    let server = array(&case["record"]["server"])?;
    let mut env = Vec::new();
    for (key, v) in server.iter() {
        let name = PhpString::from(key.bytes().into_owned());
        if params.iter().any(|(n, _)| *n == name) {
            break;
        }
        if let Value::String(s) = v {
            env.push((name, s.clone()));
        }
    }
    Ok(env)
}

fn params(case: &Json) -> Result<Vec<(PhpString, PhpString)>, TestError> {
    case["params"]
        .as_array()
        .ok_or("params")?
        .iter()
        .map(|p| Ok((PhpString::from(b64(&p[0])?), PhpString::from(b64(&p[1])?))))
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one replay loop over the recorded cases"
)]
fn builds_the_superglobals_php_fpm_builds() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = data["requests"].as_array().ok_or("requests")?;
    assert!(cases.len() >= 100, "{} request cases", cases.len());
    let env = environment(&cases[0])?;
    let dir = upload_dir();
    let dir_bytes = from_path(&dir);
    let ini = Ini {
        upload_tmp_dir: dir.clone(),
        x_powered_by: Some(PhpString::from("PHP/8.4.27")),
        ..Ini::default()
    };
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let record = &case["record"];
        let request = CgiRequest {
            environment: env.clone(),
            params: params(case)?,
            stdin: bytes(&case["stdin"])?,
            time: 0.0,
        };
        let started = sapi::start(&request, &ini)?;
        let mut g = started.globals;
        g.server.insert("REQUEST_TIME_FLOAT", Value::Int(0));
        let mut tmp = Vec::new();
        let mut files = Value::Array(g.files.clone());
        mark_files(&mut files, &dir_bytes, &mut tmp);
        let parsed = g.parsed_body.clone().map(|(post, files)| {
            let mut files = Value::Array(files);
            mark_files(&mut files, &dir_bytes, &mut tmp);
            (post, files)
        });
        for (what, got, expected) in [
            ("server", Value::Array(g.server.clone()), &record["server"]),
            ("get", Value::Array(g.get.clone()), &record["get"]),
            ("post", Value::Array(g.post.clone()), &record["post"]),
            ("cookie", Value::Array(g.cookie.clone()), &record["cookie"]),
            (
                "request",
                Value::Array(g.request.clone()),
                &record["request"],
            ),
            ("files", files, &record["files"]),
        ] {
            let expected = value(expected)?;
            if !identical(&got, &expected) {
                failures.push(format!(
                    "{name} {what}:\n  php  {expected:?}\n  rust {got:?}"
                ));
            }
        }
        if g.input.as_ref() != bytes(&record["input"])?.as_slice() {
            failures.push(format!("{name}: php://input differs"));
        }
        match (&record["parsed_body"], parsed) {
            (Json::Null, None) => {}
            (Json::Array(a), None) if a.first().and_then(Json::as_str) == Some("throws") => {}
            (Json::Array(a), Some((post, files))) if a.len() == 2 => {
                if !identical(&Value::Array(post.clone()), &value(&a[0])?)
                    || !identical(&files, &value(&a[1])?)
                {
                    failures.push(format!(
                        "{name} request_parse_body():\n  php  {a:?}\n  rust {post:?} {files:?}"
                    ));
                }
            }
            (expected, got) => failures.push(format!(
                "{name} request_parse_body(): php {expected}, rust {got:?}"
            )),
        }
        let expected_tmp = record["tmp"].as_array().ok_or("tmp")?;
        if expected_tmp.len() == tmp.len() {
            for (i, (uploaded, content)) in tmp.iter().enumerate() {
                if !uploaded || *content != bytes(&expected_tmp[i]["content"])? {
                    failures.push(format!("{name}: uploaded file #{i} differs"));
                }
            }
        } else {
            failures.push(format!(
                "{name}: {} uploaded files, php {}",
                tmp.len(),
                expected_tmp.len()
            ));
        }
        // The default response of the front controller.
        let head = matches!(g.server.get(&Key::from("REQUEST_METHOD")), Some(Value::String(m)) if m.as_bytes() == b"HEAD");
        let mut out = Sapi::new(&ini, head);
        let got = out.end().to_bytes();
        if got != b64(&case["stdout"])? {
            failures.push(format!(
                "{name}: response {:?}",
                String::from_utf8_lossy(&got)
            ));
        }
        sapi::finish(&started.uploaded);
        for path in &started.uploaded {
            if path.exists() {
                failures.push(format!("{name}: {} was not removed", path.display()));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}

fn answer_list(list: &[PhpString]) -> Json {
    Json::Array(
        list.iter()
            .map(|l| Json::String(php_value::base64::encode(l.as_bytes())))
            .collect(),
    )
}

/// Runs one recorded script; `None` when a call threw (PHP's fatal error).
fn run(sapi: &mut Sapi, script: &[Json], answers: &mut Vec<Json>) -> Result<(), TestError> {
    for call in script {
        if sapi.is_aborted() {
            return Ok(());
        }
        let op = call[0].as_str().ok_or("op")?;
        let arg = |i: usize| -> Result<Vec<u8>, TestError> { b64(&call[i]) };
        match op {
            "header" => sapi.header(
                &arg(1)?,
                call[2].as_bool().ok_or("replace")?,
                call[3].as_i64().ok_or("code")?,
            ),
            "header_remove" => match &call[1] {
                Json::Null => sapi.header_remove(None),
                _ => sapi.header_remove(Some(&arg(1)?)),
            },
            "http_response_code" => {
                let previous = sapi.set_response_code(call[1].as_i64().ok_or("code")?);
                answers.push(match previous {
                    Value::Int(i) => Json::from(i),
                    Value::Bool(b) => Json::from(b),
                    _ => Json::Null,
                });
            }
            "echo" => sapi.echo(&arg(1)?),
            "ini_set" => {
                let value = PhpString::from(arg(2)?);
                let old = match arg(1)?.as_slice() {
                    b"default_mimetype" => sapi.set_default_mimetype(value),
                    _ => sapi.set_default_charset(value),
                };
                answers.push(Json::String(old.to_string_lossy().into_owned()));
            }
            "setcookie" | "setrawcookie" => {
                let cookie = SetCookie {
                    name: PhpString::from(arg(1)?),
                    value: PhpString::from(arg(2)?),
                    expires: call[3].as_i64().ok_or("expires")?,
                    path: PhpString::from(arg(4)?),
                    domain: PhpString::from(arg(5)?),
                    secure: call[6].as_bool().ok_or("secure")?,
                    httponly: call[7].as_bool().ok_or("httponly")?,
                    samesite: PhpString::default(),
                };
                if let Ok(done) = sapi.setcookie(&cookie, op == "setrawcookie") {
                    answers.push(Json::from(done));
                } else {
                    sapi.fatal_error();
                    return Ok(());
                }
            }
            "headers_list" => answers.push(answer_list(&sapi.headers_list())),
            "headers_sent" => answers.push(Json::from(sapi.headers_sent())),
            "response_code" => {
                answers.push(sapi.response_code().map_or(Json::from(false), Json::from));
            }
            "ob_start" => sapi.ob_start(),
            "ob_end_flush" => answers.push(Json::from(sapi.ob_end_flush())),
            "ob_end_clean" => answers.push(Json::from(sapi.ob_end_clean())),
            "flush" => sapi.flush(),
            "fastcgi_finish_request" => answers.push(Json::from(sapi.fastcgi_finish_request())),
            other => return Err(format!("unknown op {other}").into()),
        }
    }
    Ok(())
}

#[test]
fn sends_what_php_fpm_sends() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = data["scripts"].as_array().ok_or("scripts")?;
    assert!(cases.len() >= 900, "{} scripts", cases.len());
    let ini = Ini {
        x_powered_by: Some(PhpString::from("PHP/8.4.27")),
        ..Ini::default()
    };
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let record = &case["record"];
        let head = params(case)?
            .iter()
            .any(|(n, v)| n.as_bytes() == b"REQUEST_METHOD" && v.as_bytes() == b"HEAD");
        let mut sapi = Sapi::new(&ini, head);
        let mut answers = Vec::new();
        run(
            &mut sapi,
            case["script"].as_array().ok_or("script")?,
            &mut answers,
        )?;
        let final_list = answer_list(&sapi.headers_list());
        let final_code = sapi.response_code().map_or(Json::from(false), Json::from);
        let got = sapi.end().to_bytes();
        let expected = b64(&case["stdout"])?;
        if got != expected {
            failures.push(format!(
                "{name}: response\n  php  {:?}\n  rust {:?}",
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&got)
            ));
        }
        if Json::Array(answers.clone()) != record["answers"] {
            failures.push(format!(
                "{name}: answers\n  php  {}\n  rust {}",
                record["answers"],
                Json::Array(answers)
            ));
        }
        // headers_list() in the shutdown function, which runs before the
        // headers are sent at the end of the script.
        if final_list != record["final_list"] {
            failures.push(format!(
                "{name}: headers_list() {} php {}",
                final_list, record["final_list"]
            ));
        }
        if final_code != record["final_code"] {
            failures.push(format!(
                "{name}: http_response_code() {final_code} php {}",
                record["final_code"]
            ));
        }
        let expected_warnings: Vec<String> = record["warnings"]
            .as_array()
            .ok_or("warnings")?
            .iter()
            .map(|w| w[1].as_str().unwrap_or_default().to_owned())
            .collect();
        let matches = expected_warnings.len() == sapi.warnings().len()
            && expected_warnings
                .iter()
                .zip(sapi.warnings())
                .all(|(php, rust)| php.starts_with(rust.as_str()));
        if !matches {
            failures.push(format!(
                "{name}: warnings {:?} php {expected_warnings:?}",
                sapi.warnings()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    Ok(())
}
