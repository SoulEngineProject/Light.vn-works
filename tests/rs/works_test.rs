//! - Tests follow the given/when/then convention: every `#[test]` / `#[rstest]` body has `// given:`, `// when:`, `// then:` sections.
//! - Parameterized tests use `rstest`, one named case per `#[case::name(...)]` — the name appears in `cargo test` output for fast bisection.
//! - Non-parameterized tests use plain `#[test]`.
//! - Common test data is built via `#[fixture]`s (e.g. `cfg`); per-call data uses plain helper fns.

use lightvn_works::{
    build_creator_paths, build_query, build_sitemap, build_tag_index, build_tags_line, encode_path,
    extract_all_images, extract_user_attachment_uuid, gallery_rows, game_page_suffixes, get_lang,
    get_related_paths, html_escape, is_composite_dimensions, load_aliases, load_tag_config,
    parse_frontmatter, pick_priority_tag, released_to_lastmod, resize_thumbnail, split_creators,
    strip_img_tags, GameMeta, ParsedGame, TagInfo, ThumbSize, RELEASED_UNKNOWN,
};
use rstest::{fixture, rstest};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

#[test]
fn build_query_empty_input() {
    // given: no key/value pairs
    let pairs: &[(&str, &str)] = &[];

    // when: building the query string
    let out = build_query(pairs);

    // then: result is empty (no leading '?')
    assert_eq!(out, "");
}

#[test]
fn build_query_all_empty_values_returns_empty() {
    // given: pairs whose values are all empty
    let pairs = &[("lang", ""), ("r18", "")];

    // when: building the query string
    let out = build_query(pairs);

    // then: empty values are filtered, leaving nothing to emit
    assert_eq!(out, "");
}

#[test]
fn build_query_single_pair() {
    // given: a single non-empty key/value pair
    let pairs = &[("lang", "ja")];

    // when: building the query string
    let out = build_query(pairs);

    // then: leading '?' followed by k=v
    assert_eq!(out, "?lang=ja");
}

#[test]
fn build_query_multiple_pairs_joined_with_ampersand() {
    // given: two non-empty key/value pairs
    let pairs = &[("lang", "ja"), ("r18", "0")];

    // when: building the query string
    let out = build_query(pairs);

    // then: pairs joined with '&', leading '?'
    assert_eq!(out, "?lang=ja&r18=0");
}

#[rstest]
#[case::lang_only(&[("lang", "en"), ("r18", "")], "?lang=en")]
#[case::r18_only(&[("lang", ""), ("r18", "0")], "?r18=0")]
fn build_query_filters_empty_values(#[case] pairs: &[(&str, &str)], #[case] expected: &str) {
    // given: mixed pairs where one side has an empty value
    // when: building the query string
    let out = build_query(pairs);

    // then: empty-valued pair is dropped, remaining pair emitted
    assert_eq!(out, expected);
}

#[test]
fn build_sitemap_lists_home_and_encoded_game_urls() {
    // given: a base URL and canonical game entries, one with spaces
    let base = "https://example.com";
    let entries = vec![
        (
            "/works/2024/42 Hallows Street".to_string(),
            Some("2024-03-15".to_string()),
        ),
        ("/works/2016/KONKON".to_string(), None),
    ];

    // when: building the sitemap
    let xml = build_sitemap(base, &entries);

    // then:
    // - well-formed XML header + urlset wrapper
    // - the home page and both games appear as absolute, percent-encoded URLs
    // - entries are sorted, so 2016 precedes 2024
    assert!(xml.starts_with("<?xml version=\"1.0\""));
    assert!(xml.contains("<loc>https://example.com/</loc>"));
    assert!(xml.contains("<loc>https://example.com/works/2016/KONKON</loc>"));
    assert!(xml.contains("<loc>https://example.com/works/2024/42%20Hallows%20Street</loc>"));
    assert!(xml.trim_end().ends_with("</urlset>"));
    assert!(xml.find("2016").unwrap() < xml.find("2024").unwrap());
}

#[test]
fn build_sitemap_emits_lastmod_only_when_present() {
    // given: one entry with a lastmod date and one without
    let entries = vec![
        ("/works/2024/A".to_string(), Some("2024-03-15".to_string())),
        ("/works/2016/B".to_string(), None),
    ];

    // when: building the sitemap
    let xml = build_sitemap("https://example.com", &entries);

    // then: the dated entry carries <lastmod>, the undated one does not
    assert!(xml.contains(
        "<loc>https://example.com/works/2024/A</loc><lastmod>2024-03-15</lastmod></url>"
    ));
    assert!(xml.contains("<loc>https://example.com/works/2016/B</loc></url>"));
    assert_eq!(xml.matches("<lastmod>").count(), 1);
}

#[test]
fn build_sitemap_trims_trailing_slash_from_base() {
    // given: a base URL with a trailing slash and no games
    // when: building the sitemap
    let xml = build_sitemap("https://example.com/", &[]);

    // then: the home URL has no doubled slash
    assert!(xml.contains("<loc>https://example.com/</loc>"));
    assert!(!xml.contains("com//"));
}

#[rstest]
#[case::full("2024/03/15", Some("2024-03-15"))]
#[case::zero_pads("2024/3/5", Some("2024-03-05"))]
#[case::year_month("2024/03", Some("2024-03"))]
#[case::year_only("2018", Some("2018"))]
#[case::empty("", None)]
#[case::unknown(RELEASED_UNKNOWN, None)]
#[case::bad_month("2024/13/01", None)]
#[case::not_a_date("soon", None)]
#[case::too_many("2024/01/02/03", None)]
fn released_to_lastmod_normalizes(#[case] input: &str, #[case] expected: Option<&str>) {
    // given: a frontmatter released value
    // when: converting it to a sitemap lastmod
    let got = released_to_lastmod(input);

    // then: valid dates are zero-padded to W3C form; anything else yields None
    assert_eq!(got.as_deref(), expected);
}

