//! `RequestContext` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/routing_request_context.php` (`scripts/record.sh
//! routing-request-context`): `fromUri()` on 3,000 generated URIs with
//! varied default hosts, schemes and ports, and `setBaseUrl()` on 10 base
//! URLs. Contexts compare field by field with `===`.
//!
//! Upstream: none

use php_value::{
    PhpArray, Value,
    vectors::{self, Error, base64},
};
use serde_json::Value as Json;
use symfony_routing::request_context::RequestContext;

fn data() -> Result<Json, Error> {
    Ok(serde_json::from_str(include_str!(
        "data/request_context.json"
    ))?)
}

fn state(c: &RequestContext) -> Value {
    let s = |v: &php_value::PhpString| Value::String(v.clone());
    Value::Array(PhpArray::list([
        s(c.base_url()),
        s(c.path_info()),
        s(c.method()),
        s(c.host()),
        s(c.scheme()),
        Value::Int(c.http_port()),
        Value::Int(c.https_port()),
        s(c.query_string()),
    ]))
}

fn bytes(json: &Json) -> Result<Vec<u8>, Error> {
    base64(json.as_str().ok_or("expected base64")?)
}

#[test]
fn recorded_from_uri() -> Result<(), Error> {
    let data = data()?;
    let cases = data["from_uri"].as_array().ok_or("from_uri")?;
    assert_eq!(3000, cases.len());
    for case in cases {
        let uri = bytes(&case[0])?;
        let context = RequestContext::from_uri(
            &uri,
            &bytes(&case[1])?,
            &bytes(&case[2])?,
            case[3].as_i64().ok_or("http port")?,
            case[4].as_i64().ok_or("https port")?,
        );
        assert_eq!(
            vectors::decode(&case[5])?,
            state(&context),
            "{:?}",
            String::from_utf8_lossy(&uri)
        );
    }
    Ok(())
}

#[test]
fn recorded_set_base_url() -> Result<(), Error> {
    let data = data()?;
    let cases = data["base_url"].as_array().ok_or("base_url")?;
    assert_eq!(10, cases.len());
    for case in cases {
        let base_url = bytes(&case[0])?;
        let mut context = RequestContext::default();
        context.set_base_url(&base_url);
        assert_eq!(vectors::decode(&case[1])?, state(&context));
    }
    Ok(())
}
