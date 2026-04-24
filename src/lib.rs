pub mod app;

use pulldown_cmark::{html, Parser, Event};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
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

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct GameMeta {
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
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

/// Split YAML frontmatter from markdown body.
/// Returns (parsed meta, body without frontmatter).
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
    /// Returns true if the image is a composite strip (width > height * 2).
    pub fn is_composite(&self) -> bool {
        match (self.width, self.height) {
            (Some(w), Some(h)) if h > 0 => w > h * 2,
            _ => false,
        }
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
                    let tag_end = abs_start + end_quote + html_str[abs_start + end_quote..].find('>').unwrap_or(0) + 1;
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
}

/// Percent-encode reserved URL characters in a path. Preserves '/' (so callers
/// can pass "/works/2021/title") and encodes everything else that's not an
/// unreserved character per RFC 3986. Titles starting with '#' or containing
/// '?' would otherwise be mis-parsed by the browser.
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

/// A parsed markdown game file. Sole source of truth for game data in-memory.
#[derive(Clone, Debug)]
pub struct ParsedGame {
    pub year: String,                // directory name
    pub title: String,               // file stem, no .md
    pub path: String,                // "/works/YYYY/title", no .md
    pub meta: GameMeta,
    pub body_html: String,           // pre-rendered markdown
    pub images: Vec<ImageInfo>,
    pub thumbnail: Option<String>,
    pub thumbnail_composite: bool,
}

/// Split a creator field into individual creator names.
pub fn split_creators(creator: &str) -> Vec<String> {
    creator
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build creator → paths index. Paths are sorted by release date descending
/// (unknown last). Creators with commas are split into separate entries.
pub fn build_creator_paths(
    games: &HashMap<String, ParsedGame>,
) -> HashMap<String, Vec<String>> {
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

/// Get related paths by the same creator(s), excluding the current path.
/// Returns (creator_name, paths) pairs for each creator that has other games.
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

/// Compute gallery row sizes. Fills rows of 3, avoids orphan (1 image alone)
/// by converting the last [3, 1] into [2, 2].
pub fn gallery_rows(n: usize) -> Vec<usize> {
    if n == 0 {
        return vec![];
    }
    if n <= 3 {
        return vec![n];
    }

    let mut rows = Vec::new();
    let mut remaining = n;

    while remaining > 0 {
        if remaining == 4 {
            rows.push(2);
            rows.push(2);
            remaining = 0;
        } else if remaining == 2 {
            rows.push(2);
            remaining = 0;
        } else if remaining >= 3 {
            rows.push(3);
            remaining -= 3;
        } else {
            rows.push(remaining);
            remaining = 0;
        }
    }

    rows
}

/// Info for a special tag (colour, optional link).
#[derive(Clone, Debug, Serialize)]
pub struct TagInfo {
    pub colour: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
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
    }
    let config: RawConfig = serde_yaml::from_str(yaml).unwrap_or(RawConfig {
        colours: HashMap::new(),
        tags: Vec::new(),
    });
    let mut map = HashMap::new();
    for group in config.tags {
        let resolved_colour = config.colours.get(&group.colour)
            .cloned()
            .unwrap_or(group.colour.clone());
        for tag in &group.tags {
            map.insert(tag.to_lowercase(), TagInfo {
                colour: resolved_colour.clone(),
                url: group.url.clone(),
                label: group.label.clone(),
            });
        }
    }
    map
}

/// Get inline style for a tag badge, or None if unknown.
pub fn tag_style(tag: &str, tag_config: &HashMap<String, TagInfo>) -> Option<String> {
    tag_config.get(&tag.to_lowercase()).map(|info| {
        format!("background:{};color:white", info.colour)
    })
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
        tags.iter().map(|tag| {
            let style_attr = match tag_style(tag, tag_config) {
                Some(s) => format!(r#" style="{}""#, s),
                None => String::new(),
            };
            let class = if tag_style(tag, tag_config).is_some() { "tag-link" } else { "tag-link tag-default" };
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
        }).collect()
    };

    // Build event links for tags that have url/label
    let year = if released.len() >= 4 { &released[..4] } else { "" };
    let event_links: String = tags.iter().filter_map(|tag| {
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
    }).collect();

    format!(
        r#"<div class="tags-line"><span class="tags-label">{}</span> {}{}</div>"#,
        html_escape(tags_label),
        tag_links,
        if event_links.is_empty() { String::new() } else { format!(r#" {}"#, event_links) }
    )
}

pub fn strip_img_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("<img") {
        result.push_str(&remaining[..start]);

        if let Some(end) = remaining[start..].find("/>") {
            remaining = &remaining[start + end + 2..];
            remaining = remaining.trim_start_matches(|c| c == '\n' || c == '\r');
        } else {
            result.push_str(&remaining[start..start + 4]);
            remaining = &remaining[start + 4..];
        }
    }

    result.push_str(remaining);
    result
}