#[test]
fn game_page_suffixes_non_r18_no_params() {
    // given: a non-R18 game with no incoming lang or r18 params
    // when: computing back/fwd suffixes
    let (back, fwd) = game_page_suffixes(None, false, false);

    // then: both suffixes are empty — nothing to propagate
    assert_eq!(back, "");
    assert_eq!(fwd, "");
}

#[test]
fn game_page_suffixes_r18_game_forces_back_r18() {
    // given: an R18 game with no incoming params
    // when: computing back/fwd suffixes
    let (back, fwd) = game_page_suffixes(None, true, false);

    // then:
    // - back forces r18=0 (so homepage shows it after navigating back)
    // - fwd stays empty (incoming request didn't carry r18=0)
    assert_eq!(back, "?r18=0");
    assert_eq!(fwd, "");
}

#[test]
fn game_page_suffixes_propagates_incoming_r18() {
    // given: a non-R18 game whose incoming request carried r18=0
    // when: computing back/fwd suffixes
    let (back, fwd) = game_page_suffixes(None, false, true);

    // then: both carry r18=0 — preserve the user's filter state
    assert_eq!(back, "?r18=0");
    assert_eq!(fwd, "?r18=0");
}

#[test]
fn game_page_suffixes_combines_lang_and_r18() {
    // given: an R18 game with lang=ja, no incoming r18
    // when: computing back/fwd suffixes
    let (back, fwd) = game_page_suffixes(Some("ja"), true, false);

    // then: both carry lang; back additionally forces r18=0
    assert_eq!(back, "?lang=ja&r18=0");
    assert_eq!(fwd, "?lang=ja");
}

#[test]
fn extract_uuid_from_user_attachment_url() {
    // given: a real-shape GitHub user-attachment URL
    let url = "https://github.com/user-attachments/assets/abc-def-123";

    // when: extracting the UUID
    let uuid = extract_user_attachment_uuid(url);

    // then: the trailing path segment is returned
    assert_eq!(uuid, Some("abc-def-123"));
}

#[rstest]
#[case::example_com("https://example.com/image.png")]
#[case::raw_github("https://raw.githubusercontent.com/user/repo/main/a.png")]
fn extract_uuid_rejects_non_github_urls(#[case] url: &str) {
    // given: a URL hosted outside the github user-attachments path
    // when: extracting the UUID
    let uuid = extract_user_attachment_uuid(url);

    // then: non-matching hosts return None
    assert_eq!(uuid, None);
}

#[rstest]
#[case::extra_path("https://github.com/user-attachments/assets/abc/extra")]
#[case::query_string("https://github.com/user-attachments/assets/abc?x=1")]
#[case::fragment("https://github.com/user-attachments/assets/abc#frag")]
#[case::empty_uuid("https://github.com/user-attachments/assets/")]
fn extract_uuid_rejects_malformed_paths(#[case] url: &str) {
    // given: a github user-attachment URL with a malformed trailing path
    // when: extracting the UUID
    let uuid = extract_user_attachment_uuid(url);

    // then: malformed shapes return None
    assert_eq!(uuid, None);
}

#[rstest]
#[case::ribbon("ribbon", Some(ThumbSize::Ribbon))]
#[case::card("card", Some(ThumbSize::Card))]
fn thumb_size_parses_valid_variants(#[case] input: &str, #[case] expected: Option<ThumbSize>) {
    // given: a valid size string used in URLs
    // when: parsing it
    let parsed = ThumbSize::parse(input);

    // then: the matching variant is returned
    assert_eq!(parsed, expected);
}

#[rstest]
#[case::empty("")]
#[case::wrong_case("Ribbon")] // case-sensitive — capital R rejected
#[case::unknown("thumb")]
#[case::with_slash("ribbon/extra")]
fn thumb_size_rejects_invalid_variants(#[case] input: &str) {
    // given: a string that doesn't match either variant
    // when: parsing it
    let parsed = ThumbSize::parse(input);

    // then: None (no fuzzy matching)
    assert_eq!(parsed, None);
}

#[rstest]
#[case::ribbon(ThumbSize::Ribbon, (240, 140))]
#[case::card(ThumbSize::Card, (600, 400))]
fn thumb_size_dimensions(#[case] size: ThumbSize, #[case] expected: (u32, u32)) {
    // given: a ThumbSize variant
    // when: querying its dimensions
    let dims = size.dimensions();

    // then: the variant returns its expected (width, height)
    assert_eq!(dims, expected);
}

#[rstest]
#[case::four_to_one(1600, 400, true)]
#[case::just_over_two(2001, 1000, true)]
#[case::exactly_two(2000, 1000, false)] // strict > threshold
#[case::sixteen_nine(1280, 720, false)]
#[case::square(1000, 1000, false)]
#[case::zero_height(100, 0, false)] // guard against div-by-zero
fn composite_detection_threshold(#[case] w: u32, #[case] h: u32, #[case] expected: bool) {
    // given: a (width, height) pair
    // when: checking composite classification
    let is_comp = is_composite_dimensions(w, h);

    // then: classification matches the 2:1-strict threshold rule
    assert_eq!(is_comp, expected);
}

#[test]
fn resize_thumbnail_preserves_composite_card_no_upscale() {
    // given: typical 1170x216 composite, card target (1600, 400)
    let img = image::DynamicImage::new_rgb8(1170, 216);

    // when: resized
    let resized = resize_thumbnail(&img, ThumbSize::Card);

    // then: source fits within the target envelope — kept as-is (upscaling would add interpolation blur before the CSS zoom)
    assert_eq!(resized.width(), 1170);
    assert_eq!(resized.height(), 216);
}

