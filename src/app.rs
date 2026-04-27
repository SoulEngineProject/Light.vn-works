use axum::{
    routing::get,
    routing::get_service,
    Router,
    body::Body,
    extract::Path as AxumPath,
    response::{Html, IntoResponse, Response},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    extract::{Query, State},
};
use dashmap::DashMap;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path as FsPath;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio::sync::Semaphore;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use walkdir::WalkDir;

use crate::{
    GameMeta, ParsedGame, TagInfo, ThumbSize, extract_user_attachment_uuid,
    parse_frontmatter, extract_all_images, markdown_to_html, html_escape, encode_path,
    game_page_suffixes, resize_thumbnail, strip_img_tags, build_creator_paths,
    get_related_paths, gallery_rows, build_tags_line, load_aliases, load_tag_config,
    pick_priority_tag, tag_style, get_lang,
};

// Inlined into <head> on both index.html and game.html so the very first
// frame paints with the dark theme even before external CSS arrives. Without
// this, slow CSS loads (e.g., Render free-tier cold start) cause a white
// flash. Hex values mirror --bg and --text in style.css.
const CRITICAL_CSS: &str = "<style>html,body{background:#0d0b12;color:#ede9fe}</style>";

#[derive(Clone)]
struct AppState {
    games: Arc<HashMap<String, ParsedGame>>,
    creator_paths: Arc<HashMap<String, Vec<String>>>,
    aliases: Arc<HashMap<String, Vec<String>>>,
    tag_config: Arc<HashMap<String, TagInfo>>,
    tree_json: Arc<String>,
    // Thumbnail proxy state
    thumb_cache: Arc<DashMap<(String, ThumbSize), Vec<u8>>>,
    thumb_in_flight: Arc<Mutex<HashSet<(String, ThumbSize)>>>,
    thumb_originals: Arc<HashMap<String, String>>,
    thumb_semaphore: Arc<Semaphore>,
    // Observability: wall-clock time since the first populate was spawned, a
    // running count of successful populates, and cumulative time spent in the
    // GitHub HTTP fetch portion (vs. decode/resize/encode). Lets you see how
    // much of warmup cost is network vs. local CPU work.
    thumb_populate_start: Arc<OnceLock<Instant>>,
    thumb_populate_count: Arc<AtomicUsize>,
    thumb_fetch_millis: Arc<AtomicU64>,
    http_client: reqwest::Client,
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
    thumbnail_ribbon: Option<String>,
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
    let incoming_r18_zero = params.get("r18").map(|s| s.as_str()) == Some("0");

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
    // Back-link (to homepage): forces r18=0 if this page is R18 so the game
    // stays visible in the list. Forward-link (more-from cards): preserves
    // whatever r18 state the user arrived with.
    let (home_suffix, fwd_suffix) = game_page_suffixes(lang_param, is_r18, incoming_r18_zero);
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

    // alt="" intentional: the <h1>{title}</h1> directly below is the accessible
    // label for this page's hero image. Empty alt avoids flashing the title as
    // overlay text during slow loads.
    let hero_html = images.first().map(|img| {
        format!(
            r#"<div class="hero-image"><div class="hero-frame"><img src="{}" alt="" /></div></div>"#,
            html_escape(&img.url)
        )
    }).unwrap_or_default();

    // Gallery layout: max 2 per row. If a trailing single image (orphan)
    // would result, strip it from the gallery — it becomes the editor
    // mockup's source instead via images.last() below. Half of all games
    // (those with even total image count) get a unique editor image this way
    // instead of duplicating the last gallery thumbnail.
    let gallery_count = match images.len() {
        0 | 1 => 0,
        n => {
            let g = n - 1;            // exclude hero
            if g % 2 == 1 { g - 1 }   // strip orphan; promoted to editor mockup
            else { g }
        }
    };

