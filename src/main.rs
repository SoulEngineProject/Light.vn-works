use std::net::SocketAddr;
use lightvn_works::app::build_app;

#[tokio::main]
async fn main() {
    let app = build_app();

    let addr = SocketAddr::from(([0, 0, 0, 0], get_port()));
    println!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}