#[test]
fn resize_thumbnail_shrinks_composite_for_ribbon() {
    // given: 1170x216 composite, ribbon target (900, 400)
    let img = image::DynamicImage::new_rgb8(1170, 216);

    // when: resized
    let resized = resize_thumbnail(&img, ThumbSize::Ribbon);

    // then: scaled to fit within (900, 400), preserving 5.4:1 aspect
    assert_eq!(resized.width(), 900);
    assert_eq!(resized.height(), 166); // 900 * 216/1170 ≈ 166
}

#[test]
fn resize_thumbnail_fills_normal_aspect() {
    // given: a 1280x720 normal screenshot, target is card (600x400)
    let img = image::DynamicImage::new_rgb8(1280, 720);

    // when: resized
    let resized = resize_thumbnail(&img, ThumbSize::Card);

    // then: filled to exact target dimensions (crops excess to cover 600x400)
    assert_eq!(resized.width(), 600);
    assert_eq!(resized.height(), 400);
}

#[rstest]
#[case::hash("/works/2021/#title", "/works/2021/%23title")]
#[case::question("/works/2021/foo?bar", "/works/2021/foo%3Fbar")]
#[case::plain("/works/2021/plain-title", "/works/2021/plain-title")]
fn encode_path_handles_reserved_chars(#[case] input: &str, #[case] expected: &str) {
    // given: a path with possible reserved chars (real case: works/2021/#水卜大作戦【デモ版】)
    // when: encoding the path
    let out = encode_path(input);

    // then: reserved chars become percent-escapes; '/' is preserved as a separator
    assert_eq!(out, expected);
}

fn make_game(year: &str, title: &str, creator: &str, released: &str) -> ParsedGame {
    ParsedGame {
        year: year.to_string(),
        title: title.to_string(),
        path: format!("/works/{}/{}", year, title),
        meta: GameMeta {
            creator: Some(creator.to_string()),
            released: Some(released.to_string()),
            ..Default::default()
        },
        body_html: String::new(),
        images: vec![],
        thumbnail: None,
        thumbnail_ribbon: None,
        thumbnail_composite: false,
    }
}

fn games_map(games: Vec<ParsedGame>) -> HashMap<String, ParsedGame> {
    games.into_iter().map(|g| (g.path.clone(), g)).collect()
}

#[test]
fn parse_frontmatter_basic() {
    // given: markdown with simple frontmatter
    let input = "---\ncreator: OldPat\nreleased: 2024/09/30\n---\n\nSome body text.";

    // when: parsing frontmatter
    let (meta, body) = parse_frontmatter(input);

    // then: creator, released, and body are extracted
    assert_eq!(meta.creator.as_deref(), Some("OldPat"));
    assert_eq!(meta.released.as_deref(), Some("2024/09/30"));
    assert!(body.contains("Some body text."));
}

#[test]
fn parse_frontmatter_missing() {
    // given: markdown without frontmatter
    let input = "<img src=\"test\" />\n\n---\nSynopsis.\n";

    // when: parsing frontmatter
    let (meta, body) = parse_frontmatter(input);

    // then: meta is empty and body is unchanged
    assert!(meta.creator.is_none());
    assert_eq!(body, input);
}

#[test]
fn parse_frontmatter_full() {
    // given: markdown with all frontmatter fields including extra_links
    let input = r#"---
creator: "Test, Author"
released: 2025/01/01
link_label: itch.io
link_url: "https://example.com"
tagline: "A test game."
extra_links:
  - label: Steam
    url: "https://steam.example.com"
---

Body here."#;

    // when: parsing frontmatter
    let (meta, body) = parse_frontmatter(input);

    // then: all fields are populated correctly
    assert_eq!(meta.creator.as_deref(), Some("Test, Author"));
    assert_eq!(meta.tagline.as_deref(), Some("A test game."));
    assert!(meta.extra_links.is_some());
    assert_eq!(meta.extra_links.as_ref().unwrap().len(), 1);
    assert!(body.contains("Body here."));
}

#[test]
fn first_image_extraction() {
    // given: markdown body with a GitHub image tag
    let md = r#"<img width="384" height="216" alt="image" src="https://github.com/user-attachments/assets/abc123" />

---
Synopsis."#;

    // when: extracting all images and taking the first
    let first_url = extract_all_images(md).first().map(|img| img.url.clone());

    // then: the GitHub image URL is returned
    assert_eq!(
        first_url.as_deref(),
        Some("https://github.com/user-attachments/assets/abc123")
    );
}

#[test]
fn frontmatter_has_og_fields() {
    // given: a complete markdown file with frontmatter, images, and synopsis
    let input = r#"---
creator: Test
released: 2024/01/01
link_label: itch.io
link_url: "https://example.com"
tagline: "A short description."
---

<img width="384" height="216" alt="image" src="https://github.com/user-attachments/assets/abc123" />
<img width="384" height="216" alt="image" src="https://github.com/user-attachments/assets/def456" />

---
Full synopsis here."#;

    // when: parsing frontmatter and extracting images
    let (meta, body) = parse_frontmatter(input);
    let images = extract_all_images(body);

    // then: tagline and og_image data are available for OG tags
    assert_eq!(meta.tagline.as_deref(), Some("A short description."));
    assert!(!images.is_empty());
    assert_eq!(
        images[0].url,
        "https://github.com/user-attachments/assets/abc123"
    );
    assert_eq!(images[0].width, Some(384));
    assert_eq!(images[0].height, Some(216));
    assert!(!images[0].is_composite());
    assert_eq!(images.len(), 2);
}

#[test]
fn composite_image_detected() {
    // given: a wide composite image (1170x216)
    let md = r#"<img width="1170" height="216" alt="image" src="https://github.com/user-attachments/assets/abc123" />"#;

    // when: extracting images
    let images = extract_all_images(md);

    // then: detected as composite
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].width, Some(1170));
    assert_eq!(images[0].height, Some(216));
    assert!(images[0].is_composite());
}

