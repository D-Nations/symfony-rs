//! Replays every request recorded from the PHP app
//! (oracle/apps/converter-fixture/responses.json, `scripts/record.sh
//! converter-fixture`) through the Rust fixture's front controller and
//! compares status, headers and body.
//!
//! Headers are compared as a multiset of (lowercased name, value): their
//! order is not behaviour. Left out: the headers PHP's built-in server adds
//! (`server_headers` in the recording: `Host`, `Connection`, `X-Powered-By`,
//! which PHP-FPM also sends) and `Date`, whose value is masked in the
//! recording and which the HTTP server, not the application, writes in
//! Rust. `Content-Length` is framing too: axum writes it for a body of
//! known size, where PHP's built-in server closed the connection instead
//! (`Connection: close`), so the recording has none. It is taken out of the
//! comparison only after checking that it is the body's length.
//!
//! Upstream: none

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::Method,
};
use converter_fixture_app::router;
use serde_json::Value;
use tower::ServiceExt;

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

/// One request through the front controller: status, headers (lowercased
/// names, `ignored` and `Date` left out, sorted) and body.
async fn send(
    request: Request,
    ignored: &[String],
) -> Result<(u16, Vec<(String, String)>, String), Box<dyn std::error::Error>> {
    let response = router().oneshot(request).await?;
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
        .filter(|(n, _)| n != "date" && !ignored.contains(n))
        .collect();
    headers.sort();
    let body = String::from_utf8(to_bytes(response.into_body(), 1 << 20).await?.to_vec())?;
    let lengths: Vec<&String> = headers
        .iter()
        .filter(|(n, _)| n == "content-length")
        .map(|(_, v)| v)
        .collect();
    if let [length] = lengths.as_slice() {
        if **length != body.len().to_string() {
            return Err(
                format!("Content-Length {length} for a body of {} bytes", body.len()).into(),
            );
        }
    } else if lengths.len() > 1 {
        return Err(format!("{} Content-Length headers", lengths.len()).into());
    }
    headers.retain(|(n, _)| n != "content-length");
    Ok((status, headers, body))
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
        let (status, headers, body) = send(request.body(Body::empty())?, &ignored).await?;

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

/// Three requests in one process each see a fresh `VisitCounter`, as each
/// PHP request does (the recording's `greet` and `greet-again`).
#[tokio::test]
async fn request_scoped_state_is_not_shared() -> TestResult {
    for _ in 0..3 {
        let request = Request::builder()
            .uri("/greet/world")
            .header("Host", "localhost")
            .body(Body::empty())?;
        let (_, _, body) = send(request, &[]).await?;
        assert!(body.ends_with("visits: 1\n"), "{body}");
    }
    Ok(())
}

/// Converter contract 0.3, section 7.
#[test]
fn concurrency_bounds_hold() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<converter_fixture_app::container::Container>();
    send_sync::<converter_fixture_app::container::ServiceContainer>();
    send_sync::<converter_fixture_app::app::error_subscriber::ErrorSubscriber>();
    send_sync::<converter_fixture_app::app::greet_controller::GreetController>();
    send_sync::<axum::Router>();
}
