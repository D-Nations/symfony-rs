//! Cookie against PHP 8.4.27's answers for what CookieTest leaves open
//! (tests/data/cookie.json, oracle/recorders/http_foundation_cookie.php):
//! 2,112 `fromString()` calls and 165 constructions on a clock fixed at
//! the recorded time. Every getter, the `Set-Cookie` text and every
//! exception's class and message must match. A text the port refuses
//! (`NotPorted`, a date it does not read) must be listed in `REFUSED`.
//!
//! Upstream: symfony/http-foundation Cookie.php

use php_value::{PhpString, base64, clock};
use serde_json::Value as Json;
use symfony_http_foundation::{
    cookie::{Cookie, Expire, Options},
    exception::{Class, Result},
};

fn bytes(field: &Json) -> Option<Vec<u8>> {
    field
        .as_str()
        .map(|text| base64::decode_strict(text.as_bytes()).unwrap())
}

fn string(field: &Json) -> Option<PhpString> {
    bytes(field).map(PhpString::from)
}

/// Expiry texts the port refuses (none so far).
const REFUSED: &[&str] = &[];

fn check(label: &str, outcome: Result<Cookie>, expected: &Json, refused: &mut Vec<String>) {
    match (outcome, expected[0].as_str().unwrap()) {
        (Ok(cookie), "cookie") => {
            assert_eq!(
                Some(cookie.name().as_bytes().to_vec()),
                bytes(&expected[1]),
                "{label}: name"
            );
            assert_eq!(
                cookie.value().map(|v| v.as_bytes().to_vec()),
                bytes(&expected[2]),
                "{label}: value"
            );
            assert_eq!(
                cookie.expires_time(),
                expected[3].as_i64().unwrap(),
                "{label}: expires"
            );
            assert_eq!(
                Some(cookie.path().as_bytes().to_vec()),
                bytes(&expected[4]),
                "{label}: path"
            );
            assert_eq!(
                cookie.domain().map(|v| v.as_bytes().to_vec()),
                bytes(&expected[5]),
                "{label}: domain"
            );
            assert_eq!(cookie.is_secure(), expected[6], "{label}: secure");
            assert_eq!(cookie.is_http_only(), expected[7], "{label}: httponly");
            assert_eq!(cookie.is_raw(), expected[8], "{label}: raw");
            assert_eq!(
                cookie.same_site().map(|s| s.as_str()),
                expected[9].as_str(),
                "{label}: samesite"
            );
            assert_eq!(
                cookie.is_partitioned(),
                expected[10],
                "{label}: partitioned"
            );
            assert_eq!(
                cookie.max_age(),
                expected[11].as_i64().unwrap(),
                "{label}: max-age"
            );
            assert_eq!(cookie.is_cleared(), expected[12], "{label}: cleared");
            assert_eq!(
                Some(cookie.to_php_string().as_bytes().to_vec()),
                bytes(&expected[13]),
                "{label}: string"
            );
        }
        (Err(error), "throws") if error.class() != Class::NotPorted => {
            assert_eq!(error.class().name(), expected[1], "{label}: class");
            assert_eq!(
                Some(error.message().as_bytes().to_vec()),
                bytes(&expected[2]),
                "{label}: message"
            );
        }
        (Err(error), _) if error.class() == Class::NotPorted => refused.push(label.to_owned()),
        (outcome, _) => panic!("{label}: {outcome:?}, PHP: {expected}"),
    }
}

#[test]
fn recorded_cookies() {
    let data: Json = serde_json::from_str(include_str!("data/cookie.json")).unwrap();
    let _clock = clock::mock(data["now"].as_i64().unwrap());
    let mut refused = Vec::new();
    for case in data["from_string"].as_array().unwrap() {
        let text = bytes(&case[0]).unwrap();
        let decode = case[1].as_bool().unwrap();
        let label = format!("fromString({:?}, {decode})", String::from_utf8_lossy(&text));
        check(
            &label,
            Cookie::from_string(&text, decode),
            &case[2],
            &mut refused,
        );
    }
    for case in data["construct"].as_array().unwrap() {
        let args = &case[0];
        let mut options = Options {
            value: Some(PhpString::from("bar")),
            ..Options::default()
        };
        let mut name = PhpString::from("foo");
        match args[0].as_str().unwrap() {
            "name" => {
                name = string(&args[1]).unwrap();
                options.value = string(&args[2]);
                options.raw = args[3].as_bool().unwrap();
            }
            "expire" => {
                options.expire = match args[1].as_i64() {
                    Some(timestamp) => Expire::Timestamp(timestamp),
                    None => Expire::Text(string(&args[1]).unwrap()),
                };
            }
            "path" => {
                options.path = string(&args[1]);
                options.domain = string(&args[2]);
            }
            "samesite" => {
                options.same_site = string(&args[1]);
                options.secure = args[2].as_bool();
                options.http_only = false;
                options.partitioned = true;
            }
            other => panic!("unknown case {other}"),
        }
        let label = format!("new Cookie {args}");
        check(&label, Cookie::new(name, options), &case[1], &mut refused);
    }
    assert_eq!(refused, REFUSED);
}
