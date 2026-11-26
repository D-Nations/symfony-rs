//! `JsonResponse` and `RedirectResponse` against PHP 8.4.27 running the
//! pinned symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_json_redirect.php`
//! (`scripts/record.sh http-foundation-json-redirect`): 1,500 generated
//! `JsonResponse` scripts (each constructor, then `setData()`,
//! `setJson()`, `setCallback()`, `setEncodingOptions()`, Content-Type and
//! `setContent()` calls, with every call's result and the final status,
//! content, encoding options and headers); 3,255 `setCallback()` names;
//! and 1,000 `RedirectResponse` constructions, some followed by
//! `setTargetUrl()`. Headers are compared without Date. An exception is
//! compared by class, message and code.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::Error,
    header_bag::HeaderValues,
    json_response::{JsonResponse, JsonResponseError},
    redirect_response::RedirectResponse,
    response::Response,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/json_redirect.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<PhpString, TestError> {
    Ok(PhpString::from(vectors::base64(text(json)?)?))
}

fn nullable_bytes(json: &Json) -> Result<Option<PhpString>, TestError> {
    match json {
        Json::Null => Ok(None),
        other => Ok(Some(bytes(other)?)),
    }
}

fn int(json: &Json) -> Result<i64, TestError> {
    json.as_i64().ok_or_else(|| "expected an int".into())
}

/// A recorded headers array, as `(name, values)` pairs.
fn headers(json: &Json) -> Result<Vec<(Vec<u8>, HeaderValues)>, TestError> {
    let Value::Array(array) = vectors::decode(json)? else {
        return Err("expected the headers array".into());
    };
    let mut pairs = Vec::new();
    for (key, value) in array.iter() {
        let name = key.to_value().to_php_string().unwrap_or_default();
        let values = match value {
            Value::Null => HeaderValues::One(None),
            Value::String(s) => HeaderValues::One(Some(s.clone())),
            Value::Array(a) => {
                HeaderValues::Many(a.iter().map(|(_, v)| v.as_string().cloned()).collect())
            }
            other => return Err(format!("unexpected header value {other:?}").into()),
        };
        pairs.push((name.as_bytes().to_vec(), values));
    }
    Ok(pairs)
}

/// `allPreserveCase()` without Date, as a PHP array.
fn headers_value(response: &Response) -> Value {
    let mut headers = PhpArray::new();
    for (name, values) in response.headers().all_preserve_case().iter() {
        if name.bytes().as_ref() == b"Date" {
            continue;
        }
        headers.insert(
            name.clone(),
            Value::Array(PhpArray::list(
                values
                    .iter()
                    .map(|v| v.clone().map_or(Value::Null, Value::String)),
            )),
        );
    }
    Value::Array(headers)
}

/// Whether an exception is the recorded `["throws", class, message,
/// code, previous]`.
fn same_exception(recorded: &Json, error: &Error) -> bool {
    recorded[0] == "throws"
        && recorded[1].as_str() == Some(error.class().name())
        && recorded[2].as_str().map(str::as_bytes) == Some(error.message().as_bytes())
        && recorded[3].as_i64() == Some(error.code())
        && recorded[4].is_null()
}

fn same_json_error(recorded: &Json, error: &JsonResponseError) -> bool {
    match error {
        JsonResponseError::Exception(e) => same_exception(recorded, e),
        // no recorded value is JsonSerializable
        JsonResponseError::Thrown(_) | JsonResponseError::JsonWithPrevious { .. } => false,
    }
}

/// A call's result: `$this`, or what it threw.
fn same_result<T>(recorded: &Json, got: &Result<T, JsonResponseError>) -> bool {
    match got {
        Ok(_) => recorded[0] == "self",
        Err(e) => same_json_error(recorded, e),
    }
}

