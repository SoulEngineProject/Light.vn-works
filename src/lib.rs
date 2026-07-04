pub mod app;

use pulldown_cmark::{html, Event, Parser};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub const RELEASED_UNKNOWN: &str = "unknown";

#[derive(Debug)]
pub struct LangStrings {
    pub more_from: String,
    pub share: String,
    pub copied: String,
    pub footer: String,
    pub breadcrumb_works: String,
    pub engine_url: String,
    pub tags_label: String,
    pub dev_example: String,
    pub creator_active_since: String,
    pub creator_latest: String,
    pub creator_more_works: String,
    pub creator_view: String,
    pub creator_all_works: String,
}

struct LangPair {
    en: LangStrings,
    ja: LangStrings,
}

static LANG: OnceLock<LangPair> = OnceLock::new();

fn load_lang() -> &'static LangPair {
    LANG.get_or_init(|| {
        let raw: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(include_str!("../config/lang.json"))
                .expect("Failed to parse lang.json");

        fn extract(raw: &HashMap<String, HashMap<String, String>>, lang: &str) -> LangStrings {
            let get = |key: &str| -> String {
                raw.get(key)
                    .and_then(|m| m.get(lang))
                    .cloned()
                    .unwrap_or_default()
            };
            LangStrings {
                more_from: get("more_from"),
                share: get("share"),
                copied: get("copied"),
                footer: get("footer"),
                breadcrumb_works: get("breadcrumb_works"),
                engine_url: get("engine_url"),
                tags_label: get("tags_label"),
                dev_example: get("dev_example"),
                creator_active_since: get("creator_active_since"),
                creator_latest: get("creator_latest"),
                creator_more_works: get("creator_more_works"),
                creator_view: get("creator_view"),
                creator_all_works: get("creator_all_works"),
            }
        }

        LangPair {
            en: extract(&raw, "en"),
            ja: extract(&raw, "ja"),
        }
    })
}

pub fn get_lang(lang: &str) -> &'static LangStrings {
    let lang_data = load_lang();
    if lang.contains("ja") {
        &lang_data.ja
    } else {
        &lang_data.en
    }
}

/// - Resolve the display language: an explicit `lang` param wins ("ja"/"en"),
///   else the Accept-Language header, else English.
pub fn detect_lang(lang_param: Option<&str>, accept_language: Option<&str>) -> &'static str {
    match lang_param {
        Some("ja") => "ja",
        Some("en") => "en",
        _ => {
            if accept_language.unwrap_or("en").contains("ja") {
                "ja"
            } else {
                "en"
            }
        }
    }
}

/// - Sort key for ordering a creator's works newest-first.
/// - Uses the release date, falling back to the folder year when the date is
///   missing, empty, or "unknown" — so an undated work sorts by its year rather
///   than jumping to the top (e.g. as the hero).
pub fn creator_work_key<'a>(released: Option<&'a str>, folder_year: &'a str) -> &'a str {
    match released {
        Some(r) if !r.is_empty() && r != RELEASED_UNKNOWN => r,
        _ => folder_year,
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct GameMeta {
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default)]
    pub date_added: Option<String>,
    #[serde(default)]
    pub link_label: Option<String>,
    #[serde(default)]
    pub link_url: Option<String>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub extra_links: Option<Vec<ExtraLink>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub thumbnail_index: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ExtraLink {
    pub label: String,
    pub url: String,
}

/// - Split YAML frontmatter from markdown body.
/// - Returns (parsed meta, body without frontmatter).
pub fn parse_frontmatter(content: &str) -> (GameMeta, &str) {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return (GameMeta::default(), content);
    }

    let after_open = &trimmed[3..];
    let after_open = after_open.trim_start_matches(['\r', '\n']);

    if let Some(close_idx) = after_open.find("\n---") {
        let yaml_str = &after_open[..close_idx];
        let body_start = close_idx + 4;
        let body = after_open[body_start..].trim_start_matches(['\r', '\n']);

        match serde_yaml::from_str::<GameMeta>(yaml_str) {
            Ok(meta) => (meta, body),
            Err(_) => (GameMeta::default(), content),
        }
    } else {
        (GameMeta::default(), content)
    }
}

