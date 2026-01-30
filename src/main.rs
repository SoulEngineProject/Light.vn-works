// src/main.rs
use axum::{
    routing::get,
    routing::get_service,
    Json,
    Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::ServeDir;
use walkdir::WalkDir;

// 1. The Node struct (your tree representation)
#[derive(Serialize)]
struct Node {
    name: String,
    path: String,              // e.g. "/works/2025/title1.md"
    is_dir: bool,
    children: Option<Vec<Node>>,
}

// 2. The handler that returns the whole tree as JSON
async fn get_tree() -> Json<Node> {
    let root_dir = Path::new("works");
    let root = build_node(root_dir, root_dir).unwrap_or(Node {
        name: "works".to_string(),
        path: "/works".to_string(),
        is_dir: true,
        children: Some(vec![]),
    });
    Json(root)
}

// 3. Recursive helper to build the tree (simplified for 2-level depth)
async fn build_node(base: &Path, entry: &Path) -> Option<Node> {
    let name = entry.file_name()?.to_string_lossy().into_owned();
    let rel_path = format!("/{}", entry.strip_prefix(base).ok()?.to_string_lossy());

    if entry.is_file() {
        return Some(Node { name, path: rel_path, is_dir: false, children: None });
    }

    if !entry.is_dir() { return None; }

    let mut children = vec![];

    for entry in WalkDir::new(entry)
        .max_depth(2)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(node) = build_node(base, entry.path()) {
            children.push(node);
        }
    }

    Some(Node {
        name,
        path: rel_path,
        is_dir: true,
        children: if children.is_empty() { None } else { Some(children) },
    })
}

#[tokio::main]
async fn main() {
    // Serve everything from the "public" folder
    let serve_dir = ServeDir::new("public")
        .not_found_service(ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))));

    let app = Router::new()
        .route("/api/tree", get(get_tree))           // ← this line uses the code above
        .nest_service("/works", tower_http::services::ServeDir::new("works"))
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
