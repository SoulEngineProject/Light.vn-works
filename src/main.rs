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
    GameMeta, CreatorGame, parse_frontmatter, extract_first_image, extract_all_images,
    markdown_to_html, html_escape, strip_img_tags, build_creator_index, get_related_games_by_creator,
};
use std::sync::Arc;
use axum::extract::State;

#[derive(Clone)]
struct AppState {
    creator_index: Arc<HashMap<String, Vec<CreatorGame>>>,
}

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
    State(state): State<AppState>,
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

    let tags_html: String = meta
        .tags
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|tag| {
            let class = if tag == "r18" { "tag-badge tag-r18" } else { "tag-badge tag-default" };
            format!(
                r#"<span class="{}">{}</span>"#,
                class,
                html_escape(&tag.to_uppercase())
            )
        })
        .collect();

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

    let tagline = meta.tagline.as_deref().unwrap_or("");
    let og_image = images.first().map(|s| s.as_str()).unwrap_or("");

    let current_path = format!("/works/{}/{}", &year, &title);
    let creator_field = meta.creator.as_deref().unwrap_or("");
    let related_by_creator = get_related_games_by_creator(&state.creator_index, creator_field, &current_path, usize::MAX);
    let more_from_creator: String = related_by_creator
        .iter()
        .map(|(name, games)| {
            let cards: String = games
                .iter()
                .map(|g| {
                    let thumb = g.thumbnail.as_deref().map(|url| {
                        format!(
                            r#"<img src="{}" alt="{}" loading="lazy" />"#,
                            html_escape(url),
                            html_escape(&g.title)
                        )
                    }).unwrap_or_else(|| r#"<div class="more-creator-placeholder">&#10024;</div>"#.to_string());
                    let badge = if g.tags.contains(&"r18".to_string()) {
                        r#"<span class="card-badge card-badge-r18">R18</span>"#
                    } else {
                        ""
                    };
                    format!(
                        r#"<a href="{}" class="more-creator-card">{}{}<span>{}</span></a>"#,
                        html_escape(&g.path),
                        badge,
                        thumb,
                        html_escape(&g.title)
                    )
                })
                .collect();
            format!(
                r#"<div class="more-creator"><h2>More from {}</h2><div class="more-creator-grid">{}</div></div>"#,
                html_escape(name),
                cards
            )
        })
        .collect();

    let page = include_str!("../public/game.html")
        .replace("{{title_display}}", &html_escape(&title_display))
        .replace("{{year}}", &html_escape(&year))
        .replace("{{tagline}}", &html_escape(tagline))
        .replace("{{og_image}}", &html_escape(og_image))
        .replace("{{hero_html}}", &hero_html)
        .replace("{{tags_html}}", &tags_html)
        .replace("{{creator_html}}", &creator_html)
        .replace("{{released_html}}", &released_html)
        .replace("{{link_html}}", &link_html)
        .replace("{{extra_links_html}}", &extra_links_html)
        .replace("{{synopsis_html}}", &synopsis_html)
        .replace("{{gallery_html}}", &gallery_html)
        .replace("{{more_from_creator}}", &more_from_creator);

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

fn build_startup_index() -> HashMap<String, Vec<CreatorGame>> {
    let root_dir = FsPath::new("works");
    let mut entries = Vec::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (meta, body) = parse_frontmatter(&content);
        let creator = meta.creator.unwrap_or_default();
        let released = meta.released.unwrap_or_default();
        let thumbnail = extract_first_image(body);

        let rel_path = path
            .strip_prefix(root_dir)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let link_path = format!("/works/{}", rel_path.trim_end_matches(".md"));

        let tags = meta.tags.unwrap_or_default();
        entries.push((creator, title, link_path, thumbnail, released, tags));
    }

    build_creator_index(&entries)
}

#[tokio::main]
async fn main() {
    let state = AppState {
        creator_index: Arc::new(build_startup_index()),
    };

    let serve_dir = ServeDir::new("public").not_found_service(
        ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))),
    );

    let app = Router::new()
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .nest_service("/raw", ServeDir::new("works"))
        .fallback_service(serve_dir)
        .with_state(state);

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
