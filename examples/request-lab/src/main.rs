//! Starts the independent request lab on loopback; no database or assets needed.
//!
//! Upstream: none

use std::{net::SocketAddr, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("request-lab: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = std::env::var("REQUEST_LAB_LISTEN_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8012".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("request-lab listening at http://{}", listener.local_addr()?);
    axum::serve(
        listener,
        request_lab::router().into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
