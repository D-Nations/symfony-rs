//! `ServerBag::getHeaders()` against PHP 8.4.27's answers for what
//! `ServerBagTest` leaves open (`tests/data/server_bag.json`,
//! `oracle/recorders/http_foundation_server_bag.php`): 360 `$_SERVER`
//! arrays, compared on the headers (in order, with `===`), any `TypeError`,
//! and the bag's parameters afterwards.
//!
//! Upstream: symfony/http-foundation ServerBag.php

use php_value::{
    Value,
    vectors::{Outcome, decode, outcome},
};
use serde_json::Value as Json;
use symfony_http_foundation::server_bag::ServerBag;

#[test]
fn recorded_server_bags() {
    let data: Json = serde_json::from_str(include_str!("data/server_bag.json")).unwrap();
    for case in data.as_array().unwrap() {
        let Value::Array(server) = decode(&case[0]).unwrap() else {
            panic!("not an array: {}", case[0]);
        };
        let mut bag = ServerBag::new(server.clone());
        let answer = bag.get_headers();
        match (answer, outcome(&case[1]).unwrap()) {
            (Ok(headers), Outcome::Value(expected)) => {
                assert_eq!(Value::Array(headers), expected, "{server:?}");
            }
            (Err(error), Outcome::Throws { class, message }) => {
                assert_eq!(error.class().name(), class, "{server:?}");
                assert_eq!(error.message().to_string_lossy(), message, "{server:?}");
            }
            (answer, expected) => panic!("{server:?}: {answer:?}, PHP: {expected:?}"),
        }
        assert_eq!(
            Value::Array(bag.all().clone()),
            decode(&case[2]).unwrap(),
            "{server:?}: parameters"
        );
    }
}
