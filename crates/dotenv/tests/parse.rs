//! `Dotenv::parse()` against PHP on generated inputs
//! (`oracle/recorders/dotenv_parse.php`, `scripts/record.sh dotenv`): the
//! same values, or the same exception class and message.
//!
//! Upstream: none
#![allow(non_snake_case)]

use php_value::{
    Value,
    vectors::{base64, decode},
};
use serde_json::Value as Json;
use symfony_dotenv::{Dotenv, Globals};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// The globals the recorder set.
fn globals() -> Globals {
    let mut g = Globals::new();
    g.putenv(b"GETENV_ONLY=from getenv");
    g.env.insert("IN_ENV", Value::from("from $_ENV"));
    g.server.insert("IN_SERVER", Value::from("from \\ server"));
    g.server.insert("HTTP_HOST", Value::from("example.com"));
    g
}

#[test]
fn parse_matches_php() -> TestResult {
    let cases: Json = serde_json::from_str(include_str!("data/parse.json"))?;
    let cases = cases.as_array().ok_or("no cases")?;
    assert!(cases.len() > 200);
    for case in cases {
        let data = base64(case["data"].as_str().ok_or("data")?)?;
        let what = String::from_utf8_lossy(&data).into_owned();
        let result = Dotenv::new().parse(&globals(), &data, ".env");
        let recorded = &case["result"];
        match (recorded[0].as_str(), result) {
            (Some("throws"), Err(e)) => {
                assert_eq!(recorded[1].as_str(), Some(e.class()), "{what:?}");
                assert_eq!(
                    String::from_utf8_lossy(&base64(recorded[2].as_str().ok_or("message")?)?),
                    String::from_utf8_lossy(&e.message()),
                    "{what:?}"
                );
            }
            (Some("throws"), Ok(values)) => panic!("{what:?}: expected {recorded}, got {values:?}"),
            (_, Err(e)) => panic!("{what:?}: expected {recorded}, got {e}"),
            (_, Ok(values)) => assert_eq!(decode(recorded)?, Value::Array(values), "{what:?}"),
        }
    }
    Ok(())
}

#[test]
fn a_value_ending_in_an_open_command_fails_instead_of_looping() {
    // upstream reads past the end of the data and never returns
    let result = Dotenv::new().parse(&Globals::new(), b"BAR=$(", ".env");
    let message = result.map_or_else(|e| e.to_string(), |_| String::new());
    assert!(
        message.starts_with("Missing closing parenthesis. in \".env\" at line 1."),
        "{message}"
    );
}
