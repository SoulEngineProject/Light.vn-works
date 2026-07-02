use lightvn_works::app::build_app;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // - Log level via RUST_LOG (defaults to info when unset).
    // - Note: RUST_LOG="" (set-but-empty) resolves to ERROR-only and hides request logs.
    // - with_ansi(false): Render captures stdout with no TTY, so drop colour escapes.
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app = build_app();

    let addr = SocketAddr::from(([0, 0, 0, 0], get_port()));
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}
