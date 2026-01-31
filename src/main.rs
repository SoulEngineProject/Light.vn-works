// src/main.rs
use axum::{
    routing::get,
    routing::get_service,
    Json,
    Router,
    extract::Path as AxumPath,           // renamed to avoid conflict
    response::{Html, IntoResponse},
    http::StatusCode,
};
use pulldown_cmark::{html, Parser, Event, Tag, LinkType, CowStr};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};   // renamed Path → FsPath
use tokio::fs;
use tower_http::services::ServeDir;
use walkdir::WalkDir;

#[derive(Serialize, Clone)]
struct Node {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<String>,
}

async fn get_tree() -> Json<Node> {
    let root_dir = FsPath::new("works");           // ← use FsPath

    println!("Current working directory: {:?}", std::env::current_dir().ok());
    println!("Does 'works' exist?     {:?}", root_dir.exists());
    println!("Is 'works' a directory? {:?}", root_dir.is_dir());

    if root_dir.is_dir() {
        match std::fs::read_dir(root_dir) {
            Ok(mut entries) => {
                println!("Entries in 'works/':");
                while let Some(entry) = entries.next() {
                    if let Ok(e) = entry {
                        println!("  - {:?}", e.path().display());
                    }
                }
            }
            Err(e) => println!("Cannot read 'works/': {}", e),
        }
    } else {
        println!("'works' is not a directory or cannot be accessed");
    }

    let mut nodes: HashMap<String, Node> = HashMap::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let full_path = entry.path();

        // Skip the root directory itself
        if full_path == root_dir {
            continue;
        }

        // Reliable relative path (works on Windows & Unix)
        let rel_path = match full_path.strip_prefix(root_dir) {
            Ok(stripped) => {
                let path_str = stripped
                    .to_string_lossy()
                    .replace('\\', "/")           // normalize to forward slashes
                    .trim_matches('/')
                    .to_string();

                if path_str.is_empty() {
                    "/works".to_string()
                } else {
                    format!("/works/{}", path_str)
                }
            }
            Err(_) => continue,
        };

        let name = full_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let is_dir = full_path.is_dir();

        let mut thumbnail = None;

        if !is_dir && full_path.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("md")) {
            if let Ok(content) = fs::read_to_string(full_path).await {
                thumbnail = extract_first_image(&content);
            }
        }

        println!(
            "Adding → path: \"{}\", name: \"{}\", is_dir: {}, thumbnail: {:?}",
            rel_path, name, is_dir, thumbnail
        );

        let node = Node {
            name,
            path: rel_path.clone(),
            is_dir,
            children: if is_dir { Some(Vec::new()) } else { None },
            thumbnail,
        };

        nodes.insert(rel_path, node);
    }

    // Build hierarchy
    let mut root = nodes.remove("/works").unwrap_or(Node {
        name: "works".to_string(),
        path: "/works".to_string(),
        is_dir: true,
        children: Some(Vec::new()),
        thumbnail: None,
    });

    let mut by_parent: HashMap<String, Vec<String>> = HashMap::new();

    for path in nodes.keys() {
        if let Some(parent) = parent_path(path) {
            by_parent.entry(parent).or_default().push(path.clone());
        }
    }

    for children in by_parent.values_mut() {
        children.sort();
    }

    attach_children(&mut root, &nodes, &by_parent);

    Json(root)
}

fn parent_path(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let last_slash = path.rfind('/')?;
    if last_slash == 0 {
        None  // root
    } else {
        Some(path[..last_slash].to_string())
    }
}

fn attach_children(node: &mut Node, all_nodes: &HashMap<String, Node>, by_parent: &HashMap<String, Vec<String>>) {
    if let Some(child_paths) = by_parent.get(&node.path) {
        let mut children = Vec::new();
        for child_path in child_paths {
            if let Some(child) = all_nodes.get(child_path) {
                let mut child_clone = child.clone();
                attach_children(&mut child_clone, all_nodes, by_parent);
                children.push(child_clone);
            }
        }
        node.children = if children.is_empty() { None } else { Some(children) };
    }
}

fn extract_first_image(md: &str) -> Option<String> {
    let parser = Parser::new(md);

    for event in parser {
        if let Event::Html(html) = event {
            let html_str = html.to_string();

            // Look for src="https://github.com/user-attachments/...
            if let Some(src_start) = html_str.find("src=\"https://github.com/user-attachments/") {
                let rest = &html_str[src_start + 5..]; // skip src="
                if let Some(end_quote) = rest.find('\"') {
                    let src_value = &rest[..end_quote];
                    // Quick sanity check: make sure it's still a github assets URL
                    if src_value.starts_with("https://github.com/user-attachments/") {
                        return Some(src_value.to_string());
                    }
                }
            }
        }
    }

    None
}

async fn render_markdown(AxumPath((year, title)): AxumPath<(String, String)>) -> impl IntoResponse {

    // Only check very basic length + no obvious traversal attempts
    if year.len() > 20
        || title.len() > 300
        || year.contains("..")
        || title.contains("..")
        || year.contains('/') 
        || title.contains('/')
    {
        return (
            StatusCode::BAD_REQUEST,
            Html("<h1>400 Bad Request</h1><p>Invalid year or title</p>".to_string()),
        );
    }
    
    let file_path = PathBuf::from("works").join(&year).join(format!("{}.md", title));

    if !file_path.starts_with("works/") || !file_path.is_file() {
        return not_found_html(&year, &title);
    }

    let content = match fs::read_to_string(&file_path).await {
        Ok(c) => c,
        Err(_) => return not_found_html(&year, &title),
    };

    let md_html = markdown_to_html(&content);

    let title_display = title
        .replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + chars.as_str()
        })
        .collect::<Vec<_>>()
        .join(" ");

    let page = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en" class="dark">
        <head>
            <meta charset="UTF-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
            <title>{title_display} - {year}</title>
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
                img {{ max-width: 100%; height: auto; border-radius: 0.5rem; }}
            </style>
        </head>
        <body>
            <a href="/" class="back">← Back to archive</a>
            <h1>{title_display}</h1>
            <p style="color: var(--text-muted);">From {year}</p>
            <div>{md_html}</div>
        </body>
        </html>
        "#,
        title_display = title_display,
        year = year,
        md_html = md_html
    );

    (StatusCode::OK, Html(page))
}

fn not_found_html(year: &str, title: &str) -> (StatusCode, Html<String>) {
    (
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
                <p>Could not find: <code>{year}/{title}.md</code></p>
                <p><a href="/" style="color:#6366f1;">← Back to archive</a></p>
            </body>
            </html>
            "#,
            year = year,
            title = title
        )),
    )
}

fn markdown_to_html(md_content: &str) -> String {
    let mut html_output = String::new();
    let parser = Parser::new(md_content);
    html::push_html(&mut html_output, parser);
    html_output
}

#[tokio::main]
async fn main() {
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