#[test]
fn frontmatter_missing_og_fields_defaults_gracefully() {
    // given: minimal frontmatter with no tagline
    let input = "---\ncreator: Test\nreleased: 2024/01/01\n---\n\nBody.";

    // when: parsing frontmatter
    let (meta, body) = parse_frontmatter(input);
    let images = extract_all_images(body);

    // then: tagline is None and images is empty — OG tags will be empty strings
    assert!(meta.tagline.is_none());
    assert!(images.is_empty());
}

#[test]
fn search_by_tag_data_available() {
    // given: frontmatter with tags
    let input = "---\ncreator: Test\nreleased: 2024/01/01\ntags: [r18]\n---\n\nBody.";

    // when: parsing frontmatter
    let (meta, _body) = parse_frontmatter(input);

    // then: tags are parsed and searchable
    let tags = meta.tags.unwrap();
    assert_eq!(tags.len(), 1);
    assert!(tags.contains(&"r18".to_string()));
}

#[test]
fn img_tag_stripping() {
    // given: HTML containing an img tag and a paragraph
    let html = "<img src=\"test.png\" />\n<p>Hello</p>";

    // when: stripping img tags
    let result = strip_img_tags(html);

    // then: img is removed but paragraph remains
    assert!(!result.contains("<img"));
    assert!(result.contains("<p>Hello</p>"));
}

#[test]
fn escape_html_special_chars() {
    // given: string with HTML special characters
    let input = "<b>\"test\"</b>";

    // when: escaping
    let result = html_escape(input);

    // then: all special characters are escaped
    assert_eq!(result, "&lt;b&gt;&quot;test&quot;&lt;/b&gt;");
}

#[test]
fn validate_all_markdown_files() {
    // given: all .md files in the works/ directory
    let works_dir = Path::new("works");
    if !works_dir.is_dir() {
        return;
    }

    // when: checking each file for valid frontmatter and images
    let mut errors = Vec::new();

    for entry in WalkDir::new(works_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        // - Reject filename characters that are illegal on Windows.
        // - A colon or similar aborts `git checkout` on NTFS for every contributor
        //   on that platform (e.g. the `POV: Verity.md` breakage).
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(bad) = name
                .chars()
                .find(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
            {
                errors.push(format!(
                    "{}: filename contains '{}', which is illegal on Windows",
                    path.display(),
                    bad
                ));
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{}: read error: {}", path.display(), e));
                continue;
            }
        };

        if !content.trim_start().starts_with("---") {
            errors.push(format!("{}: missing frontmatter", path.display()));
            continue;
        }

        let (meta, body) = parse_frontmatter(&content);

        if meta.creator.as_deref().unwrap_or("").is_empty() {
            errors.push(format!("{}: creator is empty", path.display()));
        }
        if meta.released.as_deref().unwrap_or("").is_empty() {
            errors.push(format!("{}: released date is empty", path.display()));
        }
        if meta.tags.is_none() {
            errors.push(format!(
                "{}: tags field missing from frontmatter",
                path.display()
            ));
        }

        // released year should match the folder year
        let released = meta.released.as_deref().unwrap_or("");
        let folder_year = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !released.is_empty()
            && released != RELEASED_UNKNOWN
            && !folder_year.is_empty()
            && !released.starts_with(folder_year)
        {
            errors.push(format!(
                "{}: released '{}' does not match folder year '{}'",
                path.display(),
                released,
                folder_year
            ));
        }

        if !body.contains("<!-- TODO")
            && !body.contains("src=\"https://github.com/user-attachments/")
        {
            errors.push(format!("{}: no GitHub image found in body", path.display()));
        }

        if let Some(idx) = meta.thumbnail_index {
            let image_count = extract_all_images(body).len();
            if idx >= image_count {
                errors.push(format!(
                    "{}: thumbnail_index {} out of range (only {} images)",
                    path.display(),
                    idx,
                    image_count
                ));
            }
        }

        let frontmatter_raw = content
            .trim_start()
            .trim_start_matches("---")
            .split("\n---")
            .next()
            .unwrap_or("");
        if !frontmatter_raw
            .lines()
            .any(|l| l.trim_start().starts_with("thumbnail_index:"))
        {
            errors.push(format!(
                "{}: thumbnail_index field missing from frontmatter",
                path.display()
            ));
        }
    }

    // then: no validation errors
    if !errors.is_empty() {
        panic!(
            "Markdown validation failed ({} issues):\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
}

#[test]
fn creator_index_groups_by_creator() {
    // given: 3 games by 2 different creators
    let games = games_map(vec![
        make_game("2024", "Game A", "Alice", "2024/01/01"),
        make_game("2024", "Game B", "Alice", "2024/06/01"),
        make_game("2024", "Game C", "Bob", "2024/03/01"),
    ]);

    // when: building the creator paths index
    let index = build_creator_paths(&games);

    // then: 2 creators, Alice has 2 games, Bob has 1
    assert_eq!(index.len(), 2);
    assert_eq!(index.get("alice").unwrap().len(), 2);
    assert_eq!(index.get("bob").unwrap().len(), 1);
}

#[test]
fn creator_index_excludes_current_game() {
    // given: creator with 3 games
    let games = games_map(vec![
        make_game("2024", "Game A", "Alice", "2024/01/01"),
        make_game("2024", "Game B", "Alice", "2024/06/01"),
        make_game("2024", "Game C", "Alice", "2024/12/01"),
    ]);
    let index = build_creator_paths(&games);

    // when: getting related paths for Game A
    let related = get_related_paths(&index, "Alice", "/works/2024/Game A", 4, &HashMap::new());

    // then: returns 1 creator group with 2 paths, not including Game A
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, "Alice");
    assert_eq!(related[0].1.len(), 2);
    assert!(related[0].1.iter().all(|p| *p != "/works/2024/Game A"));
}

#[test]
fn creator_index_single_game_creator() {
    // given: creator with only 1 game
    let games = games_map(vec![make_game("2024", "Only Game", "Solo", "2024/01/01")]);
    let index = build_creator_paths(&games);

    // when: getting related paths
    let related = get_related_paths(&index, "Solo", "/works/2024/Only Game", 4, &HashMap::new());

    // then: no related games
    assert!(related.is_empty());
}

#[test]
fn split_creators_comma_separated() {
    // given: a creator field with multiple names
    let field = "Snow Ground, ユキハラ創作企画";

    // when: splitting creators
    let names = split_creators(field);

    // then: both names are extracted and trimmed
    assert_eq!(names.len(), 2);
    assert_eq!(names[0], "Snow Ground");
    assert_eq!(names[1], "ユキハラ創作企画");
}

#[test]
fn creator_index_multi_creator_game() {
    // given: a game with 2 creators, and another game by one of them
    let games = games_map(vec![
        make_game("2024", "Collab", "Alice, Bob", "2024/05/01"),
        make_game("2024", "Solo", "Bob", "2024/02/01"),
    ]);
    let index = build_creator_paths(&games);

    // when: getting related paths for the collab game
    let related = get_related_paths(
        &index,
        "Alice, Bob",
        "/works/2024/Collab",
        4,
        &HashMap::new(),
    );

    // then: Bob's section shows Solo
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, "Bob");
    assert_eq!(related[0].1[0], "/works/2024/Solo");
}

