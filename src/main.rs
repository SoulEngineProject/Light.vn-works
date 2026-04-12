use axum::{
    routing::get,
    routing::get_service,
    Json,
    Router,
    extract::Path as AxumPath,
    response::{Html, IntoResponse},
    http::StatusCode,
};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use tokio::fs;
use tower_http::services::ServeDir;
use walkdir::WalkDir;

use lightvn_works::{
    GameMeta, parse_frontmatter, extract_first_image, extract_all_images,
    markdown_to_html, html_escape, strip_img_tags,
};

#[derive(Serialize, Clone)]
struct Node {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<GameMeta>,
}

async fn get_tree() -> Json<Node> {
    let root_dir = FsPath::new("works");

    let mut nodes: HashMap<String, Node> = HashMap::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let full_path = entry.path();

        if full_path == root_dir {
            continue;
        }

        let rel_path = match full_path.strip_prefix(root_dir) {
            Ok(stripped) => {
                let path_str = stripped
                    .to_string_lossy()
                    .replace('\\', "/")
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
        let mut meta = None;

        if !is_dir
            && full_path
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |e| e.eq_ignore_ascii_case("md"))
        {
            if let Ok(content) = fs::read_to_string(full_path).await {
                let (parsed_meta, body) = parse_frontmatter(&content);
                thumbnail = extract_first_image(body);
                meta = Some(parsed_meta);
            }
        }

        let node = Node {
            name,
            path: rel_path.clone(),
            is_dir,
            children: if is_dir { Some(Vec::new()) } else { None },
            thumbnail,
            meta,
        };

        nodes.insert(rel_path, node);
    }

    let mut root = nodes.remove("/works").unwrap_or(Node {
        name: "works".to_string(),
        path: "/works".to_string(),
        is_dir: true,
        children: Some(Vec::new()),
        thumbnail: None,
        meta: None,
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
        None
    } else {
        Some(path[..last_slash].to_string())
    }
}

fn attach_children(
    node: &mut Node,
    all_nodes: &HashMap<String, Node>,
    by_parent: &HashMap<String, Vec<String>>,
) {
    if let Some(child_paths) = by_parent.get(&node.path) {
        let mut children = Vec::new();
        for child_path in child_paths {
            if let Some(child) = all_nodes.get(child_path) {
                let mut child_clone = child.clone();
                attach_children(&mut child_clone, all_nodes, by_parent);
                children.push(child_clone);
            }
        }
        node.children = if children.is_empty() {
            None
        } else {
            Some(children)
        };
    }
}

async fn render_markdown(
    AxumPath((year, title)): AxumPath<(String, String)>,
) -> impl IntoResponse {
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

    let file_path = PathBuf::from("works")
        .join(&year)
        .join(format!("{}.md", title));

    if !file_path.starts_with("works/") || !file_path.is_file() {
        return not_found_html(&year, &title);
    }

    let content = match fs::read_to_string(&file_path).await {
        Ok(c) => c,
        Err(_) => return not_found_html(&year, &title),
    };

    let (meta, body) = parse_frontmatter(&content);
    let images = extract_all_images(body);
    let md_html = markdown_to_html(body);

    let title_display = title.clone();

    let creator_html = meta
        .creator
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(|c| format!(r#"<span class="meta-item">by {}</span>"#, html_escape(c)))
        .unwrap_or_default();

    let released_html = meta
        .released
        .as_deref()
        .filter(|r| !r.is_empty())
        .map(|r| format!(r#"<span class="meta-item">{}</span>"#, html_escape(r)))
        .unwrap_or_default();

    let mut link_html = String::new();
    if let (Some(label), Some(url)) = (meta.link_label.as_deref(), meta.link_url.as_deref()) {
        if !url.is_empty() {
            link_html = format!(
                r#"<a href="{}" class="play-btn" target="_blank" rel="noopener">{} ↗</a>"#,
                html_escape(url),
                html_escape(if label.is_empty() { "Play" } else { label })
            );
        }
    }

    let mut extra_links_html = String::new();
    if let Some(extras) = &meta.extra_links {
        for link in extras {
            if !link.url.is_empty() {
                extra_links_html += &format!(
                    r#"<a href="{}" class="extra-link" target="_blank" rel="noopener">{} ↗</a>"#,
                    html_escape(&link.url),
                    html_escape(&link.label)
                );
            }
        }
    }

    let hero_html = images.first().map(|url| {
        format!(
            r#"<div class="hero-image"><img src="{}" alt="{}" /></div>"#,
            html_escape(url),
            html_escape(&title_display)
        )
    }).unwrap_or_default();

    let gallery_html = if images.len() > 1 {
        let imgs: String = images[1..]
            .iter()
            .map(|url| {
                format!(
                    r#"<img src="{}" alt="Screenshot" loading="lazy" />"#,
                    html_escape(url)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(r#"<div class="gallery">{}</div>"#, imgs)
    } else {
        String::new()
    };

    let synopsis_html = strip_img_tags(&md_html);

    let page = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
    <title>{title_display} ({year}) - Light.vn Works</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg: #0d0b12;
            --surface: #16131e;
            --text: #ede9fe;
            --text-muted: #a8a2c6;
            --accent: #c084fc;
            --accent-hover: #d8b4fe;
            --border: #2a2440;
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: 'Inter', system-ui, sans-serif;
            background: var(--bg);
            color: var(--text);
            min-height: 100vh;
            line-height: 1.7;
        }}
        .breadcrumb {{
            max-width: 960px;
            margin: 0 auto;
            padding: 1.5rem 1.5rem 0;
            font-size: 0.9rem;
        }}
        .breadcrumb a {{
            color: var(--text-muted);
            text-decoration: none;
            transition: color 0.2s;
        }}
        .breadcrumb a:hover {{ color: var(--accent); }}
        .breadcrumb span {{ color: var(--text-muted); margin: 0 0.4rem; }}

        .hero-image {{
            max-width: 960px;
            margin: 1.5rem auto 0;
            padding: 0 1.5rem;
        }}
        .hero-image img {{
            width: 100%;
            max-height: 420px;
            object-fit: cover;
            border-radius: 1rem;
            display: block;
        }}

        .content {{
            max-width: 720px;
            margin: 0 auto;
            padding: 2rem 1.5rem 4rem;
        }}
        .content h1 {{
            font-size: 2rem;
            font-weight: 700;
            color: #fff;
            margin-bottom: 0.75rem;
            letter-spacing: -0.02em;
        }}
        .meta-row {{
            display: flex;
            flex-wrap: wrap;
            align-items: center;
            gap: 0.75rem;
            margin-bottom: 1.5rem;
            font-size: 0.95rem;
        }}
        .meta-item {{
            color: var(--text-muted);
        }}
        .play-btn {{
            display: inline-block;
            padding: 0.5rem 1.25rem;
            background: var(--accent);
            color: #0d0b12;
            font-weight: 600;
            font-size: 0.9rem;
            border-radius: 0.5rem;
            text-decoration: none;
            transition: background 0.2s, transform 0.15s;
        }}
        .play-btn:hover {{
            background: var(--accent-hover);
            transform: translateY(-1px);
        }}
        .extra-link {{
            display: inline-block;
            padding: 0.4rem 1rem;
            background: var(--surface);
            border: 1px solid var(--border);
            color: var(--text);
            font-size: 0.85rem;
            border-radius: 0.5rem;
            text-decoration: none;
            transition: border-color 0.2s, color 0.2s;
        }}
        .extra-link:hover {{
            border-color: var(--accent);
            color: var(--accent);
        }}

        .synopsis {{
            font-size: 1.05rem;
            line-height: 1.8;
            margin-bottom: 2rem;
        }}
        .synopsis p {{ margin-bottom: 1em; }}
        .synopsis hr {{
            border: none;
            border-top: 1px solid var(--border);
            margin: 1.5rem 0;
        }}
        .synopsis a {{ color: var(--accent); }}
        .synopsis img {{ display: none; }}

        .gallery {{
            display: flex;
            gap: 0.75rem;
            overflow-x: auto;
            padding-bottom: 0.5rem;
            margin-top: 1rem;
        }}
        .gallery img {{
            height: 180px;
            border-radius: 0.75rem;
            object-fit: cover;
            flex-shrink: 0;
        }}

        @media (max-width: 640px) {{
            .content h1 {{ font-size: 1.5rem; }}
            .hero-image img {{ max-height: 240px; }}
            .gallery img {{ height: 120px; }}
        }}
    </style>
</head>
<body>
    <nav class="breadcrumb">
        <a href="/">Works</a>
        <span>/</span>
        <a href="/">{year}</a>
        <span>/</span>
        {title_display}
    </nav>
    {hero_html}
    <div class="content">
        <h1>{title_display}</h1>
        <div class="meta-row">
            {creator_html}
            {released_html}
            {link_html}
            {extra_links_html}
        </div>
        <div class="synopsis">{synopsis_html}</div>
        {gallery_html}
    </div>
</body>
</html>"##,
        title_display = html_escape(&title_display),
        year = html_escape(&year),
        hero_html = hero_html,
        creator_html = creator_html,
        released_html = released_html,
        link_html = link_html,
        extra_links_html = extra_links_html,
        synopsis_html = synopsis_html,
        gallery_html = gallery_html,
    );

    (StatusCode::OK, Html(page))
}

fn not_found_html(year: &str, title: &str) -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head><title>404 Not Found</title>
<style>body {{ background:#0d0b12; color:#ede9fe; font-family:sans-serif; padding:4rem; text-align:center; }}</style>
</head>
<body>
    <h1>404 - Not Found</h1>
    <p>Could not find: <code>{year}/{title}.md</code></p>
    <p><a href="/" style="color:#c084fc;">Back to archive</a></p>
</body>
</html>"#,
            year = year,
            title = title
        )),
    )
}

#[tokio::main]
async fn main() {
    let serve_dir = ServeDir::new("public").not_found_service(
        ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))),
    );

    let app = Router::new()
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .nest_service("/raw", ServeDir::new("works"))
        .fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], get_port()));
    println!("Listening on http://{}", addr);

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
