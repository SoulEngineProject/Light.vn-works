// src/main.rs
use axum::{
    routing::get_service,
    Router,
};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // Serve everything from the "public" folder
    let serve_dir = ServeDir::new("public")
        .not_found_service(ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))));

    let app = Router::new()
        .fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], get_port()));

    println!("→ Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handler_404() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../public/404.html"))
}

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}