#[test]
fn lang_json_parses_both_languages() {
    // given: i18n.json exists and is loaded

    // when: loading English and Japanese strings
    let en = get_lang("en");
    let ja = get_lang("ja");

    // then: all required fields are non-empty
    assert!(!en.more_from.is_empty());
    assert!(!en.share.is_empty());
    assert!(!en.copied.is_empty());
    assert!(!en.footer.is_empty());
    assert!(!en.breadcrumb_works.is_empty());
    assert!(!en.engine_url.is_empty());

    assert!(!ja.more_from.is_empty());
    assert!(!ja.share.is_empty());
    assert!(!ja.copied.is_empty());
    assert!(!ja.footer.is_empty());
    assert!(!ja.breadcrumb_works.is_empty());
    assert!(!ja.engine_url.is_empty());
}

/// - Production tag config, loaded from the real `config/tags.yaml`.
/// - Tests bind to this so they track config changes — if a tag is removed (e.g. "Terrace and Ray"), the relevant priority test should start failing.
#[fixture]
fn cfg() -> HashMap<String, TagInfo> {
    load_tag_config(include_str!("../../config/tags.yaml"))
}

#[rstest]
#[case::alone(&["r18"])]
#[case::with_ai(&["r18", "ai"])]
#[case::ai_first(&["ai", "r18"])]
#[case::with_terrace(&["r18", "Terrace and Ray"])]
fn priority_r18_always_wins(cfg: HashMap<String, TagInfo>, #[case] tags: &[&str]) {
    // given: tag list contains r18 alongside other tags in any order
    let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();

    // when: picking the priority tag
    let result = pick_priority_tag(&owned, &cfg);

    // then: r18 wins regardless of order or co-occurring tags
    assert_eq!(result, Some("r18"));
}

#[rstest]
#[case::alone(&["Terrace and Ray"])]
#[case::with_spook(&["Terrace and Ray", "Spooktober"])]
#[case::with_ai(&["ai", "Terrace and Ray"])]
fn priority_terrace_and_ray_second(cfg: HashMap<String, TagInfo>, #[case] tags: &[&str]) {
    // given: tag list with "Terrace and Ray" present but no r18
    let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();

    // when: picking the priority tag
    let result = pick_priority_tag(&owned, &cfg);

    // then: "Terrace and Ray" wins over other configured tags and over ai
    assert_eq!(result, Some("Terrace and Ray"));
}

#[rstest]
#[case::spook_alone(&["Spooktober"])]
#[case::spook_with_ai(&["ai", "Spooktober"])]
fn priority_other_configured_third(cfg: HashMap<String, TagInfo>, #[case] tags: &[&str]) {
    // given: tag list with a configured non-r18, non-Terrace tag
    let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();

    // when: picking the priority tag
    let result = pick_priority_tag(&owned, &cfg);

    // then: the configured tag wins; ai is not promoted into the right slot
    assert_eq!(result, Some("Spooktober"));
}

#[rstest]
fn priority_ai_alone_yields_none(cfg: HashMap<String, TagInfo>) {
    // given: a tag list containing only ai
    let tags: Vec<String> = vec!["ai".into()];

    // when: picking the priority tag
    let result = pick_priority_tag(&tags, &cfg);

    // then: None — ai has its own dedicated left-slot, never the right slot
    assert_eq!(result, None);
}

#[rstest]
#[case::mystery_alone(&["mystery"])]
#[case::mystery_with_ai(&["ai", "mystery"])]
fn priority_unconfigured_yields_none(cfg: HashMap<String, TagInfo>, #[case] tags: &[&str]) {
    // given:
    // - tag list with only unconfigured tags (and possibly ai)
    // - "mystery" is intentionally absent from tags.yaml — adjust if it ever gets added
    let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();

    // when: picking the priority tag
    let result = pick_priority_tag(&owned, &cfg);

    // then: None — unconfigured tags don't promote, ai doesn't fill the slot
    assert_eq!(result, None);
}

#[rstest]
fn priority_empty_yields_none(cfg: HashMap<String, TagInfo>) {
    // given: an empty tag list
    let tags: Vec<String> = vec![];

    // when: picking the priority tag
    let result = pick_priority_tag(&tags, &cfg);

    // then: None
    assert_eq!(result, None);
}