fn json_scripts(data: &Json, failures: &mut Vec<String>) -> Result<usize, TestError> {
    let scripts = data["json"].as_array().ok_or("json")?;
    assert_eq!(1500, scripts.len());
    let mut answers = 0;
    for (n, script) in scripts.iter().enumerate() {
        let new = &script["new"];
        let value = vectors::decode(&new[1])?;
        let (status, headers) = (int(&new[2])?, headers(&new[3])?);
        let created = match text(&new[0])? {
            "from" => match value {
                Value::String(s) => JsonResponse::from_json_string(s, status, headers),
                other => return Err(format!("fromJsonString({other:?})").into()),
            },
            kind => JsonResponse::new(value, status, headers, kind == "json"),
        };
        answers += 1;
        let mut response = match (created, script.get("throws")) {
            (Err(e), Some(throws)) => {
                if !same_json_error(throws, &e) {
                    failures.push(format!("json {n}: new: expected {throws}, got {e:?}"));
                }
                continue;
            }
            (Ok(_), Some(throws)) => {
                failures.push(format!("json {n}: new: expected {throws}"));
                continue;
            }
            (Err(e), None) => {
                failures.push(format!("json {n}: new: {e:?}"));
                continue;
            }
            (Ok(response), None) => response,
        };
        let ops = script["ops"].as_array().ok_or("ops")?;
        let results = script["results"].as_array().ok_or("results")?;
        for (i, (op, result)) in ops.iter().zip(results).enumerate() {
            answers += 1;
            let same = match text(&op[0])? {
                "data" => same_result(result, &response.set_data(&vectors::decode(&op[1])?)),
                "json" => {
                    response.set_json(bytes(&op[1])?);
                    result[0] == "self"
                }
                "callback" => {
                    let callback = nullable_bytes(&op[1])?;
                    same_result(
                        result,
                        &response
                            .set_callback(callback.as_ref().map(php_value::PhpString::as_bytes)),
                    )
                }
                "options" => same_result(result, &response.set_encoding_options(int(&op[1])?)),
                "content_type" => {
                    let value = HeaderValues::One(nullable_bytes(&op[1])?);
                    match response.headers_mut().set(b"Content-Type", value, true) {
                        Ok(()) => result[0] == "null",
                        Err(e) => same_exception(result, &e),
                    }
                }
                "content" => {
                    response.set_content(Some(bytes(&op[1])?))?;
                    result[0] == "self"
                }
                other => return Err(format!("unknown op {other}").into()),
            };
            if !same {
                failures.push(format!("json {n} op {i} {op}: expected {result}"));
            }
        }
        let state = &script["state"];
        answers += 1;
        let expected_headers = vectors::decode(&state[3])?;
        if int(&state[0])? != response.get_status_code()
            || bytes(&state[1])? != *response.get_content()
            || int(&state[2])? != response.get_encoding_options()
            || !expected_headers.identical(&headers_value(&response))
        {
            failures.push(format!(
                "json {n}: state: expected {state}, got {} {:?} {} {:?}",
                response.get_status_code(),
                response.get_content(),
                response.get_encoding_options(),
                headers_value(&response)
            ));
        }
    }
    Ok(answers)
}

fn callbacks(data: &Json, failures: &mut Vec<String>) -> Result<usize, TestError> {
    let cases = data["callbacks"].as_array().ok_or("callbacks")?;
    assert_eq!(3255, cases.len());
    let mut data_value = PhpArray::new();
    data_value.insert("a", Value::Int(1));
    let data_value = Value::Array(data_value);
    for case in cases {
        let name = bytes(&case[0])?;
        let mut response = JsonResponse::new(data_value.clone(), 200, [("", ""); 0], false)?;
        let got = response
            .set_callback(Some(name.as_bytes()))
            .map(|r| r.get_content().clone());
        let result = &case[1];
        let same = match &got {
            Ok(content) => result[0] == "ok" && bytes(&result[1])? == *content,
            Err(e) => same_json_error(result, e),
        };
        if !same {
            failures.push(format!(
                "callback {:?}: expected {result}, got {got:?}",
                String::from_utf8_lossy(name.as_bytes())
            ));
        }
    }
    Ok(cases.len())
}

fn redirects(data: &Json, failures: &mut Vec<String>) -> Result<usize, TestError> {
    let cases = data["redirect"].as_array().ok_or("redirect")?;
    assert_eq!(1000, cases.len());
    for (n, case) in cases.iter().enumerate() {
        let (url, status, headers) = (bytes(&case[0])?, int(&case[1])?, headers(&case[2])?);
        let then = nullable_bytes(&case[3])?;
        let expected = &case[4];
        let mut response = match RedirectResponse::new(url, status, headers) {
            Ok(response) => response,
            Err(e) => {
                if !same_exception(expected, &e) {
                    failures.push(format!("redirect {n}: expected {expected}, got {e:?}"));
                }
                continue;
            }
        };
        if expected[0] != "ok" {
            failures.push(format!("redirect {n}: expected {expected}"));
            continue;
        }
        let after_ok = match then {
            None => expected[1].is_null(),
            Some(then) => match response.set_target_url(then) {
                Ok(_) => expected[1][0] == "self",
                Err(e) => same_exception(&expected[1], &e),
            },
        };
        let expected_headers = vectors::decode(&expected[5])?;
        if !after_ok
            || bytes(&expected[2])? != *response.get_target_url()
            || bytes(&expected[3])? != *response.get_content()
            || int(&expected[4])? != response.get_status_code()
            || !expected_headers.identical(&headers_value(&response))
        {
            failures.push(format!(
                "redirect {n} {case}: got {:?} {:?} {} {:?}",
                response.get_target_url(),
                response.get_content(),
                response.get_status_code(),
                headers_value(&response)
            ));
        }
    }
    Ok(cases.len())
}

#[test]
fn json_and_redirect_responses_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let mut failures = Vec::new();
    let answers = json_scripts(&data, &mut failures)?
        + callbacks(&data, &mut failures)?
        + redirects(&data, &mut failures)?;
    assert!(
        failures.is_empty(),
        "{} of {answers} answers differ:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    eprintln!("{answers} answers compared");
    Ok(())
}
