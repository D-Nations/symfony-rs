//! A second application using the request crate, without demo dependencies.
//! These diagnostic routes are examples, not ports of demo controllers.

#![forbid(unsafe_code)]

use axum::{
    Json, Router,
    body::to_bytes,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{any, get},
};
use serde_json::{Map, Value, json};
use symfony_http_foundation::{
    POST_MAX_SIZE, accept_language,
    cookies::Cookies,
    form_fields,
    query::{Query, QueryValue},
    scheme::Scheme,
};

enum Route {
    Inspect,
    Redirect,
    Error,
}

impl Route {
    const fn path(self) -> &'static str {
        match self {
            Self::Inspect => "/inspect",
            Self::Redirect => "/redirect",
            Self::Error => "/error",
        }
    }
}

pub fn router() -> Router {
    Router::new()
        .route(Route::Inspect.path(), any(inspect))
        .route(
            Route::Redirect.path(),
            get(|| async { Redirect::to(Route::Inspect.path()) }),
        )
        .route(
            Route::Error.path(),
            get(|| async {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Example bad request"})),
                )
            }),
        )
}

fn fields(entries: &[(String, QueryValue)]) -> Value {
    Value::Object(
        entries
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    QueryValue::Str(text) => Value::String(text.clone()),
                    QueryValue::Map(children) => fields(children),
                };
                (name.clone(), value)
            })
            .collect(),
    )
}

async fn inspect(request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let query = Query::parse(parts.uri.query().unwrap_or_default());
    // Exercise InputBag's scalar contract as well as its array view.
    let scalar = match query.get("scalar") {
        Ok(value) => value.map(str::to_owned),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "scalar must be a string"})),
            )
                .into_response();
        }
    };
    let body = match to_bytes(body, POST_MAX_SIZE).await {
        Ok(body) => body,
        Err(error) => {
            eprintln!("request-lab: body read failed: {error}");
            Default::default()
        }
    };
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .map(|value| value.as_bytes())
        .unwrap_or_default();
    let form = form_fields(parts.method.as_str(), content_type, &body);
    let cookies = Cookies::parse(
        parts
            .headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok()),
    );
    let cookies: Map<String, Value> = cookies
        .iter()
        .map(|(key, value)| (key.to_owned(), Value::String(value.to_owned())))
        .collect();
    let language = accept_language::preferred_language(
        parts
            .headers
            .get(header::ACCEPT_LANGUAGE)
            .map(|value| value.as_bytes()),
        &["en", "fr"],
    );
    Json(json!({
        "query": fields(query.all()),
        "form": fields(form.all()),
        "cookies": cookies,
        "scalar": scalar,
        "language": language,
        "scheme": Scheme::from_request(&parts, &[]).as_str(),
    }))
    .into_response()
}