#[derive(Clone, Debug)]
pub struct ImageInfo {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl ImageInfo {
    /// Returns true if the image is a composite strip (see `is_composite_dimensions`).
    pub fn is_composite(&self) -> bool {
        match (self.width, self.height) {
            (Some(w), Some(h)) => is_composite_dimensions(w, h),
            _ => false,
        }
    }
}

/// - A composite thumbnail is a wide-aspect strip (width more than 2× height),
///   typically a side-by-side title screen.
/// - The site renders these with CSS `background-size: 340%` + center crop;
///   preserving that wide aspect matters at every step (resize, detection,
///   rendering).
/// - Single source of truth for the threshold so a future tweak only changes
///   one place.
pub fn is_composite_dimensions(width: u32, height: u32) -> bool {
    height > 0 && width > height * 2
}

/// Resize a decoded image for the thumbnail proxy.
///
/// - **Normal thumbnails**: `resize_to_fill` to the target dimensions, cleanly
///   filling the card/ribbon area without letterboxing.
/// - **Composites** (wide-aspect strips): rendered via CSS `background-size:
///   340%` zoom/crop, which needs enough source resolution to survive retina
///   + zoom without upsampling blur. Uses wider composite-specific targets
///     and *never upscales* — if the source already fits, it's kept as-is.
///
/// Triangle filter is 2–4× faster than Lanczos3 with imperceptible quality
/// loss at thumbnail sizes.
pub fn resize_thumbnail(img: &image::DynamicImage, size: ThumbSize) -> image::DynamicImage {
    let filter = image::imageops::FilterType::Triangle;
    if is_composite_dimensions(img.width(), img.height()) {
        let (tw, th) = match size {
            ThumbSize::Ribbon => (900, 400),
            ThumbSize::Card => (1600, 400),
        };
        if img.width() <= tw && img.height() <= th {
            img.clone()
        } else {
            img.resize(tw, th, filter)
        }
    } else {
        let (w, h) = size.dimensions();
        img.resize_to_fill(w, h, filter)
    }
}

fn extract_attr_u32(tag: &str, attr: &str) -> Option<u32> {
    let needle = format!("{}=\"", attr);
    let start = tag.find(&needle)? + needle.len();
    let end = start + tag[start..].find('"')?;
    tag[start..end].parse().ok()
}

pub fn extract_all_images(md: &str) -> Vec<ImageInfo> {
    let parser = Parser::new(md);
    let mut images = Vec::new();

    for event in parser {
        if let Event::Html(html) = event {
            let html_str = html.to_string();
            let mut search_from = 0;
            while let Some(src_start) =
                html_str[search_from..].find("src=\"https://github.com/user-attachments/")
            {
                let abs_start = search_from + src_start + 5;
                if let Some(end_quote) = html_str[abs_start..].find('\"') {
                    let url = html_str[abs_start..abs_start + end_quote].to_string();
                    // Find the tag boundaries to extract width/height
                    let tag_start = html_str[..search_from + src_start].rfind('<').unwrap_or(0);
                    let tag_end = abs_start
                        + end_quote
                        + html_str[abs_start + end_quote..].find('>').unwrap_or(0)
                        + 1;
                    let tag = &html_str[tag_start..tag_end];
                    images.push(ImageInfo {
                        url,
                        width: extract_attr_u32(tag, "width"),
                        height: extract_attr_u32(tag, "height"),
                    });
                    search_from = abs_start + end_quote;
                } else {
                    break;
                }
            }
        }
    }

    images
}

/// - Return the first image source in a markdown body that is NOT a GitHub
///   user-attachment URL, if any.
/// - Covers the sinks contributor content actually uses: `<img src>` / `srcset`
///   attributes (case- and whitespace-tolerant) and markdown `![](url)`.
/// - CI lint against a tracking-pixel PR, run over every works file. It's
///   defense-in-depth with the CSP `img-src` allowlist, not a hard boundary —
///   a browser parses more image sinks than this scan does, so the CSP stays
///   the enforcement backstop.
pub fn first_offsite_image(body: &str) -> Option<String> {
    const OK: &str = "https://github.com/user-attachments/";
    let lower = body.to_lowercase();
    let b = lower.as_bytes();
    let n = b.len();

    let flag = |url: &str| -> Option<String> {
        let url = url.trim();
        (!url.is_empty() && !url.starts_with(OK)).then(|| url.to_string())
    };

    // Read the value of an attribute whose '=' is at `eq`; handles quoted and
    // bare forms. Returns the raw value.
    let read_value = |eq: usize| -> Option<&str> {
        let mut j = eq + 1;
        while j < n && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= n {
            return None;
        }
        if b[j] == b'"' || b[j] == b'\'' {
            let q = b[j];
            let start = j + 1;
            let end = start + lower[start..].find(q as char)?;
            Some(&lower[start..end])
        } else {
            let start = j;
            let end = start
                + lower[start..]
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(lower.len() - start);
            Some(&lower[start..end])
        }
    };

    for (i, _) in lower.char_indices() {
        // markdown image: ![alt](url "optional title")
        if b[i] == b'!' && lower[i..].starts_with("![") {
            if let Some(rel) = lower[i..].find("](") {
                let us = i + rel + 2;
                if let Some(e) = lower[us..].find(')') {
                    let url = lower[us..us + e].split_whitespace().next().unwrap_or("");
                    if let Some(bad) = flag(url) {
                        return Some(bad);
                    }
                }
            }
        }

        // src= / srcset= attribute, at an attribute-name boundary (so
        // "described" and "data-src" don't match).
        let boundary = i == 0 || matches!(b[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b'<' | b'/');
        if boundary && lower[i..].starts_with("src") {
            let mut j = i + 3;
            if lower[j..].starts_with("set") {
                j += 3;
            }
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < n && b[j] == b'=' {
                if let Some(val) = read_value(j) {
                    // srcset is a comma-separated "url descriptor" list.
                    for cand in val.split(',') {
                        let url = cand.split_whitespace().next().unwrap_or("");
                        if let Some(bad) = flag(url) {
                            return Some(bad);
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn markdown_to_html(md_content: &str) -> String {
    let mut html_output = String::new();
    let parser = Parser::new(md_content);
    html::push_html(&mut html_output, parser);
    html_output
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// - Percent-encode the characters that could terminate a CSS `url('…')`
///   token or its wrapping style attribute: quotes, parens, backslash, space.
/// - HTML entities don't survive into CSS — the HTML parser decodes them
///   before the CSS engine reads the style attribute, so `&#39;` turns back
///   into a breakout quote. `%27` survives both parsers and the URL still
///   resolves (percent-encoding is transparent to the server).
/// - `%` itself is deliberately NOT encoded: source URLs are often already
///   percent-encoded and double-encoding would break them.
pub fn escape_css_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            '\'' => out.push_str("%27"),
            '"' => out.push_str("%22"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '\\' => out.push_str("%5C"),
            ' ' => out.push_str("%20"),
            _ => out.push(c),
        }
    }
    out
}

/// - Make serialized JSON safe to embed in an inline `<script>`: the HTML
///   parser ends the script element at the first "</" regardless of JSON
///   string context, and serde_json doesn't escape '<'.
/// - "<\/" is identical to "</" inside a JSON string, so parsing is unchanged.
pub fn json_script_escape(s: &str) -> String {
    s.replace("</", "<\\/")
}

/// - Percent-encode reserved URL characters in a path.
/// - Preserves '/' (so callers can pass "/works/2021/title") and encodes
///   everything else that's not an unreserved character per RFC 3986.
/// - Titles starting with '#' or containing '?' would otherwise be mis-parsed
///   by the browser.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

/// - Build a URL query string from (key, value) pairs.
/// - Empty values are filtered out.
/// - Returns "" for no non-empty pairs, or "?k1=v1&k2=v2".
pub fn build_query(params: &[(&str, &str)]) -> String {
    let parts: Vec<String> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// - Build an XML sitemap listing the home page and every game URL.
/// - `base_url` is scheme+host without a trailing slash (e.g. https://example.com).
/// - `game_paths` are canonical paths ("/works/YYYY/title"); each segment is
///   percent-encoded and paths are sorted for deterministic output.
/// - No `<lastmod>`: our only date is the release date, which never updates when a
///   page is edited, so it would misreport freshness (see docs/seo.md).
pub fn build_sitemap(base_url: &str, game_paths: &[String]) -> String {
    let base = base_url.trim_end_matches('/');
    let mut sorted: Vec<&String> = game_paths.iter().collect();
    sorted.sort();

    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    out.push_str(&format!("  <url><loc>{}/</loc></url>\n", html_escape(base)));
    for path in sorted {
        let loc = format!("{}{}", base, encode_path(path));
        out.push_str(&format!("  <url><loc>{}</loc></url>\n", html_escape(&loc)));
    }
    out.push_str("</urlset>\n");
    out
}

/// - Convert a `YYYY/MM/DD` (or `YYYY/MM`, `YYYY`) date to ISO `YYYY-MM-DD`, zero-padded.
/// - Returns None for empty, RELEASED_UNKNOWN, or malformed input.
pub fn released_to_iso(date: &str) -> Option<String> {
    let s = date.trim();
    if s.is_empty() || s == RELEASED_UNKNOWN {
        return None;
    }
    let mut parts = s.split('/');

    let year = parts.next()?;
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut out = year.to_string();

    if let Some(month) = parts.next() {
        let m: u32 = month.parse().ok()?;
        if !(1..=12).contains(&m) {
            return None;
        }
        out.push_str(&format!("-{:02}", m));

        if let Some(day) = parts.next() {
            let d: u32 = day.parse().ok()?;
            if !(1..=31).contains(&d) {
                return None;
            }
            out.push_str(&format!("-{:02}", d));
        }
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

/// - Whether a frontmatter `released` value is catalog-canonical: "unknown",
///   or zero-padded `YYYY/MM/DD` optionally followed by a non-digit suffix
///   (e.g. "2014/09/15～ (連載作品)" for serialized works).
/// - Sorts compare `released` strings lexicographically (build_creator_paths,
///   creator_work_key, the homepage year sort), which is only correct when
///   zero-padded; the works validator uses this so unpadded dates fail CI.
/// - Positional byte checks, not `&s[..10]` — slicing panics when a multibyte
///   char straddles byte 10 (malformed input like "2024/09/1あ").
pub fn is_canonical_released(s: &str) -> bool {
    if s == RELEASED_UNKNOWN {
        return true;
    }
    let b = s.as_bytes();
    if b.len() < 10 {
        return false;
    }
    let all_digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    if !(all_digits(0..4) && b[4] == b'/' && all_digits(5..7) && b[7] == b'/' && all_digits(8..10))
    {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    let day = (b[8] - b'0') * 10 + (b[9] - b'0');
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }
    // A suffix may follow the date, but not another digit ("2024/09/155").
    !matches!(b.get(10), Some(c) if c.is_ascii_digit())
}

/// - The date a work is ordered/timestamped by in the feed.
/// - Prefers `date_added` (when to the site), falling back to `released`.
/// - Returns ISO `YYYY-MM-DD`, or None when neither is a valid date.
pub fn feed_date(meta: &GameMeta) -> Option<String> {
    meta.date_added
        .as_deref()
        .and_then(released_to_iso)
        .or_else(|| meta.released.as_deref().and_then(released_to_iso))
}

/// One entry in the Atom feed.
pub struct FeedEntry {
    pub title: String,
    pub path: String,    // canonical path "/works/YYYY/title"
    pub summary: String, // tagline (may be empty)
    pub updated: String, // ISO date "YYYY-MM-DD"
}

/// - Build an Atom 1.0 feed from entries already sorted newest-first.
/// - `base_url` is scheme+host without a trailing slash.
/// - Links are absolute (base + percent-encoded path); dates become RFC-3339
///   `{updated}T00:00:00Z`. Feed-level `<updated>` is the newest entry's date.
pub fn build_atom_feed(base_url: &str, entries: &[FeedEntry]) -> String {
    let base = base_url.trim_end_matches('/');
    let feed_updated = entries
        .first()
        .map(|e| e.updated.as_str())
        .unwrap_or("1970-01-01");

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    out.push_str("  <title>Light.vn Works</title>\n");
    out.push_str("  <author><name>Light.vn Works</name></author>\n");
    out.push_str(&format!("  <link href=\"{}/\"/>\n", html_escape(base)));
    out.push_str(&format!(
        "  <link rel=\"self\" href=\"{}/feed.xml\"/>\n",
        html_escape(base)
    ));
    out.push_str(&format!("  <id>{}/</id>\n", html_escape(base)));
    out.push_str(&format!(
        "  <updated>{}T00:00:00Z</updated>\n",
        html_escape(feed_updated)
    ));

    for entry in entries {
        let loc = format!("{}{}", base, encode_path(&entry.path));
        out.push_str("  <entry>\n");
        out.push_str(&format!(
            "    <title>{}</title>\n",
            html_escape(&entry.title)
        ));
        out.push_str(&format!("    <link href=\"{}\"/>\n", html_escape(&loc)));
        out.push_str(&format!("    <id>{}</id>\n", html_escape(&loc)));
        out.push_str(&format!(
            "    <updated>{}T00:00:00Z</updated>\n",
            html_escape(&entry.updated)
        ));
        if !entry.summary.is_empty() {
            out.push_str(&format!(
                "    <summary>{}</summary>\n",
                html_escape(&entry.summary)
            ));
        }
        out.push_str("  </entry>\n");
    }
    out.push_str("</feed>\n");
    out
}

/// - Compute the breadcrumb-back suffix and the forward-link suffix for a game
///   page.
/// - Both propagate `lang`.
/// - Back-suffix forces `r18=0` if this page is R18 (so the homepage shows it);
///   forward-suffix only carries `r18=0` if the incoming request had it.
pub fn game_page_suffixes(
    lang_param: Option<&str>,
    is_r18: bool,
    incoming_r18_zero: bool,
) -> (String, String) {
    let lang = lang_param.unwrap_or("");
    let back = build_query(&[
        ("lang", lang),
        ("r18", if is_r18 || incoming_r18_zero { "0" } else { "" }),
    ]);
    let fwd = build_query(&[
        ("lang", lang),
        ("r18", if incoming_r18_zero { "0" } else { "" }),
    ]);
    (back, fwd)
}

/// - A parsed markdown game file.
/// - Sole source of truth for game data in-memory.
#[derive(Clone, Debug)]
pub struct ParsedGame {
    pub year: String,  // directory name
    pub title: String, // file stem, no .md
    pub path: String,  // "/works/YYYY/title", no .md
    pub meta: GameMeta,
    pub body_html: String, // pre-rendered markdown
    pub images: Vec<ImageInfo>,
    pub thumbnail: Option<String>, // card-size URL: "/thumb/UUID/card" or passthrough
    pub thumbnail_ribbon: Option<String>, // ribbon-size URL: "/thumb/UUID/ribbon" or passthrough
    pub thumbnail_composite: bool,
}

/// - Size variant for the thumbnail proxy.
/// - Rendered dimensions are 2× display size for retina screens.
/// - The actual encoded output is JPEG q=80.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThumbSize {
    Ribbon,
    Card,
}

impl ThumbSize {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ribbon" => Some(Self::Ribbon),
            "card" => Some(Self::Card),
            _ => None,
        }
    }

    /// Target dimensions (width, height) for resize_to_fill.
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Ribbon => (240, 140),
            Self::Card => (600, 400),
        }
    }
}

/// - Extract the UUID from a GitHub user-attachment URL.
/// - `https://github.com/user-attachments/assets/<UUID>` -> Some("<UUID>").
/// - Returns None for any other URL shape (non-GitHub hosts, different paths, etc).
pub fn extract_user_attachment_uuid(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://github.com/user-attachments/assets/")?;
    if rest.is_empty() || rest.contains('/') || rest.contains('?') || rest.contains('#') {
        None
    } else {
        Some(rest)
    }
}

/// Split a creator field into individual creator names.
pub fn split_creators(creator: &str) -> Vec<String> {
    creator
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// - Build creator → paths index.
/// - Paths are sorted by release date descending (unknown last).
/// - Creators with commas are split into separate entries.
pub fn build_creator_paths(games: &HashMap<String, ParsedGame>) -> HashMap<String, Vec<String>> {
    let mut index: HashMap<String, Vec<String>> = HashMap::new();

    for game in games.values() {
        let creator = match game.meta.creator.as_deref() {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        for name in split_creators(creator) {
            index
                .entry(name.to_lowercase())
                .or_default()
                .push(game.path.clone());
        }
    }

    // Sort each creator's paths by release date (newest first, "unknown" last)
    for paths in index.values_mut() {
        paths.sort_by(|a, b| {
            let a_date = released_for_sort(games.get(a));
            let b_date = released_for_sort(games.get(b));
            b_date.cmp(a_date)
        });
    }

    index
}

fn released_for_sort(game: Option<&ParsedGame>) -> &str {
    match game.and_then(|g| g.meta.released.as_deref()) {
        Some(r) if r != RELEASED_UNKNOWN => r,
        _ => "",
    }
}

/// - Get related paths by the same creator(s), excluding the current path.
/// - Returns (creator_name, paths) pairs for each creator that has other games.
pub fn get_related_paths<'a>(
    index: &'a HashMap<String, Vec<String>>,
    creator_field: &str,
    current_path: &str,
    limit: usize,
    aliases: &HashMap<String, Vec<String>>,
) -> Vec<(String, Vec<&'a str>)> {
    if creator_field.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    seen_paths.insert(current_path.to_string());

    // Collect all names to look up: direct creators + their aliases
    let mut names_to_check: Vec<String> = Vec::new();
    for name in split_creators(creator_field) {
        let lower = name.to_lowercase();
        if seen_names.insert(lower.clone()) {
            names_to_check.push(name);
        }
        if let Some(alias_list) = aliases.get(&lower) {
            for alias in alias_list {
                let alias_lower = alias.to_lowercase();
                if seen_names.insert(alias_lower) {
                    names_to_check.push(alias.clone());
                }
            }
        }
    }

    for name in names_to_check {
        if let Some(paths) = index.get(&name.to_lowercase()) {
            let related: Vec<&str> = paths
                .iter()
                .map(|s| s.as_str())
                .filter(|p| !seen_paths.contains(*p))
                .take(limit)
                .collect();

            if !related.is_empty() {
                for p in &related {
                    seen_paths.insert(p.to_string());
                }
                result.push((name, related));
            }
        }
    }

    result
}

/// - A creator's links (homepage / socials / shop / store), gathered from all
///   their games (primary + extras).
/// - One button per label: deduped by label (case-insensitive), keeping the
///   first-seen version. `metas` arrive newest-first, so a repeated label (e.g.
///   several "itch.io" works) resolves to the newest game that carries it.
/// - "HP" (homepage) leads when present.
pub fn aggregate_creator_links(metas: &[&GameMeta]) -> Vec<ExtraLink> {
    let mut seen_label: HashSet<String> = HashSet::new();
    let mut links: Vec<ExtraLink> = Vec::new();

    for meta in metas {
        // Collect this game's links (primary + extras) in order.
        let mut game_links: Vec<ExtraLink> = Vec::new();
        if let (Some(label), Some(url)) = (meta.link_label.as_deref(), meta.link_url.as_deref()) {
            if !url.is_empty() {
                game_links.push(ExtraLink {
                    label: label.to_string(),
                    url: url.to_string(),
                });
            }
        }
        if let Some(extras) = &meta.extra_links {
            for l in extras {
                if !l.url.is_empty() {
                    game_links.push(l.clone());
                }
            }
        }

        for l in game_links {
            if seen_label.insert(l.label.to_lowercase()) {
                links.push(l);
            }
        }
    }

    // - Always lead with the creator's homepage ("HP") when present.
    // - Stable sort, so the remaining links keep their newest-first order.
    links.sort_by_key(|l| {
        if l.label.eq_ignore_ascii_case("hp") {
            0
        } else {
            1
        }
    });
    links
}

/// - Compute gallery row sizes — max 2 per row.
/// - Bigger thumbnails for screenshot detail.
/// - Orphan (single trailing image) is handled upstream by promoting it to the
///   editor mockup, so this function is typically called with even `n` in
///   production.
pub fn gallery_rows(n: usize) -> Vec<usize> {
    if n == 0 {
        return vec![];
    }
    let mut rows = vec![2; n / 2];
    if n % 2 == 1 {
        rows.push(1);
    }
    rows
}

/// Info for a special tag (colour, optional link).
#[derive(Clone, Debug)]
pub struct TagInfo {
    pub colour: String,
    /// - Original yaml casing (e.g. "Terrace and Ray").
    /// - Map keys are lowercased for case-insensitive lookup; this preserves the
    ///   canonical display form.
    pub display_name: String,
    pub url: Option<String>,
    pub label: Option<String>,
    /// - Whether this tag is eligible for the card's priority (right-slot) badge.
    /// - `false` means the tag exists for filtering/discovery (e.g. languages,
    ///   AI which has its own dedicated left slot) but should not promote into
    ///   the priority cascade.
    /// - Defaults to `true` when the yaml omits it.
    pub card_priority_badge: bool,
}

/// Parse tag config YAML into a map of lowercased tag name → TagInfo.
pub fn load_tag_config(yaml: &str) -> HashMap<String, TagInfo> {
    #[derive(Deserialize)]
    struct RawConfig {
        #[serde(default)]
        colours: HashMap<String, String>,
        #[serde(default)]
        tags: Vec<RawTagGroup>,
    }
    #[derive(Deserialize)]
    struct RawTagGroup {
        colour: String,
        tags: Vec<String>,
        url: Option<String>,
        label: Option<String>,
        card_priority_badge: Option<bool>,
    }
    let config: RawConfig = serde_yaml::from_str(yaml).unwrap_or(RawConfig {
        colours: HashMap::new(),
        tags: Vec::new(),
    });
    let mut map = HashMap::new();
    for group in config.tags {
        let resolved_colour = config
            .colours
            .get(&group.colour)
            .cloned()
            .unwrap_or(group.colour.clone());
        let card_priority_badge = group.card_priority_badge.unwrap_or(true);
        for tag in &group.tags {
            map.insert(
                tag.to_lowercase(),
                TagInfo {
                    colour: resolved_colour.clone(),
                    display_name: tag.clone(),
                    url: group.url.clone(),
                    label: group.label.clone(),
                    card_priority_badge,
                },
            );
        }
    }
    map
}

/// One row of the homepage tag-filter bar.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TagBarEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    pub count: usize,
}

/// - Build the tag-filter bar entries: union of yaml-configured tags and tags
///   found in game frontmatter, deduped case-insensitively.
/// - Counts are total games per tag (not affected by R18 toggle or current
///   search).
/// - `r18` is excluded — already covered by the dedicated toggle.
/// - Sort: count desc, then name asc (case-insensitive).
/// - Configured tags use the yaml display casing; unconfigured (md-only) tags
///   use first-seen casing.
pub fn build_tag_index(
    games: &HashMap<String, ParsedGame>,
    config: &HashMap<String, TagInfo>,
) -> Vec<TagBarEntry> {
    struct Row {
        display: String,
        colour: Option<String>,
        count: usize,
    }
    let mut rows: HashMap<String, Row> = HashMap::new();

    for (lower, info) in config {
        if lower == "r18" {
            continue;
        }
        rows.insert(
            lower.clone(),
            Row {
                display: info.display_name.clone(),
                colour: Some(info.colour.clone()),
                count: 0,
            },
        );
    }

    for game in games.values() {
        let tags = match &game.meta.tags {
            Some(t) => t,
            None => continue,
        };
        let mut seen_in_game: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tag in tags {
            let lower = tag.to_lowercase();
            if lower == "r18" || !seen_in_game.insert(lower.clone()) {
                continue;
            }
            let entry = rows.entry(lower).or_insert_with(|| Row {
                display: tag.clone(),
                colour: None,
                count: 0,
            });
            entry.count += 1;
        }
    }

    let mut out: Vec<TagBarEntry> = rows
        .into_values()
        .map(|r| TagBarEntry {
            name: r.display,
            colour: r.colour,
            count: r.count,
        })
        .collect();

    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    out
}

/// Get inline style for a tag badge, or None if unknown.
pub fn tag_style(tag: &str, tag_config: &HashMap<String, TagInfo>) -> Option<String> {
    tag_config
        .get(&tag.to_lowercase())
        .map(|info| format!("background:{};color:white", info.colour))
}

/// Pick the tag for the top-right priority badge slot. Priority order:
///   1. R18 (content warning)
///   2. "Terrace and Ray" (publisher identity)
///   3. First other configured tag whose group has `card_priority_badge: true`
///   4. None — no fallback to non-configured tags. Empty slot is acceptable.
///
/// Tags whose group sets `card_priority_badge: false` (AI, languages) exist
/// for filtering only and never promote into this slot — AI has its own
/// dedicated top-left slot, languages are metadata, not identity.
pub fn pick_priority_tag<'a>(
    tags: &'a [String],
    config: &HashMap<String, TagInfo>,
) -> Option<&'a str> {
    if let Some(t) = tags.iter().find(|t| t.eq_ignore_ascii_case("r18")) {
        return Some(t.as_str());
    }
    if let Some(t) = tags
        .iter()
        .find(|t| t.eq_ignore_ascii_case("terrace and ray"))
    {
        return Some(t.as_str());
    }
    if let Some(t) = tags.iter().find(|t| {
        config
            .get(&t.to_lowercase())
            .is_some_and(|info| info.card_priority_badge)
    }) {
        return Some(t.as_str());
    }
    None
}

/// Parse alias groups YAML into a bidirectional lookup map (lowercased).
pub fn load_aliases(yaml: &str) -> HashMap<String, Vec<String>> {
    let groups: Vec<Vec<String>> = serde_yaml::from_str(yaml).unwrap_or_default();
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for group in &groups {
        for name in group {
            let others: Vec<String> = group
                .iter()
                .filter(|n| n.to_lowercase() != name.to_lowercase())
                .cloned()
                .collect();
            if !others.is_empty() {
                map.insert(name.to_lowercase(), others);
            }
        }
    }

    map
}

pub fn build_tags_line(
    tags: &[String],
    tags_label: &str,
    lang_param: Option<&str>,
    tag_config: &HashMap<String, TagInfo>,
    released: &str,
) -> String {
    let tag_links: String = if tags.is_empty() {
        "<span class=\"tags-none\">\u{2014}</span>".to_string()
    } else {
        tags.iter()
            .map(|tag| {
                let style_attr = match tag_style(tag, tag_config) {
                    Some(s) => format!(r#" style="{}""#, s),
                    None => String::new(),
                };
                let class = if tag_style(tag, tag_config).is_some() {
                    "tag-link"
                } else {
                    "tag-link tag-default"
                };
                let href = if let Some(lang) = lang_param {
                    format!("/?lang={}&search={}", html_escape(lang), html_escape(tag))
                } else {
                    format!("/?search={}", html_escape(tag))
                };
                format!(
                    r#"<a href="{}" class="{}"{}>{}</a>"#,
                    href,
                    class,
                    style_attr,
                    html_escape(&tag.to_uppercase())
                )
            })
            .collect()
    };

    // Build event links for tags that have url/label
    let year = if released.len() >= 4 {
        &released[..4]
    } else {
        ""
    };
    let event_links: String = tags
        .iter()
        .filter_map(|tag| {
            let info = tag_config.get(&tag.to_lowercase())?;
            let url_template = info.url.as_deref()?;
            let label_template = info.label.as_deref()?;
            let url = url_template.replace("{year}", year).replace("{tag}", tag);
            let label = label_template.replace("{year}", year).replace("{tag}", tag);
            Some(format!(
                r#"<a href="{}" class="tag-event-link" target="_blank" rel="noopener">{}</a>"#,
                html_escape(&url),
                html_escape(&label)
            ))
        })
        .collect();

    format!(
        r#"<div class="tags-line"><span class="tags-label">{}</span> {}{}</div>"#,
        html_escape(tags_label),
        tag_links,
        if event_links.is_empty() {
            String::new()
        } else {
            format!(r#" {}"#, event_links)
        }
    )
}

pub fn strip_img_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("<img") {
        result.push_str(&remaining[..start]);

        if let Some(end) = remaining[start..].find("/>") {
            remaining = &remaining[start + end + 2..];
            remaining = remaining.trim_start_matches(['\n', '\r']);
        } else {
            result.push_str(&remaining[start..start + 4]);
            remaining = &remaining[start + 4..];
        }
    }

    result.push_str(remaining);
    result
}
