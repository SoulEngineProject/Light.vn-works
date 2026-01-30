// src/main.rs
use axum::{
    routing::get,
    routing::get_service,
    Json,
    Router,
    extract::Path,
    response::{Html, IntoResponse},
    http::StatusCode,
};
use pulldown_cmark::{html, Parser};
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::fs;
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
    let root_dir = std::path::Path::new("works");

    // Wrap the initial call too (since it's recursive async)
    let root = Box::pin(build_node(root_dir, root_dir)).await
        .unwrap_or(Node {
            name: "works".to_string(),
            path: "/works".to_string(),
            is_dir: true,
            children: Some(vec![]),
        });

    Json(root)
}

async fn render_markdown(
    Path((year, title)): Path<(String, String)>,
) -> impl IntoResponse {
    // Build safe path: works/year/title.md
    println!("Requested: year={}, title={}", year, title);  // ← add this

    let mut file_path = PathBuf::from("works");
    file_path.push(&year);
    file_path.push(format!("{}.md", title));

    println!("Trying to read: {:?}", file_path);  // ← add this

    let content = match fs::read_to_string(&file_path).await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!(
                    r#"
                    <!DOCTYPE html>
                    <html lang="en" class="dark">
                    <head><title>404 Not Found</title>
                    <style>body {{ background:#0a0a0f; color:#e0e0ff; font-family:sans-serif; padding:4rem; text-align:center; }}</style>
                    </head>
                    <body>
                        <h1>404 - Not Found</h1>
                        <p>Could not find: <code>{}/{}.md</code></p>
                        <p><a href="/" style="color:#6366f1;">← Back to archive</a></p>
                    </body>
                    </html>
                    "#,
                    year, title
                )),
            );
        }
    };

    // Convert to HTML
    let md_html = markdown_to_html(&content);

    // Nice wrapper page (reuse your dark theme)
    let full_page = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en" class="dark">
        <head>
            <meta charset="UTF-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
            <title>{title} - {year}</title>
            <link rel="preconnect" href="https://fonts.googleapis.com">
            <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
            <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
            <style>
                :root {{
                    --bg: #0a0a0f;
                    --text: #e0e0ff;
                    --text-muted: #a0a0cc;
                    --accent: #6366f1;
                }}
                body {{
                    font-family: 'Inter', system-ui, sans-serif;
                    background: var(--bg);
                    color: var(--text);
                    min-height: 100vh;
                    padding: 3rem 1rem;
                    line-height: 1.7;
                    max-width: 900px;
                    margin: 0 auto;
                }}
                h1, h2, h3 {{ color: #fff; }}
                a {{ color: var(--accent); }}
                pre {{ background: #111119; padding: 1rem; border-radius: 0.5rem; overflow-x: auto; }}
                code {{ background: #111119; padding: 0.2em 0.4em; border-radius: 0.3rem; }}
                .back {{ display: inline-block; margin: 1.5rem 0; color: var(--text-muted); text-decoration: none; }}
                .back:hover {{ color: var(--accent); }}
            </style>
        </head>
        <body>
            <a href="/" class="back">← Back to archive</a>
            <h1>{title}</h1>
            <p style="color: var(--text-muted);">From {year}</p>
            <div>{md_html}</div>
        </body>
        </html>
        "#,
        title = title.replace('-', " ").replace('_', " "),  // optional: make title pretty
        year = year,
        md_html = md_html
    );

    // Return success as tuple too (StatusCode::OK is implicit if omitted, but explicit is clearer)
    (
        StatusCode::OK,
        Html(full_page),
    )
}

// Returns HTML string from Markdown content
fn markdown_to_html(md_content: &str) -> String {
    let mut html_output = String::new();
    let parser = Parser::new(md_content);
    html::push_html(&mut html_output, parser);
    html_output
}

// 3. Recursive helper — stays async
async fn build_node(base: &std::path::Path, entry: &std::path::Path) -> Option<Node> {
    let name = entry.file_name()?.to_string_lossy().into_owned();
    let rel_path = {
        let stripped = entry.strip_prefix(base).ok()?.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = stripped.split('/').filter(|s| !s.is_empty()).collect();
        
        if parts.is_empty() {
            "/works".to_string()
        } else {
            format!("/works/{}", parts.join("/"))
        }
    };

    if entry.is_file() {
        return Some(Node {
            name,
            path: rel_path,
            is_dir: false,
            children: None,
        });
    }

    if !entry.is_dir() {
        return None;
    }

    let mut children = vec![];

    for entry in WalkDir::new(entry)
        .max_depth(2)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        // ← Key change: Box::pin around the recursive call
        if let Some(node) = Box::pin(build_node(base, entry.path())).await {
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
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .nest_service("/raw", ServeDir::new("works"))
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
