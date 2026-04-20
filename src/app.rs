use axum::{
    routing::get,
    routing::get_service,
    Json,
    Router,
    extract::Path as AxumPath,
    response::{Html, IntoResponse},
    http::{HeaderMap, StatusCode},
    extract::{Query, State},
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tower_http::services::ServeDir;
use walkdir::WalkDir;

use crate::{
    GameMeta, CreatorGame, TagInfo, parse_frontmatter, extract_all_images, pick_thumbnail,
    markdown_to_html, html_escape, strip_img_tags, build_creator_index,
    get_related_games_by_creator, gallery_rows, build_tags_line, load_aliases, load_tag_config,
    tag_style, get_lang,
};

#[derive(Clone)]
struct AppState {
    creator_index: Arc<HashMap<String, Vec<CreatorGame>>>,
    aliases: Arc<HashMap<String, Vec<String>>>,
    tag_config: Arc<HashMap<String, TagInfo>>,
    game_count: usize,
    tree_json: Arc<String>,
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
                thumbnail = pick_thumbnail(body, parsed_meta.thumbnail_index);
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
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    AxumPath((year, title)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let lang_param = params.get("lang").map(|s| s.as_str());
    let accept_lang = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("en");
    let detected_lang = match lang_param {
        Some("ja") => "ja",
        Some("en") => "en",
        _ => if accept_lang.contains("ja") { "ja" } else { "en" },
    };
    let lang = get_lang(detected_lang);
    let lang_suffix = if lang_param.is_some() {
        format!("?lang={}", detected_lang)
    } else {
        String::new()
    };

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

    let tags = meta.tags.as_deref().unwrap_or(&[]);
    let is_r18 = tags.iter().any(|t| t == "r18");
    // When navigating back from an R18 game page, uncheck "Hide R18" on the
    // home page so the game is visible in the list.
    let home_suffix = match (lang_param.is_some(), is_r18) {
        (true, true) => format!("?lang={}&r18=0", detected_lang),
        (true, false) => format!("?lang={}", detected_lang),
        (false, true) => "?r18=0".to_string(),
        (false, false) => String::new(),
    };
    let released = meta.released.as_deref().unwrap_or("");
    let tags_line = build_tags_line(tags, &lang.tags_label, lang_param, &state.tag_config, released);

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
        let gallery_images = &images[1..];
        let rows = gallery_rows(gallery_images.len());
        let mut idx = 0;
        let mut html = String::new();
        for cols in &rows {
            html += &format!(r#"<div class="gallery gallery-{}">"#, cols);
            for _ in 0..*cols {
                html += &format!(
                    r#"<img src="{}" alt="Screenshot" loading="lazy" />"#,
                    html_escape(&gallery_images[idx])
                );
                idx += 1;
            }
            html += "</div>";
        }
        html
    } else {
        String::new()
    };

    let synopsis_html = strip_img_tags(&md_html);

    let tagline = meta.tagline.as_deref().unwrap_or("");
    let og_image = images.first().map(|s| s.as_str()).unwrap_or("");

    let current_path = format!("/works/{}/{}", &year, &title);
    let creator_field = meta.creator.as_deref().unwrap_or("");
    let related_by_creator = get_related_games_by_creator(&state.creator_index, creator_field, &current_path, usize::MAX, &state.aliases);
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
                    let badge: String = g.tags.iter().map(|tag| {
                        let style_attr = match tag_style(tag, &state.tag_config) {
                            Some(s) => format!(r#" style="{}""#, s),
                            None => String::new(),
                        };
                        format!(
                            r#"<span class="card-badge"{}>{}</span>"#,
                            style_attr,
                            html_escape(&tag.to_uppercase())
                        )
                    }).collect();
                    format!(
                        r#"<a href="{}{}" class="more-creator-card"><div class="more-creator-thumb">{}{}</div><span class="more-creator-title">{}</span></a>"#,
                        html_escape(&g.path),
                        lang_suffix,
                        badge,
                        thumb,
                        html_escape(&g.title)
                    )
                })
                .collect();
            format!(
                r#"<div class="more-creator"><h2>{}</h2><div class="more-creator-grid">{}</div></div>"#,
                lang.more_from.replace("{creator}", &html_escape(name)),
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
        .replace("{{tags_line}}", &tags_line)
        .replace("{{creator_html}}", &creator_html)
        .replace("{{released_html}}", &released_html)
        .replace("{{link_html}}", &link_html)
        .replace("{{extra_links_html}}", &extra_links_html)
        .replace("{{synopsis_html}}", &synopsis_html)
        .replace("{{gallery_html}}", &gallery_html)
        .replace("{{more_from_creator}}", &more_from_creator)
        .replace("{{lang_share}}", &lang.share)
        .replace("{{lang_copied}}", &lang.copied)
        .replace("{{lang_footer}}", &lang.footer)
        .replace("{{lang_breadcrumb_works}}", &lang.breadcrumb_works)
        .replace("{{lang_detected_lang}}", detected_lang)
        .replace("{{lang_suffix}}", &lang_suffix)
        .replace("{{home_suffix}}", &home_suffix);

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

fn build_tree_sync() -> Node {
    let root_dir = FsPath::new("works");
    let mut nodes: HashMap<String, Node> = HashMap::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let full_path = entry.path();
        if full_path == root_dir { continue; }

        let rel_path = match full_path.strip_prefix(root_dir) {
            Ok(stripped) => {
                let path_str = stripped.to_string_lossy().replace('\\', "/").trim_matches('/').to_string();
                if path_str.is_empty() { "/works".to_string() } else { format!("/works/{}", path_str) }
            }
            Err(_) => continue,
        };

        let name = full_path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let is_dir = full_path.is_dir();
        let mut thumbnail = None;
        let mut meta = None;

        if !is_dir && full_path.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("md")) {
            if let Ok(content) = std::fs::read_to_string(full_path) {
                let (parsed_meta, body) = parse_frontmatter(&content);
                thumbnail = pick_thumbnail(body, parsed_meta.thumbnail_index);
                meta = Some(parsed_meta);
            }
        }

        nodes.insert(rel_path.clone(), Node {
            name, path: rel_path, is_dir,
            children: if is_dir { Some(Vec::new()) } else { None },
            thumbnail, meta,
        });
    }

    let mut root = nodes.remove("/works").unwrap_or(Node {
        name: "works".to_string(), path: "/works".to_string(), is_dir: true,
        children: Some(Vec::new()), thumbnail: None, meta: None,
    });

    let mut by_parent: HashMap<String, Vec<String>> = HashMap::new();
    for path in nodes.keys() {
        if let Some(parent) = parent_path(path) {
            by_parent.entry(parent).or_default().push(path.clone());
        }
    }
    for children in by_parent.values_mut() { children.sort(); }
    attach_children(&mut root, &nodes, &by_parent);
    root
}

fn build_startup_index() -> (HashMap<String, Vec<CreatorGame>>, usize) {
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
        let thumbnail = pick_thumbnail(body, meta.thumbnail_index);

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

    let count = entries.len();
    (build_creator_index(&entries), count)
}

pub fn build_app() -> Router {
    // Tree data, translations, and tag config are all embedded in the home page HTML
    // at serve time. This eliminates client-side API fetches and renders the page
    // instantly without a loading state. The /api/tree route is kept for external use.
    let tree = build_tree_sync();
    let tree_json = serde_json::to_string(&tree).unwrap_or_default();
    let (creator_index, game_count) = build_startup_index();
    // Creator aliases: maps different names for the same person so "More from"
    // sections find games across all their aliases.
    let aliases = load_aliases(include_str!("../config/aliases.yaml"));
    // Tag config: defines colours and optional contest URLs per tag.
    let tag_config = load_tag_config(include_str!("../config/tags.yaml"));
    let state = AppState {
        creator_index: Arc::new(creator_index),
        aliases: Arc::new(aliases),
        tag_config: Arc::new(tag_config),
        game_count,
        tree_json: Arc::new(tree_json),
    };

    let serve_dir = ServeDir::new("public").not_found_service(
        ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))),
    );

    Router::new()
        .route("/", get(serve_home))
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .nest_service("/raw", ServeDir::new("works"))
        .fallback_service(serve_dir)
        .with_state(state)
}

async fn serve_home(State(state): State<AppState>) -> Html<String> {
    let page = include_str!("../public/index.html")
        .replace("{{game_count}}", &state.game_count.to_string())
        .replace("{{lang_json}}", include_str!("../config/lang.json"))
        .replace("{{tag_colours_json}}", &{
            let colours: HashMap<String, String> = state.tag_config.iter()
                .map(|(k, v)| (k.clone(), v.colour.clone()))
                .collect();
            serde_json::to_string(&colours).unwrap_or_default()
        })
        .replace("{{tree_json}}", &state.tree_json);
    Html(page)
}

async fn handler_404() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../public/404.html"))
}
