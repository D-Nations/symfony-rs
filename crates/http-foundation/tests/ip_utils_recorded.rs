//! IpUtils and the PHP built-ins it calls, against PHP 8.4.27's answers
//! recorded by oracle/recorders/http_foundation_ip_utils.php
//! (`scripts/record.sh http-foundation`) with symfony/http-foundation
//! v8.1.11: `filter_var(FILTER_VALIDATE_IP)`, `inet_pton()`, `ip2long()`
//! and `inet_ntop()` over generated strings, then `checkIp4()`,
//! `checkIp6()`, `checkIp()`, `isPrivateIp()` and `anonymize()` over
//! requests x subnets, odd netmasks included. The calls run in PHP's
//! order in one thread, so the results cache sees the same keys.
//!
//! Upstream: none

use php_value::net;
use php_value::vectors::{Error, base64};
use serde_json::{Value as Json, json};
use symfony_http_foundation::exception::Error as PhpError;
use symfony_http_foundation::ip_utils::{anonymize, check_ip, check_ip4, check_ip6, is_private_ip};

fn data() -> Result<Json, Error> {
    Ok(serde_json::from_str(include_str!("data/ip_utils.json"))?)
}

fn text(json: &Json) -> Result<String, Error> {
    let bytes = base64(json.as_str().ok_or("expected base64")?)?;
    Ok(String::from_utf8(bytes).map_err(|_| "a recorded string is not UTF-8")?)
}

/// A Rust answer in the recording's form: the value, or ["throw", class,
/// message].
fn answer<T: Into<Json>>(result: Result<T, PhpError>) -> Json {
    match result {
        Ok(value) => value.into(),
        Err(e) => json!(["throw", e.class().name(), e.message().to_string_lossy()]),
    }
}

fn report(section: &str, failures: &[String], total: usize) {
    assert!(
        failures.is_empty(),
        "{section}: {} of {total} differ:\n{}",
        failures.len(),
        failures[..failures.len().min(30)].join("\n")
    );
}

#[test]
fn builtins_match_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["builtins"].as_array().ok_or("builtins")?;
    let mut failures = Vec::new();
    for case in cases {
        let s = text(&case[0])?;
        let b = s.as_bytes();
        let pton = match net::inet_pton(b) {
            Ok(Some(packed)) => json!(
                packed
                    .iter()
                    .map(|x| format!("{x:02x}"))
                    .collect::<String>()
            ),
            Ok(None) => json!(false),
            Err(e) => json!(["throw", "ValueError", e.to_string()]),
        };
        let long = match net::ip2long(b) {
            Ok(Some(long)) => json!(long),
            Ok(None) => json!(false),
            Err(e) => json!(["throw", "ValueError", e.to_string()]),
        };
        let actual = json!([
            case[0],
            net::validate_ip(b, true, false),
            net::validate_ip(b, false, true),
            net::validate_ip(b, false, false),
            pton,
            long,
        ]);
        if &actual != case {
            failures.push(format!("{s:?}: PHP {case}, Rust {actual}"));
        }
    }
    report("builtins", &failures, cases.len());
    Ok(())
}

#[test]
fn inet_ntop_matches_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["ntop"].as_array().ok_or("ntop")?;
    let mut failures = Vec::new();
    for case in cases {
        let hex = case[0].as_str().ok_or("hex")?;
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<_, _>>()
            .map_err(|_| "bad hex")?;
        let actual = net::inet_ntop(&bytes).map_or(json!(false), |s| json!(s));
        if actual != case[1] {
            failures.push(format!("{hex}: PHP {}, Rust {actual}", case[1]));
        }
    }
    report("ntop", &failures, cases.len());
    Ok(())
}

/// Every IpUtils section, in the order PHP ran them.
#[test]
fn ip_utils_matches_php() -> Result<(), Error> {
    let data = data()?;
    let lists = data["lists"].as_object().ok_or("lists")?;
    let mut failures = Vec::new();
    let mut total = 0;
    for section in ["check4", "check6"] {
        for case in data[section].as_array().ok_or("section")? {
            let (request, subnet) = (text(&case[0])?, text(&case[1])?);
            let actual = answer(if section == "check4" {
                check_ip4(&request, &subnet)
            } else {
                check_ip6(&request, &subnet)
            });
            total += 1;
            if actual != case[2] {
                failures.push(format!(
                    "{section} {request:?} {subnet:?}: PHP {}, Rust {actual}",
                    case[2]
                ));
            }
        }
    }
    for case in data["check"].as_array().ok_or("check")? {
        let request = text(&case[0])?;
        let name = case[1].as_str().ok_or("list name")?;
        let list: Vec<&str> = lists[name]
            .as_array()
            .ok_or("list")?
            .iter()
            .filter_map(Json::as_str)
            .collect();
        let actual = answer(check_ip(&request, &list));
        total += 1;
        if actual != case[2] {
            failures.push(format!(
                "check {request:?} {name}: PHP {}, Rust {actual}",
                case[2]
            ));
        }
    }
    for case in data["private"].as_array().ok_or("private")? {
        let request = text(&case[0])?;
        let actual = answer(is_private_ip(&request));
        total += 1;
        if actual != case[1] {
            failures.push(format!(
                "private {request:?}: PHP {}, Rust {actual}",
                case[1]
            ));
        }
    }
    for case in data["anonymize"].as_array().ok_or("anonymize")? {
        let ip = text(&case[0])?;
        let (v4, v6) = (case[1].as_i64().ok_or("v4")?, case[2].as_i64().ok_or("v6")?);
        let actual = answer(anonymize(&ip, v4, v6));
        total += 1;
        if actual != case[3] {
            failures.push(format!(
                "anonymize {ip:?} {v4} {v6}: PHP {}, Rust {actual}",
                case[3]
            ));
        }
    }
    report("IpUtils", &failures, total);
    Ok(())
}
