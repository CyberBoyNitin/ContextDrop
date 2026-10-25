#![forbid(unsafe_code)]
//! Entry point for the loopback-only ContextDrop server.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use contextdrop_server::{build_app, AppState};

#[derive(Debug, Parser)]
#[command(
    name = "contextdrop-server",
    version,
    about = "Serve the ContextDrop local API and dashboard",
    long_about = "Starts a loopback-only HTTP server for ContextDrop. Non-loopback bind addresses are rejected. The server does not collect telemetry or log request content."
)]
struct Arguments {
    /// Loopback address to bind (for example 127.0.0.1:8787 or [::1]:8787).
    #[arg(long, default_value = "127.0.0.1:8787")]
    bind: SocketAddr,

    /// Directory containing the built dashboard.
    #[arg(long, default_value = "web/dist")]
    web_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("contextdrop-server: error: {message}");
        process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let arguments = Arguments::parse();
    if !arguments.bind.ip().is_loopback() {
        return Err(format!(
            "refusing to bind to {}; ContextDrop only listens on loopback addresses",
            arguments.bind
        ));
    }

    let state = AppState::load()?;
    let app = build_app(state, Some(arguments.web_dir.clone()));
    let listener = tokio::net::TcpListener::bind(arguments.bind)
        .await
        .map_err(|error| format!("cannot bind {}: {error}", arguments.bind))?;
    let actual = listener
        .local_addr()
        .map_err(|error| format!("cannot read listener address: {error}"))?;

    if !arguments.web_dir.join("index.html").is_file() {
        eprintln!(
            "note: no dashboard build found at {}; serving API and build instructions",
            arguments.web_dir.display()
        );
    }

    eprintln!(
        "ContextDrop {VERSION} listening on http://{actual}",
        VERSION = contextdrop_core::VERSION
    );
    eprintln!("Dashboard: http://{actual}/  |  Local API: http://{actual}/api/health");
    eprintln!(
        "Processing is loopback-only. No telemetry, content logging, or external model calls."
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| format!("server stopped: {error}"))
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        // If signal registration fails, simply wait forever rather than exiting
        // the already-running server without a shutdown path.
        std::future::pending::<()>().await;
    }
}
