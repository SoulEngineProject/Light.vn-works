use axum::{
    routing::get,
    routing::get_service,
    Router,
    extract::Path as AxumPath,
    response::{Html, IntoResponse},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    extract::{Query, State},
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path as FsPath;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use walkdir::WalkDir;

use crate::{
    GameMeta, ParsedGame, TagInfo, parse_frontmatter, extract_all_images,
    markdown_to_html, html_escape, strip_img_tags, build_creator_paths,
    get_related_paths, gallery_rows, build_tags_line, load_aliases, load_tag_config,
    tag_style, get_lang,
};

#[derive(Clone)]
struct AppState {
    games: Arc<HashMap<String, ParsedGame>>,
    creator_paths: Arc<HashMap<String, Vec<String>>>,
    aliases: Arc<HashMap<String, Vec<String>>>,
    tag_config: Arc<HashMap<String, TagInfo>>,
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
    thumbnail_composite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<GameMeta>,
}

async fn get_tree(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        state.tree_json.to_string(),
    )
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

    let canonical_path = format!("/works/{}/{}", &year, &title);
    let game = match state.games.get(&canonical_path) {
        Some(g) => g,
        None => return not_found_html(&year, &title),
    };
    let meta = &game.meta;
    let images = &game.images;
    let md_html = game.body_html.as_str();

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

    let hero_html = images.first().map(|img| {
        format!(
            r#"<div class="hero-image"><img src="{}" alt="{}" /></div>"#,
            html_escape(&img.url),
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
                    html_escape(&gallery_images[idx].url)
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

    // Fallback to title if no tagline — only used in meta/OG tags (SEO), not visible on page
    let tagline = meta.tagline.as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or(&title_display);
    let og_image = images.first().map(|img| img.url.as_str()).unwrap_or("");

    // Editor mockup: show last screenshot inside the Light.vn editor frame.
    // For composite images (width > height*2), crop to the rightmost third via CSS.
    let editor_img = if detected_lang == "ja" { "editor_jp.webp" } else { "editor_en.webp" };
    let editor_mockup = images.last().map(|img| {
        let preview_html = if img.is_composite() {
            format!(
                r#"<div class="editor-preview-crop" style="background-image:url('{}')"></div>"#,
                html_escape(&img.url)
            )
        } else {
            format!(
                r#"<img class="editor-preview" src="{}" alt="{}" loading="lazy" />"#,
                html_escape(&img.url),
                html_escape(&title_display)
            )
        };
        format!(
            r#"<div class="editor-mockup"><h2>{}</h2><div class="editor-mockup-frame"><img class="editor-frame" src="/{}" alt="" />{}</div></div>"#,
            html_escape(&lang.dev_example),
            editor_img,
            preview_html
        )
    }).unwrap_or_default();

    let creator_field = meta.creator.as_deref().unwrap_or("");
    let related = get_related_paths(&state.creator_paths, creator_field, &canonical_path, usize::MAX, &state.aliases);
    let more_from_creator: String = related
        .iter()
        .map(|(name, paths)| {
            let cards: String = paths
                .iter()
                .filter_map(|p| state.games.get(*p))
                .map(|g| {
                    let thumb = g.thumbnail.as_deref().map(|url| {
                        if g.thumbnail_composite {
                            format!(
                                r#"<div class="more-creator-thumb-composite" style="background-image:url('{}')"></div>"#,
                                html_escape(url)
                            )
                        } else {
                            format!(
                                r#"<img src="{}" alt="{}" loading="lazy" />"#,
                                html_escape(url),
                                html_escape(&g.title)
                            )
                        }
                    }).unwrap_or_else(|| r#"<div class="more-creator-placeholder">&#10024;</div>"#.to_string());
                    let tags = g.meta.tags.as_deref().unwrap_or(&[]);
                    let badge: String = tags.iter().map(|tag| {
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
        .replace("{{editor_mockup}}", &editor_mockup)
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

// Walk works/ once at startup. Parses each .md into a ParsedGame and keys by
// canonical path ("/works/YYYY/title"). Per-file parse is wrapped in
// catch_unwind so a panic in one file logs + skips, rather than crashing the
// server. The bad file is missing from the index; the rest of the catalog
// serves normally. Request for the skipped file yields 404.
fn build_games_index() -> HashMap<String, ParsedGame> {
    let root_dir = FsPath::new("works");
    let mut games = HashMap::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let rel_path = match path.strip_prefix(root_dir) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        // Expect shape "YYYY/title.md"
        let (year, _rest) = match rel_path.split_once('/') {
            Some(parts) => parts,
            None => continue,
        };
        let title = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let year = year.to_string();
        let canonical_path = format!("/works/{}/{}", year, title);

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            let (meta, body) = parse_frontmatter(&content);
            let images = extract_all_images(body);
            let body_html = markdown_to_html(body);
            let thumb_idx = meta.thumbnail_index.unwrap_or(0);
            let thumb_img = images.get(thumb_idx).or(images.first());
            let thumbnail = thumb_img.map(|img| img.url.clone());
            let thumbnail_composite = thumb_img.map_or(false, |img| img.is_composite());
            ParsedGame {
                year: year.clone(),
                title: title.clone(),
                path: canonical_path.clone(),
                meta,
                body_html,
                images,
                thumbnail,
                thumbnail_composite,
            }
        }));

        match parsed {
            Ok(game) => { games.insert(canonical_path, game); }
            Err(_) => {
                eprintln!("[startup] panic parsing {}; skipping", path.display());
            }
        }
    }

    games
}

// Build Node tree from pre-parsed games, grouped by year. Directory shape is
// always works/YYYY/file.md, so no recursive traversal is needed. The output
// JSON shape matches the legacy walker (node names and paths keep their .md
// suffix for client compat).
fn build_tree_from_games(games: &HashMap<String, ParsedGame>) -> Node {
    let mut by_year: BTreeMap<String, Vec<Node>> = BTreeMap::new();

    for game in games.values() {
        by_year.entry(game.year.clone()).or_default().push(Node {
            name: format!("{}.md", game.title),
            path: format!("{}.md", game.path),
            is_dir: false,
            children: None,
            thumbnail: game.thumbnail.clone(),
            thumbnail_composite: if game.thumbnail_composite { Some(true) } else { None },
            meta: Some(game.meta.clone()),
        });
    }

    for nodes in by_year.values_mut() {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let year_nodes: Vec<Node> = by_year
        .into_iter()
        .map(|(year, games)| Node {
            name: year.clone(),
            path: format!("/works/{}", year),
            is_dir: true,
            children: Some(games),
            thumbnail: None,
            thumbnail_composite: None,
            meta: None,
        })
        .collect();

    Node {
        name: "works".to_string(),
        path: "/works".to_string(),
        is_dir: true,
        children: Some(year_nodes),
        thumbnail: None,
        thumbnail_composite: None,
        meta: None,
    }
}

pub fn build_app() -> Router {
    // Walk works/ once, parse every markdown file into a ParsedGame. All
    // derived data (creator index, tree JSON for home-page embedding) is built
    // from this single source of truth.
    let games = build_games_index();
    let creator_paths = build_creator_paths(&games);
    let tree = build_tree_from_games(&games);
    let tree_json = serde_json::to_string(&tree).unwrap_or_default();
    // Creator aliases: maps different names for the same person so "More from"
    // sections find games across all their aliases.
    let aliases = load_aliases(include_str!("../config/aliases.yaml"));
    // Tag config: defines colours and optional contest URLs per tag.
    let tag_config = load_tag_config(include_str!("../config/tags.yaml"));
    let state = AppState {
        games: Arc::new(games),
        creator_paths: Arc::new(creator_paths),
        aliases: Arc::new(aliases),
        tag_config: Arc::new(tag_config),
        tree_json: Arc::new(tree_json),
    };

    let serve_dir = ServeDir::new("public").not_found_service(
        ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))),
    );

    // "no-cache" means "cache, but revalidate every time". Combined with the
    // Last-Modified header that ServeDir emits, browsers send conditional
    // requests and get 304 Not Modified (no body) for unchanged static files.
    // Zero stale-content risk; no build-time versioning needed.
    // Applied router-wide: dynamic routes get the header too, but without
    // ETag/Last-Modified the effect is identical to today (full response every
    // request) — no harm, and future ETag support would get free 304s.
    let cache_control = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );

    Router::new()
        .route("/", get(serve_home))
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .nest_service("/raw", ServeDir::new("works"))
        .fallback_service(serve_dir)
        .layer(cache_control)
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn serve_home(State(state): State<AppState>) -> Html<String> {
    let page = include_str!("../public/index.html")
        .replace("{{game_count}}", &state.games.len().to_string())
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
