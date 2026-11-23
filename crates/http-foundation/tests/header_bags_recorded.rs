//! HeaderBag and ResponseHeaderBag against PHP 8.4.27's answers for what
//! their tests leave open (tests/data/header_bags.json,
//! oracle/recorders/http_foundation_header_bags.php): 400 seeded scripts
//! of calls on a fixed clock, each compared on `all()`, `keys()`, the
//! string form, `count()`, the Cache-Control readers and, for responses,
//! `allPreserveCase()` with and without cookies. A TypeError's message
//! is compared up to PHP's ", called in FILE on line N".
//!
//! Upstream: symfony/http-foundation HeaderBag.php ResponseHeaderBag.php

use php_value::{Key, PhpString, Value, base64, clock};
use serde_json::Value as Json;
use symfony_http_foundation::{
    cookie::Cookie,
    exception::Result,
    header_bag::{ClearCookie, HeaderBag, HeaderValues, Ordered, ResponseHeaderBag, Values},
};

fn bytes(field: &Json) -> Option<Vec<u8>> {
    field
        .as_str()
        .map(|text| base64::decode_strict(text.as_bytes()).unwrap())
}

fn values(field: &Json) -> HeaderValues {
    match field {
        Json::Array(list) => {
            HeaderValues::Many(list.iter().map(|v| bytes(v).map(PhpString::from)).collect())
        }
        other => HeaderValues::One(bytes(other).map(PhpString::from)),
    }
}

/// The recorder's `[name => values]` array: a repeated name keeps its
/// first place and takes the last values, as PHP array keys do.
fn headers(list: &Json) -> Vec<(Vec<u8>, HeaderValues)> {
    let mut out: Vec<(Key, Vec<u8>, HeaderValues)> = Vec::new();
    for pair in list.as_array().unwrap() {
        let name = bytes(&pair[0]).unwrap();
        let k = Key::from_bytes(&name);
        match out.iter_mut().find(|(existing, _, _)| *existing == k) {
            Some(slot) => slot.2 = values(&pair[1]),
            None => out.push((k, name, values(&pair[1]))),
        }
    }
    out.into_iter().map(|(_, name, v)| (name, v)).collect()
}

fn key(field: &Json) -> Key {
    match field.as_i64() {
        Some(i) => Key::Int(i),
        None => Key::from_bytes(&bytes(field).unwrap()),
    }
}

fn pairs(list: &Json) -> Vec<(Key, Values)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let vals = pair[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| bytes(v).map(PhpString::from))
                .collect();
            (key(&pair[0]), vals)
        })
        .collect()
}

fn listed(map: &Ordered<Values>) -> Vec<(Key, Values)> {
    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

enum Bag {
    Plain(HeaderBag),
    Response(ResponseHeaderBag),
}

impl Bag {
    fn base(&mut self) -> &mut HeaderBag {
        match self {
            Self::Plain(bag) => bag,
            Self::Response(bag) => bag,
        }
    }

    fn response(&mut self) -> &mut ResponseHeaderBag {
        match self {
            Self::Response(bag) => bag,
            Self::Plain(_) => panic!("not a response"),
        }
    }
}

fn run(script: &Json) -> Result<(Bag, Vec<String>)> {
    let initial = headers(&script["initial"]);
    let mut bag = if script["response"].as_bool().unwrap() {
        Bag::Response(ResponseHeaderBag::new(initial)?)
    } else {
        Bag::Plain(HeaderBag::new(initial))
    };
    for op in script["ops"].as_array().unwrap() {
        match op[0].as_str().unwrap() {
            "set" => bag.base().set(
                &bytes(&op[1]).unwrap(),
                values(&op[2]),
                op[3].as_bool().unwrap(),
            )?,
            "remove" => bag
                .base()
                .remove(&String::from_utf8(bytes(&op[1]).unwrap()).unwrap()),
            "add" => {
                let value = match &op[2] {
                    Json::Bool(true) => Value::Bool(true),
                    other => Value::String(PhpString::from(bytes(other).unwrap())),
                };
                bag.base()
                    .add_cache_control_directive(op[1].as_str().unwrap(), value)?;
            }
            "del" => bag
                .base()
                .remove_cache_control_directive(op[1].as_str().unwrap())?,
            "replace" => bag.base().replace(headers(&op[1]))?,
            "cookie" => {
                let cookie = Cookie::from_string(&bytes(&op[1]).unwrap(), false)?;
                bag.response().set_cookie(cookie);
            }
            "clear" => bag.response().clear_cookie(
                op[1].as_str().unwrap(),
                Some("/"),
                None,
                ClearCookie::default(),
            )?,
            "rmcookie" => bag.response().remove_cookie(
                op[1].as_str().unwrap(),
                op[2].as_str(),
                op[3].as_str(),
            ),
            other => panic!("unknown op {other}"),
        }
    }
    Ok((bag, Vec::new()))
}

#[test]
fn recorded_header_bags() {
    let data: Json = serde_json::from_str(include_str!("data/header_bags.json")).unwrap();
    let _clock = clock::mock(data["now"].as_i64().unwrap());
    let directives: Vec<&str> = data["directives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap())
        .collect();
    for (n, script) in data["scripts"].as_array().unwrap().iter().enumerate() {
        let expected = &script["result"];
        let label = format!("script {n}: {}", script["ops"]);
        match (run(script), expected.get("throws")) {
            (Err(error), Some(throws)) => {
                assert_eq!(error.class().name(), throws[0], "{label}");
                let message = bytes(&throws[1]).unwrap();
                let message = match message.windows(11).position(|w| w == b", called in") {
                    Some(at) => message[..at].to_vec(),
                    None => message,
                };
                assert_eq!(error.message().as_bytes(), message.as_slice(), "{label}");
            }
            (Ok((mut bag, _)), None) => {
                let base = bag.base().clone();
                assert_eq!(listed(&base.all()), pairs(&expected["all"]), "{label}: all");
                let keys: Vec<Key> = expected["keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(key)
                    .collect();
                assert_eq!(base.keys(), keys, "{label}: keys");
                assert_eq!(
                    Some(base.to_php_string().as_bytes().to_vec()),
                    bytes(&expected["string"]),
                    "{label}: string"
                );
                assert_eq!(
                    base.count() as i64,
                    expected["count"].as_i64().unwrap(),
                    "{label}: count"
                );
                for (d, answer) in directives
                    .iter()
                    .zip(expected["directives"].as_array().unwrap())
                {
                    assert_eq!(
                        base.has_cache_control_directive(d),
                        answer[0],
                        "{label}: has {d}"
                    );
                    let value = match &answer[1] {
                        Json::Null => Value::Null,
                        Json::Bool(b) => Value::Bool(*b),
                        other => Value::String(PhpString::from(bytes(other).unwrap())),
                    };
                    assert_eq!(
                        base.get_cache_control_directive(d),
                        value,
                        "{label}: get {d}"
                    );
                }
                if let Bag::Response(response) = &bag {
                    assert_eq!(
                        listed(&response.all_preserve_case()),
                        pairs(&expected["preserve"]),
                        "{label}: preserve"
                    );
                    assert_eq!(
                        listed(&response.all_preserve_case_without_cookies()),
                        pairs(&expected["without_cookies"]),
                        "{label}: without cookies"
                    );
                }
            }
            (outcome, _) => panic!("{label}: {:?}, PHP: {expected}", outcome.map(|_| "a bag")),
        }
    }
}