#[rstest]
#[case::english_alone(&["English"])]
#[case::korean_alone(&["한국어"])]
#[case::chinese_alone(&["中文"])]
#[case::language_with_ai(&["English", "ai"])]
fn priority_language_does_not_promote(cfg: HashMap<String, TagInfo>, #[case] tags: &[&str]) {
    // given: tag list with only language tags (and possibly ai) — no r18, no Terrace and Ray, no other priority-eligible tag
    let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();

    // when: picking the priority tag
    let result = pick_priority_tag(&owned, &cfg);

    // then:
    // - None — languages are filter-only (card_priority_badge: false)
    // - AI has its own slot, so the right slot stays empty
    assert_eq!(result, None);
}

#[rstest]
fn priority_other_configured_beats_language(cfg: HashMap<String, TagInfo>) {
    // given:
    // - a real-world combo: a Spooktober game also tagged English
    // - a bulk-edit pass added English to all Spooktober games — the right slot must still pick Spooktober
    let tags: Vec<String> = vec!["Spooktober".into(), "English".into()];

    // when: picking the priority tag
    let result = pick_priority_tag(&tags, &cfg);

    // then: Spooktober wins; English is filter-only and skipped
    assert_eq!(result, Some("Spooktober"));
}

#[rstest]
fn card_priority_badge_defaults_to_true_when_omitted() {
    // given: a yaml group with no card_priority_badge key
    let yaml = "colours:\n  c: \"#000\"\ntags:\n  - colour: c\n    tags: [Foo]";
    let cfg = load_tag_config(yaml);

    // when: looking up the tag
    let info = cfg.get("foo").expect("Foo configured");

    // then: defaults to true (priority-eligible) — backwards-compatible with yaml that omits the field
    assert!(info.card_priority_badge);
}

#[rstest]
fn card_priority_badge_propagates_false_from_yaml() {
    // given: a yaml group with card_priority_badge: false
    let yaml = "colours:\n  c: \"#000\"\ntags:\n  - colour: c\n    tags: [Foo]\n    card_priority_badge: false";
    let cfg = load_tag_config(yaml);

    // when: looking up the tag
    let info = cfg.get("foo").expect("Foo configured");

    // then: flag is false, and pick_priority_tag will skip it
    assert!(!info.card_priority_badge);
    assert_eq!(pick_priority_tag(&["Foo".to_string()], &cfg), None);
}

#[rstest]
fn cards_emit_at_most_one_left_and_one_right_badge(cfg: HashMap<String, TagInfo>) {
    // given: every configured colour-category tag thrown at the system at once
    let tags: Vec<String> = vec![
        "r18".into(),
        "ai".into(),
        "Terrace and Ray".into(),
        "Spooktober".into(),
        "mystery".into(), // unconfigured noise
    ];

    // when: deriving the two badge slots the way the renderers do
    let right_slot = pick_priority_tag(&tags, &cfg);
    let left_slot = tags.iter().any(|t| t.eq_ignore_ascii_case("ai"));

    // then:
    // - priority returns a single tag (Option enforces this by type)
    // - AI detection is a single boolean (also type-bounded)
    // - R18 wins the priority cascade even with everything else present
    assert_eq!(right_slot, Some("r18"));
    assert!(left_slot, "AI detected in left slot");

    // total badges rendered = at most 1 (right) + at most 1 (left) = max 2
    let total_badges = right_slot.is_some() as usize + left_slot as usize;
    assert_eq!(
        total_badges, 2,
        "exactly two badges rendered for this input"
    );
    assert!(total_badges <= 2, "badge count is bounded at 2");
}

#[rstest]
fn cards_emit_zero_badges_when_no_tag_qualifies(cfg: HashMap<String, TagInfo>) {
    // given: only an unconfigured tag, with AI absent
    let tags: Vec<String> = vec!["mystery".into()];

    // when: deriving the two badge slots
    let right_slot = pick_priority_tag(&tags, &cfg);
    let left_slot = tags.iter().any(|t| t.eq_ignore_ascii_case("ai"));

    // then: both slots are empty — card renders with no badges
    assert_eq!(right_slot, None);
    assert!(!left_slot);
}

#[rstest]
#[case::english("English")]
#[case::korean("한국어")]
#[case::chinese("中文")]
fn language_tag_propagates_yaml_colour_to_bar(cfg: HashMap<String, TagInfo>, #[case] tag: &str) {
    // given: a registered language tag and a game using it
    let info = cfg
        .get(&tag.to_lowercase())
        .expect("tag is configured in yaml");
    let games = games_map(vec![make_game_with_tags("2024", "g", vec![tag])]);

    // when: building the tag-index row
    let bar = build_tag_index(&games, &cfg);
    let row = bar
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(tag))
        .expect("row present");

    // then: bar entry carries whatever colour yaml/config defined — no hardcoded hex, so the test tracks yaml changes
    assert_eq!(row.colour.as_deref(), Some(info.colour.as_str()));
}

#[rstest]
fn language_tags_share_a_colour(cfg: HashMap<String, TagInfo>) {
    // given: the registered language tags loaded from the production yaml
    let english = &cfg.get("english").expect("English configured").colour;
    let korean = &cfg.get("한국어").expect("한국어 configured").colour;
    let chinese = &cfg.get("中文").expect("中文 configured").colour;

    // when: comparing their colours
    // then: all three share one colour — they're a single category
    assert_eq!(english, korean);
    assert_eq!(english, chinese);
}

#[rstest]
#[case::n_0(0, vec![])]
#[case::n_1(1, vec![1])]
#[case::n_2(2, vec![2])]
#[case::n_3(3, vec![2, 1])]
#[case::n_4(4, vec![2, 2])]
#[case::n_5(5, vec![2, 2, 1])]
#[case::n_6(6, vec![2, 2, 2])]
#[case::n_7(7, vec![2, 2, 2, 1])]
#[case::n_8(8, vec![2, 2, 2, 2])]
#[case::n_9(9, vec![2, 2, 2, 2, 1])]
fn gallery_rows_layout(#[case] n: usize, #[case] expected: Vec<usize>) {
    // given:
    // - an image count
    // - in production the orphan trailing image is stripped upstream and promoted to the editor mockup, so this fn is usually called with even n
    // - odd cases here verify the boundary is correct anyway

    // when: computing the row layout
    let rows = gallery_rows(n);

    // then: max 2 per row, trailing [1] when odd
    assert_eq!(rows, expected);
}

