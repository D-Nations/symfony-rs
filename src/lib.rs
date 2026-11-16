//! A second application using the request crate, without demo dependencies.
//! These diagnostic routes are examples, not ports of demo controllers.

#![forbid(unsafe_code)]

use axum::{
    Json, Router,
    body::to_bytes,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use serde_json::{Map, Value, json};
use std::sync::LazyLock;
use symfony_http_foundation::{
    POST_MAX_SIZE, accept_language,
    cookies::Cookies,
    form_fields,
    query::{Query, QueryValue},
    scheme::Scheme,
};
use symfony_routing::{Definition, Match, Requirement, RouteCollection, Segment, Target};

#[derive(Clone, Copy)]
enum Route {
    Inspect,
    Redirect,
    Error,
    Item,
}

impl Route {
    const fn path(self) -> &'static str {
        match self {
            Self::Inspect => "/inspect",
            Self::Redirect => "/redirect",
            Self::Error => "/error",
            Self::Item => "/item",
        }
    }
}

impl Target for Route {
    fn methods(&self) -> Option<&'static [&'static str]> {
        match self {
            Self::Inspect => None,
            _ => Some(&["GET"]),
        }
    }
}

static ROUTES: LazyLock<RouteCollection<Route>> = LazyLock::new(|| {
    let mut routes = RouteCollection::default();
    for route in [Route::Inspect, Route::Redirect, Route::Error] {
        routes.add(Definition::new(
            vec![Segment::Literal(route.path().trim_start_matches('/'))],
            move |_| Some(route),
        ));
    }
    routes.add(Definition::new(
        vec![
            Segment::Literal(Route::Item.path().trim_start_matches('/')),
            Segment::Parameter("id", Requirement::PositiveInt64),
        ],
        |_| Some(Route::Item),
    ));
    routes
});

pub fn router() -> Router {
    Router::new().fallback(dispatch)
}

async fn dispatch(request: Request) -> Response {
    let head = request.method() == axum::http::Method::HEAD;
    let mut response = match ROUTES.match_request(request.method().as_str(), request.uri().path()) {
        Match::Found(Route::Inspect) => inspect(request).await,
        Match::Found(Route::Redirect) => Redirect::to(Route::Inspect.path()).into_response(),
        Match::Found(Route::Error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Example bad request"})),
        )
            .into_response(),
        Match::Found(Route::Item) => Json(json!({"path": request.uri().path()})).into_response(),
        Match::MethodNotAllowed { allow } => (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, allow.join(", "))],
        )
            .into_response(),
        Match::TrailingSlashRedirect { mut path, .. } => {
            if let Some(query) = request.uri().query() {
                path.push('?');
                path.push_str(query);
            }
            match axum::http::HeaderValue::from_str(&path) {
                Ok(location) => (
                    StatusCode::MOVED_PERMANENTLY,
                    [(header::LOCATION, location)],
                )
                    .into_response(),
                Err(error) => {
                    eprintln!("request-lab: invalid redirect header: {error}");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Match::NotFound => StatusCode::NOT_FOUND.into_response(),
    };
    if head {
        *response.body_mut() = axum::body::Body::empty();
    }
    response
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
