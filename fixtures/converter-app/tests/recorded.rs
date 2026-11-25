//! Replays every request recorded from the PHP app
//! (oracle/apps/converter-fixture/responses.json, `scripts/record.sh
//! converter-fixture`) through the Rust fixture and compares status,
//! headers and body.
//!
//! Headers are compared as a multiset of (lowercased name, value): their
//! order is not behaviour. Left out: the headers PHP's built-in server adds
//! (`server_headers` in the recording) and `Date`, whose value is masked in
//! the recording and which the HTTP server, not the application, writes in
//! Rust.
//!
//! Upstream: none

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::Method,
};
use converter_fixture_app::{App, Server};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn recording() -> Result<Value, Box<dyn std::error::Error>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/apps/converter-fixture/responses.json"
    );
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn text(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("expected a string, got {value}"))
}

fn headers_of(case: &Value, ignored: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut headers = Vec::new();
    for pair in case["headers"].as_array().ok_or("headers")? {
        let name = text(&pair[0])?.to_ascii_lowercase();
        if name != "date" && !ignored.contains(&name) {
            headers.push((name, text(&pair[1])?.to_owned()));
        }
    }
    headers.sort();
    Ok(headers)
}

#[tokio::test]
async fn every_recorded_response_matches() -> TestResult {
    let recording = recording()?;
    assert_eq!(recording["kind"], "recorded from PHP");
    let ignored: Vec<String> = recording["server_headers"]
        .as_array()
        .ok_or("server_headers")?
        .iter()
        .map(|h| text(h).map(str::to_ascii_lowercase))
        .collect::<Result<_, _>>()?;
    let app = App::new(Server {
        name: "0.0.0.0".to_owned(),
        port: 8000,
    });
    let cases = recording["cases"].as_array().ok_or("cases")?;
    assert_eq!(cases.len(), 16);
    let mut failures = Vec::new();
    for case in cases {
        let name = text(&case["name"])?;
        let sent = &case["request"];
        let mut request = Request::builder()
            .method(Method::from_bytes(text(&sent["method"])?.as_bytes())?)
            .uri(text(&sent["path"])?);
        for (header, value) in sent["headers"].as_object().ok_or("request headers")? {
            request = request.header(header, text(value)?);
        }
        let response = app.handle(request.body(Body::empty())?).await;
        let status = response.status().as_u16();
        let mut headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(n, v)| {
                (
                    n.as_str().to_owned(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        headers.sort();
        let body = String::from_utf8(to_bytes(response.into_body(), 1 << 20).await?.to_vec())?;

        let expected_status = case["status"].as_u64().ok_or("status")?;
        let expected_headers = headers_of(case, &ignored)?;
        let expected_body = text(&case["body"])?;
        if u64::from(status) != expected_status {
            failures.push(format!("{name}: status {status}, PHP {expected_status}"));
        }
        if headers != expected_headers {
            failures.push(format!(
                "{name}: headers {headers:?}\n  PHP {expected_headers:?}"
            ));
        }
        if body != expected_body {
            failures.push(format!("{name}: body {body:?}\n  PHP {expected_body:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}

/// Two requests in one process each see a fresh `VisitCounter`, as each
/// PHP request does (the recording's `greet` and `greet-again`).
#[tokio::test]
async fn request_scoped_state_is_not_shared() -> TestResult {
    let app = App::new(Server {
        name: "0.0.0.0".to_owned(),
        port: 8000,
    });
    for _ in 0..3 {
        let request = Request::builder()
            .uri("/greet/world")
            .header("Host", "localhost")
            .body(Body::empty())?;
        let response = app.handle(request).await;
        let body = String::from_utf8(to_bytes(response.into_body(), 1 << 20).await?.to_vec())?;
        assert!(body.ends_with("visits: 1\n"), "{body}");
    }
    Ok(())
}

/// Converter contract 0.2, section 7.
#[test]
fn concurrency_bounds_hold() {
    fn send<T: Send>(_: &T) {}
    fn send_sync<T: Send + Sync>() {}
    fn scope_is_send<T: Send>() {}
    send_sync::<converter_fixture_app::container::Container>();
    send_sync::<App>();
    send_sync::<converter_fixture_app::app::error_subscriber::ErrorSubscriber>();
    scope_is_send::<converter_fixture_app::container::RequestScope>();
    let app = App::new(Server {
        name: "0.0.0.0".to_owned(),
        port: 8000,
    });
    let future = app.handle(Request::new(Body::empty()));
    send(&future);
}