#[test]
fn tags_line_empty() {
    // given: no tags
    let tags: Vec<String> = vec![];

    // when: building tags line
    let html = build_tags_line(&tags, "Tags:", None, &HashMap::new(), "");

    // then: shows em dash
    assert!(html.contains("tags-line"));
    assert!(html.contains("\u{2014}"));
    assert!(!html.contains("tag-link"));
}

#[test]
fn tags_line_with_tags() {
    // given: r18 and ai tags with config
    let config = load_tag_config("colours:\n  c1: \"#dc2626\"\n  c2: \"#2563eb\"\ntags:\n  - colour: c1\n    tags: [r18]\n  - colour: c2\n    tags: [ai]");
    let tags = vec!["r18".to_string(), "ai".to_string()];

    // when: building tags line
    let html = build_tags_line(&tags, "Tags:", None, &config, "2024/01/01");

    // then: contains clickable links with inline colours
    assert!(html.contains("background:#dc2626"));
    assert!(html.contains("background:#2563eb"));
    assert!(html.contains("R18"));
    assert!(html.contains("AI"));
    assert!(html.contains("/?search=r18"));
    assert!(html.contains("/?search=ai"));
}

#[test]
fn tags_line_with_lang() {
    // given: tags with language param
    let tags = vec!["r18".to_string()];

    // when: building tags line with Japanese
    let html = build_tags_line(&tags, "タグ：", Some("ja"), &HashMap::new(), "2024/01/01");

    // then: link includes lang param
    assert!(html.contains("/?lang=ja&search=r18"));
    assert!(html.contains("タグ："));
}

#[test]
fn alias_groups_resolve_bidirectionally() {
    // given: an alias group
    let json = r#"[["Alice", "アリス", "A-chan"]]"#;

    // when: loading aliases
    let aliases = load_aliases(json);

    // then: each name maps to all others
    assert_eq!(aliases.get("alice").unwrap().len(), 2);
    assert_eq!(aliases.get("アリス").unwrap().len(), 2);
    assert_eq!(aliases.get("a-chan").unwrap().len(), 2);
    assert!(aliases
        .get("alice")
        .unwrap()
        .contains(&"アリス".to_string()));
    assert!(aliases
        .get("alice")
        .unwrap()
        .contains(&"A-chan".to_string()));
}

#[test]
fn related_games_via_alias() {
    // given: two games by different names that are aliases
    let games = games_map(vec![
        make_game("2024", "Game A", "Alice", "2024/01/01"),
        make_game("2024", "Game B", "アリス", "2024/06/01"),
    ]);
    let index = build_creator_paths(&games);
    let aliases = load_aliases(r#"[["Alice", "アリス"]]"#);

    // when: getting related paths for Game A (by Alice)
    let related = get_related_paths(&index, "Alice", "/works/2024/Game A", 4, &aliases);

    // then: finds Game B via alias アリス
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, "アリス");
    assert_eq!(related[0].1[0], "/works/2024/Game B");
}