    // alt="" intentional: gallery screenshots are decorative in the context
    // of a page that already has tagline, synopsis, and hero for descriptive
    // content. We have no meaningful per-image description to provide; a
    // generic "Screenshot" adds nothing for screen readers and flashes as
    // overlay text during slow loads.
    let gallery_html = if gallery_count > 0 {
        let gallery_images = &images[1..1 + gallery_count];
        let rows = gallery_rows(gallery_images.len());
        let mut idx = 0;
        let mut html = String::new();
        for cols in &rows {
            html += &format!(r#"<div class="gallery gallery-{}">"#, cols);
            for _ in 0..*cols {
                html += &format!(
                    r#"<img src="{}" alt="" loading="lazy" />"#,
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
            // alt="" intentional: the game page already has <h1>{title_display}</h1>,
            // so this image is decorative. Empty alt avoids flashing the title
            // as overlay text during slow loads.
            format!(
                r#"<img class="editor-preview" src="{}" alt="" loading="lazy" />"#,
                html_escape(&img.url)
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
                            // alt="" intentional: .more-creator-title below is
                            // the link's accessible name; empty alt avoids
                            // flashing title during slow loads.
                            format!(
                                r#"<img src="{}" alt="" loading="lazy" />"#,
                                html_escape(url)
                            )
                        }
                    }).unwrap_or_else(|| r#"<div class="more-creator-placeholder">&#10024;</div>"#.to_string());
                    let tags = g.meta.tags.as_deref().unwrap_or(&[]);
                    // Two-slot layout: priority badge (top-right) + AI (top-left).
                    // See pick_priority_tag() docs for priority order.
                    let mut badge = String::new();
                    if let Some(t) = pick_priority_tag(tags, &state.tag_config) {
                        let style_attr = match tag_style(t, &state.tag_config) {
                            Some(s) => format!(r#" style="{}""#, s),
                            None => String::new(),
                        };
                        badge.push_str(&format!(
                            r#"<span class="card-badge"{}>{}</span>"#,
                            style_attr,
                            html_escape(&t.to_uppercase())
                        ));
                    }
                    if tags.iter().any(|t| t.eq_ignore_ascii_case("ai")) {
                        if let Some(style) = tag_style("ai", &state.tag_config) {
                            badge.push_str(&format!(
                                r#"<span class="card-badge card-badge-left" style="{}">AI</span>"#,
                                style
                            ));
                        }
                    }
                    format!(
                        r#"<a href="{}{}" class="more-creator-card"><div class="more-creator-thumb">{}{}</div><span class="more-creator-title">{}</span></a>"#,
                        html_escape(&encode_path(&g.path)),
                        fwd_suffix,
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
        .replace("{{critical_css}}", CRITICAL_CSS)
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
//
// Also builds the `thumb_originals` map: for each thumbnail that's a GitHub
// user-attachment URL, records (UUID → original URL) so the `/thumb/:uuid/:size`
// handler knows what to fetch/proxy. Thumbnails get their URLs rewritten to
// `/thumb/UUID/{card,ribbon}` form.
fn build_games_index() -> (HashMap<String, ParsedGame>, HashMap<String, String>) {
    let root_dir = FsPath::new("works");
    let mut games = HashMap::new();
    let mut thumb_originals: HashMap<String, String> = HashMap::new();

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
            let original_thumbnail = thumb_img.map(|img| img.url.clone());
            let thumbnail_composite = thumb_img.map_or(false, |img| img.is_composite());

            // Rewrite GitHub user-attachment URLs to the proxy form; pass
            // through anything else unchanged.
            let (thumbnail, thumbnail_ribbon, uuid_to_register) = match original_thumbnail
                .as_deref()
                .and_then(extract_user_attachment_uuid)
            {
                Some(uuid) => (
                    Some(format!("/thumb/{}/card", uuid)),
                    Some(format!("/thumb/{}/ribbon", uuid)),
                    Some((uuid.to_string(), original_thumbnail.clone().unwrap())),
                ),
                None => (original_thumbnail.clone(), original_thumbnail, None),
            };

            let game = ParsedGame {
                year: year.clone(),
                title: title.clone(),
                path: canonical_path.clone(),
                meta,
                body_html,
                images,
                thumbnail,
                thumbnail_ribbon,
                thumbnail_composite,
            };
            (game, uuid_to_register)
        }));

        match parsed {
            Ok((game, uuid_to_register)) => {
                if let Some((uuid, orig)) = uuid_to_register {
                    thumb_originals.insert(uuid, orig);
                }
                games.insert(canonical_path, game);
            }
            Err(_) => {
                eprintln!("[startup] panic parsing {}; skipping", path.display());
            }
        }
    }

    (games, thumb_originals)
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
            thumbnail_ribbon: game.thumbnail_ribbon.clone(),
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
            thumbnail_ribbon: None,
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
        thumbnail_ribbon: None,
        thumbnail_composite: None,
        meta: None,
    }
}

// Thumbnail proxy handler.
// - Cache hit: serve resized JPEG bytes from memory with aggressive caching.
// - Cache miss: respond 302 to the original GitHub URL (no-store so the
//   browser re-hits us once cache is warm), and spawn a background populate
//   task if one isn't already running for this (uuid, size).
async fn serve_thumb(
    State(state): State<AppState>,
    AxumPath((uuid, size_str)): AxumPath<(String, String)>,
) -> Response {
    let size = match ThumbSize::parse(&size_str) {
        Some(s) => s,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let original_url = match state.thumb_originals.get(&uuid) {
        Some(url) => url.clone(),
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let key = (uuid.clone(), size);

    if let Some(bytes) = state.thumb_cache.get(&key) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/webp")
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(Body::from(bytes.clone()))
            .unwrap();
    }

    // Miss path. Debounce: only spawn a populate if this (uuid, size) isn't
    // already in flight. Prevents thundering herd on first visit.
    let should_spawn = {
        let mut in_flight = state.thumb_in_flight.lock().unwrap();
        in_flight.insert(key.clone())
    };
    if should_spawn {
        let state_clone = state.clone();
        tokio::spawn(populate_thumbnail(state_clone, key, original_url.clone()));
    }

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, original_url)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::empty())
        .unwrap()
}

// Fetch the original from GitHub, decode, resize, re-encode as JPEG q=80,
// insert into cache. Semaphore caps concurrent populates to avoid saturating
// free-tier CPU when many misses arrive in a burst. Failures are logged and
// the in-flight slot released so the next miss retries.
async fn populate_thumbnail(
    state: AppState,
    key: (String, ThumbSize),
    original_url: String,
) {
    // Record the first populate's start time for cumulative progress logging.
    // OnceLock::set is a no-op after the first call.
    let _ = state.thumb_populate_start.set(Instant::now());
    let _permit = state.thumb_semaphore.acquire().await.ok();
    let (uuid, size) = key.clone();

    let result = async {
        let fetch_start = Instant::now();
        // One retry on transient network errors. "Connection closed before
        // message complete" is the common flake from HTTP/2 pooled connections
        // being reused as the server-side closes them; a retry almost always
        // succeeds. Covers errors from both send() and body read. 4xx/5xx
        // responses aren't retried (they're not transient).
        let fetch_once = || async {
            state
                .http_client
                .get(&original_url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await
        };
        let bytes = match fetch_once().await {
            Ok(b) => b,
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                fetch_once().await?
            }
        };
        state
            .thumb_fetch_millis
            .fetch_add(fetch_start.elapsed().as_millis() as u64, Ordering::Relaxed);

        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode: {}", e))?;
        let resized = resize_thumbnail(&img, size);
        // WebP q=80 lossy via libwebp. Smaller than JPEG at equivalent visual
        // quality, and preserves alpha channels (JPEG would flatten them).
        let out: Vec<u8> = if resized.color().has_alpha() {
            let rgba = resized.to_rgba8();
            webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
                .encode(80.0)
                .to_vec()
        } else {
            let rgb = resized.to_rgb8();
            webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height())
                .encode(80.0)
                .to_vec()
        };
        Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(out)
    }
    .await;

