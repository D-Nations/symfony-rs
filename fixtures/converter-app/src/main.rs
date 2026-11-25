//! Serves the converter fixture over HTTP on 127.0.0.1 and the port in
//! `FIXTURE_PORT` (default 8022), as nginx and PHP-FPM would, for comparing
//! on the wire.
//!
//! Upstream: none

#![forbid(unsafe_code)]

use std::{net::SocketAddr, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("converter fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = match std::env::var("FIXTURE_PORT") {
        Ok(text) => text
            .parse()
            .map_err(|_| format!("FIXTURE_PORT is not a port: {text:?}"))?,
        Err(_) => 8022,
    };
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("converter fixture on http://{address}");
    axum::serve(
        listener,
        converter_fixture_app::kernel::router_at(address, "127.0.0.1")
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