#[test]
fn alias_no_duplicate_games() {
    // given: a game listed under both alias names (comma-separated creator)
    let games = games_map(vec![
        make_game("2024", "Collab", "Alice, アリス", "2024/01/01"),
        make_game("2024", "Solo", "Alice", "2024/06/01"),
    ]);
    let index = build_creator_paths(&games);
    let aliases = load_aliases(r#"[["Alice", "アリス"]]"#);

    // when: getting related paths for Collab
    let related = get_related_paths(&index, "Alice, アリス", "/works/2024/Collab", 4, &aliases);

    // then: Solo appears only once (not duplicated across alias lookups)
    let all_paths: Vec<&str> = related
        .iter()
        .flat_map(|(_, paths)| paths.iter().copied())
        .collect();
    assert_eq!(
        all_paths
            .iter()
            .filter(|p| **p == "/works/2024/Solo")
            .count(),
        1
    );
}

#[test]
fn special_tags_get_colour() {
    // given: tag config with palette and groups
    let yaml = "colours:\n  content: \"#dc2626\"\n  contest: \"#d97706\"\ntags:\n  - colour: content\n    tags: [r18]\n  - colour: contest\n    tags: [Summer Jam]";
    let config = load_tag_config(yaml);
    let tags = vec![
        "r18".to_string(),
        "Summer Jam".to_string(),
        "mystery".to_string(),
    ];

    // when: building tags line
    let html = build_tags_line(&tags, "Tags:", None, &config, "2024/01/01");

    // then: r18 gets red, Summer Jam gets gold, mystery gets tag-default
    assert!(html.contains("background:#dc2626"));
    assert!(html.contains("background:#d97706"));
    assert!(html.contains("tag-default"));
    assert!(html.contains("R18"));
    assert!(html.contains("SUMMER JAM"));
    assert!(html.contains("MYSTERY"));
}

#[test]
fn tag_with_url_renders_event_link() {
    // given: tag config with url and label
    let yaml = "colours:\n  contest: \"#d97706\"\ntags:\n  - colour: contest\n    tags: [TestFest]\n    url: \"https://example.com/{year}\"\n    label: \"{tag}{year} Entry\"";
    let config = load_tag_config(yaml);
    let tags = vec!["TestFest".to_string()];

    // when: building tags line with released year
    let html = build_tags_line(&tags, "Tags:", None, &config, "2025/06/01");

    // then: event link rendered with resolved year and tag
    assert!(html.contains("https://example.com/2025"));
    assert!(html.contains("TestFest2025 Entry"));
    assert!(html.contains("tag-event-link"));
}

fn make_game_with_tags(year: &str, title: &str, tags: Vec<&str>) -> ParsedGame {
    ParsedGame {
        year: year.to_string(),
        title: title.to_string(),
        path: format!("/works/{}/{}", year, title),
        meta: GameMeta {
            tags: Some(tags.into_iter().map(String::from).collect()),
            ..Default::default()
        },
        body_html: String::new(),
        images: vec![],
        thumbnail: None,
        thumbnail_ribbon: None,
        thumbnail_composite: false,
    }
}

#[test]
fn tag_index_excludes_r18() {
    // given: a config with both r18 and ai, plus games using both
    let cfg = load_tag_config("colours:\n  c: \"#000\"\ntags:\n  - colour: c\n    tags: [r18, ai]");
    let games = games_map(vec![
        make_game_with_tags("2024", "a", vec!["r18", "ai"]),
        make_game_with_tags("2024", "b", vec!["r18"]),
    ]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: r18 is filtered out (handled by the dedicated toggle); ai stays
    assert!(bar.iter().all(|e| e.name.to_lowercase() != "r18"));
    assert!(bar.iter().any(|e| e.name == "ai"));
}

#[test]
fn tag_index_counts_games_per_tag() {
    // given: 3 games — 2 carry "Spooktober", 1 has no tags
    let cfg =
        load_tag_config("colours:\n  c: \"#000\"\ntags:\n  - colour: c\n    tags: [Spooktober]");
    let games = games_map(vec![
        make_game_with_tags("2024", "a", vec!["Spooktober"]),
        make_game_with_tags("2024", "b", vec!["Spooktober", "ai"]),
        make_game_with_tags("2024", "c", vec![]),
    ]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: Spooktober's count reflects only the games that carry it
    let spook = bar
        .iter()
        .find(|e| e.name == "Spooktober")
        .expect("Spooktober present");
    assert_eq!(spook.count, 2);
}

#[test]
fn tag_index_dedupes_case_insensitively() {
    // given: 3 games using the same tag with different casings
    let cfg = HashMap::new();
    let games = games_map(vec![
        make_game_with_tags("2024", "a", vec!["AI"]),
        make_game_with_tags("2024", "b", vec!["ai"]),
        make_game_with_tags("2024", "c", vec!["Ai"]),
    ]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: collapses to a single entry whose count covers all 3 games
    let ai_rows: Vec<_> = bar
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case("ai"))
        .collect();
    assert_eq!(ai_rows.len(), 1);
    assert_eq!(ai_rows[0].count, 3);
}

#[test]
fn tag_index_dedupes_within_single_game() {
    // given: a single game listing the same tag twice with different casings
    let cfg = HashMap::new();
    let games = games_map(vec![make_game_with_tags(
        "2024",
        "a",
        vec!["mystery", "MYSTERY"],
    )]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: the duplicate within one game still counts as 1
    let row = bar
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("mystery"))
        .unwrap();
    assert_eq!(row.count, 1);
}

#[test]
fn tag_index_yaml_casing_wins_over_md_casing() {
    // given: yaml declares "Terrace and Ray", md uses lowercase "terrace and ray"
    let cfg = load_tag_config(
        "colours:\n  pub: \"#0891b2\"\ntags:\n  - colour: pub\n    tags: [Terrace and Ray]",
    );
    let games = games_map(vec![make_game_with_tags(
        "2024",
        "a",
        vec!["terrace and ray"],
    )]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: the entry uses the yaml's canonical display casing
    let row = bar
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("terrace and ray"))
        .unwrap();
    assert_eq!(row.name, "Terrace and Ray");
}

#[test]
fn tag_index_includes_unconfigured_md_tags() {
    // given: a tag that appears only in md files, never in yaml config
    let cfg = HashMap::new();
    let games = games_map(vec![
        make_game_with_tags("2024", "a", vec!["한국어"]),
        make_game_with_tags("2024", "b", vec!["한국어"]),
    ]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: tag is included with correct count and no colour assignment
    let row = bar.iter().find(|e| e.name == "한국어").unwrap();
    assert_eq!(row.count, 2);
    assert!(row.colour.is_none());
}

#[test]
fn tag_index_includes_configured_tags_with_zero_uses() {
    // given: yaml configures a tag that no game currently uses
    let cfg =
        load_tag_config("colours:\n  c: \"#000\"\ntags:\n  - colour: c\n    tags: [GhostFest]");
    let games: HashMap<String, ParsedGame> = HashMap::new();

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: tag still appears (count 0, with its configured colour)
    let row = bar.iter().find(|e| e.name == "GhostFest").unwrap();
    assert_eq!(row.count, 0);
    assert_eq!(row.colour.as_deref(), Some("#000"));
}

#[test]
fn tag_index_sorts_count_desc_then_name_asc() {
    // given: tags with varied counts including a tie that needs alphabetical fallback
    let cfg = HashMap::new();
    let games = games_map(vec![
        make_game_with_tags("2024", "a", vec!["zeta"]),
        make_game_with_tags("2024", "b", vec!["alpha", "beta"]),
        make_game_with_tags("2024", "c", vec!["alpha", "beta"]),
        make_game_with_tags("2024", "d", vec!["beta"]),
    ]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: count desc primary, name asc secondary → beta(3), alpha(2), zeta(1)
    let names: Vec<&str> = bar.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["beta", "alpha", "zeta"]);
}

#[test]
fn tag_index_configured_carries_colour() {
    // given: a configured tag with a known colour, used by one game
    let cfg = load_tag_config("colours:\n  c: \"#abcdef\"\ntags:\n  - colour: c\n    tags: [Foo]");
    let games = games_map(vec![make_game_with_tags("2024", "a", vec!["Foo"])]);

    // when: building the tag index
    let bar = build_tag_index(&games, &cfg);

    // then: the entry carries the configured colour through to the bar
    let row = bar.iter().find(|e| e.name == "Foo").unwrap();
    assert_eq!(row.colour.as_deref(), Some("#abcdef"));
}