    match result {
        Ok(bytes) => {
            state.thumb_cache.insert(key.clone(), bytes);
            let count = state.thumb_populate_count.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(start) = state.thumb_populate_start.get() {
                let secs = start.elapsed().as_secs_f32();
                let fetch_ms = state.thumb_fetch_millis.load(Ordering::Relaxed);
                let avg_fetch_secs = (fetch_ms as f32 / count as f32) / 1000.0;
                let total = state.thumb_originals.len() * 2;
                eprintln!(
                    "[thumb] {} / {} images cached. Took {:.1}s (avg fetch {:.1}s/img)",
                    count, total, secs, avg_fetch_secs
                );
            }
        }
        Err(e) => {
            // Walk the error's source chain so we see the underlying cause
            // (connect/DNS/TLS/timeout/etc.) not just reqwest's wrapper.
            let mut msg = e.to_string();
            let mut src: Option<&(dyn std::error::Error + 'static)> = e.source();
            while let Some(inner) = src {
                msg.push_str(" | ");
                msg.push_str(&inner.to_string());
                src = inner.source();
            }
            eprintln!("[thumb] populate failed for {}/{:?}: {}", uuid, size, msg);
        }
    }
    state.thumb_in_flight.lock().unwrap().remove(&key);
}

// Background warmup: spawn a populate for every (uuid, size) pair that isn't
// already cached. Reuses populate_thumbnail + in-flight debouncing, so races
// with user requests are harmless (either the warmer or the user spawns the
// task, never both). Semaphore throttles actual concurrency; spawning all
// tasks up front just queues them.
async fn warm_all_thumbnails(state: AppState) {
    for (uuid, original_url) in state.thumb_originals.iter() {
        for size in [ThumbSize::Card, ThumbSize::Ribbon] {
            let key = (uuid.clone(), size);
            if state.thumb_cache.contains_key(&key) {
                continue;
            }
            let should_spawn = {
                let mut in_flight = state.thumb_in_flight.lock().unwrap();
                in_flight.insert(key.clone())
            };
            if should_spawn {
                let state_clone = state.clone();
                let url_clone = original_url.clone();
                tokio::spawn(populate_thumbnail(state_clone, key, url_clone));
            }
        }
    }
}

pub fn build_app() -> Router {
    // Walk works/ once, parse every markdown file into a ParsedGame. All
    // derived data (creator index, tree JSON for home-page embedding) is built
    // from this single source of truth.
    let (games, thumb_originals) = build_games_index();
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
        thumb_cache: Arc::new(DashMap::new()),
        thumb_in_flight: Arc::new(Mutex::new(HashSet::new())),
        thumb_originals: Arc::new(thumb_originals),
        thumb_semaphore: Arc::new(Semaphore::new(8)),
        thumb_populate_start: Arc::new(OnceLock::new()),
        thumb_populate_count: Arc::new(AtomicUsize::new(0)),
        thumb_fetch_millis: Arc::new(AtomicU64::new(0)),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            // Drop idle pooled connections sooner than GitHub's ~60s. Reusing
            // a stale HTTP/2 connection is the typical cause of
            // "connection closed before message complete" errors.
            .pool_idle_timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("build reqwest client"),
    };

