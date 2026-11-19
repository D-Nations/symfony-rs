//! HTTP-level checks that the second application uses the compatibility core.
//!
//! Upstream: none

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::Value;
use tower::ServiceExt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn send(
    method: &str,
    path: &str,
    content_type: &str,
    body: &str,
) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
    let response = request_lab::router()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::COOKIE, "session=first; session=second; a.b=x%2By")
                .header(header::ACCEPT_LANGUAGE, "fr-CA, en;q=0.5")
                .header("x-forwarded-proto", "https")
                .body(Body::from(body.to_owned()))?,
        )
        .await?;
    let status = response.status();
    let value = serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await?)?;
    Ok((status, value))
}

#[tokio::test]
async fn independent_app_reads_php_query_cookie_and_language_rules() -> TestResult {
    let (status, value) = send(
        "GET",
        "/inspect?a.b=first&a_b=last&list[]=x&list[]=y&scalar=hello+world",
        "",
        "",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["query"]["a_b"], "last");
    assert_eq!(
        value["query"]["list"],
        serde_json::json!({"0":"x", "1":"y"})
    );
    assert_eq!(value["scalar"], "hello world");
    assert_eq!(value["cookies"]["session"], "first");
    assert_eq!(value["cookies"]["a_b"], "x+y");
    assert_eq!(value["language"], "fr");
    assert_eq!(value["scheme"], "http");
    Ok(())
}

#[tokio::test]
async fn forms_share_method_and_multipart_failure_rules() -> TestResult {
    for method in ["POST", "PUT", "PATCH", "DELETE", "QUERY"] {
        let (status, value) = send(
            method,
            "/inspect",
            "application/x-www-form-urlencoded",
            "name=one&name=two&tags[]=rust",
        )
        .await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["form"]["name"], "two", "{method}");
        assert_eq!(value["form"]["tags"]["0"], "rust", "{method}");
    }
    let (_, value) = send(
        "GET",
        "/inspect",
        "application/x-www-form-urlencoded",
        "name=ignored",
    )
    .await?;
    assert_eq!(value["form"], serde_json::json!({}));
    // A nameless part triggers a warning after the first field: POST
    // retains that field, but request_parse_body() fails as a whole for PUT.
    let body = "--Lab\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nvalue\r\n--Lab\r\nContent-Disposition: form-data\r\n\r\nignored\r\n--Lab--\r\n";
    let (_, post) = send(
        "POST",
        "/inspect",
        "multipart/form-data; boundary=Lab",
        body,
    )
    .await?;
    let (_, put) = send("PUT", "/inspect", "multipart/form-data; boundary=Lab", body).await?;
    assert_eq!(post["form"]["name"], "value");
    assert_eq!(put["form"], serde_json::json!({}));
    Ok(())
}

#[tokio::test]
async fn scalar_errors_and_fixed_redirects_are_visible_over_http() -> TestResult {
    let (status, value) = send("GET", "/inspect?scalar[]=x", "", "").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"], "scalar must be a string");
    let (status, _) = send("GET", "/error", "", "").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response = request_lab::router()
        .oneshot(
            Request::builder()
                .uri("/redirect?to=https://example.com")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/inspect");
    Ok(())
}

#[tokio::test]
async fn shared_router_checks_requirements_methods_and_slash_redirects() -> TestResult {
    for (method, path, status, location, allow) in [
        (
            "GET",
            "/inspect/?a=1",
            StatusCode::MOVED_PERMANENTLY,
            Some("/inspect?a=1"),
            None,
        ),
        (
            "POST",
            "/error",
            StatusCode::METHOD_NOT_ALLOWED,
            None,
            Some("GET"),
        ),
        ("GET", "/item/42", StatusCode::OK, None, None),
        ("GET", "/item/0", StatusCode::NOT_FOUND, None, None),
        (
            "GET",
            "/item/9223372036854775808",
            StatusCode::NOT_FOUND,
            None,
            None,
        ),
        ("HEAD", "/item/42", StatusCode::OK, None, None),
    ] {
        let response = request_lab::router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), status, "{method} {path}");
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            location
        );
        assert_eq!(
            response
                .headers()
                .get(header::ALLOW)
                .and_then(|v| v.to_str().ok()),
            allow
        );
        if method == "HEAD" {
            assert!(to_bytes(response.into_body(), 1024).await?.is_empty());
        }
    }
    Ok(())
}
