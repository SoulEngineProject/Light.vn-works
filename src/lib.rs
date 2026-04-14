use pulldown_cmark::{html, Parser, Event};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct I18nStrings {
    pub more_from: String,
    pub share: String,
    pub copied: String,
    pub footer: String,
    pub breadcrumb_works: String,
    pub engine_url: String,
}

struct I18nPair {
    en: I18nStrings,
    ja: I18nStrings,
}

static I18N: OnceLock<I18nPair> = OnceLock::new();

fn load_i18n() -> &'static I18nPair {
    I18N.get_or_init(|| {
        let raw: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(include_str!("../public/i18n.json"))
                .expect("Failed to parse i18n.json");

        fn extract(raw: &HashMap<String, HashMap<String, String>>, lang: &str) -> I18nStrings {
            let get = |key: &str| -> String {
                raw.get(key)
                    .and_then(|m| m.get(lang))
                    .cloned()
                    .unwrap_or_default()
            };
            I18nStrings {
                more_from: get("more_from"),
                share: get("share"),
                copied: get("copied"),
                footer: get("footer"),
                breadcrumb_works: get("breadcrumb_works"),
                engine_url: get("engine_url"),
            }
        }

        I18nPair {
            en: extract(&raw, "en"),
            ja: extract(&raw, "ja"),
        }
    })
}

pub fn get_i18n(lang: &str) -> &'static I18nStrings {
    let i18n = load_i18n();
    if lang.contains("ja") {
        &i18n.ja
    } else {
        &i18n.en
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

pub fn extract_first_image(md: &str) -> Option<String> {
    let parser = Parser::new(md);

    for event in parser {
        if let Event::Html(html) = event {
            let html_str = html.to_string();

            if let Some(src_start) =
                html_str.find("src=\"https://github.com/user-attachments/")
            {
                let rest = &html_str[src_start + 5..];
                if let Some(end_quote) = rest.find('\"') {
                    let src_value = &rest[..end_quote];
                    if src_value.starts_with("https://github.com/user-attachments/") {
                        return Some(src_value.to_string());
                    }
                }
            }
        }
    }

    None
}

pub fn extract_all_images(md: &str) -> Vec<String> {
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
                    let url = &html_str[abs_start..abs_start + end_quote];
                    images.push(url.to_string());
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

#[derive(Clone, Debug)]
pub struct CreatorGame {
    pub title: String,
    pub path: String,
    pub thumbnail: Option<String>,
    pub released: String,
    pub tags: Vec<String>,
}

/// Split a creator field into individual creator names.
pub fn split_creators(creator: &str) -> Vec<String> {
    creator
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build an index of creator (lowercased) → list of their games.
/// Creators with commas are split into separate entries.
pub fn build_creator_index(
    entries: &[(String, String, String, Option<String>, String, Vec<String>)], // (creator, title, path, thumbnail, released, tags)
) -> HashMap<String, Vec<CreatorGame>> {
    let mut index: HashMap<String, Vec<CreatorGame>> = HashMap::new();

    for (creator, title, path, thumbnail, released, tags) in entries {
        if creator.is_empty() {
            continue;
        }

        let game = CreatorGame {
            title: title.clone(),
            path: path.clone(),
            thumbnail: thumbnail.clone(),
            released: released.clone(),
            tags: tags.clone(),
        };

        for name in split_creators(creator) {
            index
                .entry(name.to_lowercase())
                .or_default()
                .push(game.clone());
        }
    }

    // Sort each creator's games by release date (newest first)
    for games in index.values_mut() {
        games.sort_by(|a, b| b.released.cmp(&a.released));
    }

    index
}

/// Get related games by the same creator(s), excluding the current game.
/// Returns a list of (creator_name, games) pairs for each creator that has other games.
pub fn get_related_games_by_creator<'a>(
    index: &'a HashMap<String, Vec<CreatorGame>>,
    creator_field: &str,
    current_path: &str,
    limit: usize,
) -> Vec<(String, Vec<&'a CreatorGame>)> {
    if creator_field.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();
    seen_paths.insert(current_path.to_string());

    for name in split_creators(creator_field) {
        if let Some(games) = index.get(&name.to_lowercase()) {
            let related: Vec<&CreatorGame> = games
                .iter()
                .filter(|g| !seen_paths.contains(&g.path))
                .take(limit)
                .collect();

            if !related.is_empty() {
                for g in &related {
                    seen_paths.insert(g.path.clone());
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