    // Kick off background warmup. Runs concurrently with request handling;
    // server is already listening by the time the spawned task progresses.
    tokio::spawn(warm_all_thumbnails(state.clone()));

    let serve_dir = ServeDir::new("public").not_found_service(
        ServeDir::new("public").fallback(get_service(axum::routing::get(handler_404))),
    );

    // "no-cache" means "cache, but revalidate every time". Combined with the
    // Last-Modified header that ServeDir emits, browsers send conditional
    // requests and get 304 Not Modified (no body) for unchanged static files.
    // Zero stale-content risk; no build-time versioning needed.
    //
    // `if_not_present` (not `overriding`) so handlers that set their own
    // Cache-Control keep theirs — notably /thumb/:uuid/:size uses
    // `immutable` since UUID-keyed URLs never change.
    let cache_control = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );

    Router::new()
        .route("/", get(serve_home))
        .route("/api/tree", get(get_tree))
        .route("/works/:year/:title", get(render_markdown))
        .route("/thumb/:uuid/:size", get(serve_thumb))
        .nest_service("/raw", ServeDir::new("works"))
        .fallback_service(serve_dir)
        .layer(cache_control)
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn serve_home(State(state): State<AppState>) -> Html<String> {
    let page = include_str!("../public/index.html")
        .replace("{{critical_css}}", CRITICAL_CSS)
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
