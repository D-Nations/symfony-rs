//! Serves the converter fixture over HTTP with Axum, on 127.0.0.1 and the
//! port in `FIXTURE_PORT` (default 8022), for comparing on the wire.
//!
//! Upstream: none

#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{Router, extract::Request};
use converter_fixture_app::{App, Server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = match std::env::var("FIXTURE_PORT") {
        Ok(text) => text
            .parse()
            .map_err(|_| std::io::Error::other(format!("FIXTURE_PORT is not a port: {text:?}")))?,
        Err(_) => 8022,
    };
    let app = Arc::new(App::new(Server {
        name: "127.0.0.1".to_owned(),
        port: i64::from(port),
    }));
    let router = Router::new().fallback(move |request: Request| {
        let app = Arc::clone(&app);
        async move { app.handle(request).await }
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!("converter fixture on http://127.0.0.1:{port}");
    axum::serve(listener, router).await
}
