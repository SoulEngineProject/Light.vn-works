use lightvn_works::{parse_frontmatter, extract_all_images, strip_img_tags, html_escape, encode_path, extract_user_attachment_uuid, build_query, game_page_suffixes, is_composite_dimensions, resize_thumbnail, build_creator_paths, get_related_paths, split_creators, get_lang, gallery_rows, build_tags_line, load_aliases, load_tag_config, pick_priority_tag, GameMeta, ParsedGame, ThumbSize, RELEASED_UNKNOWN};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

#[test]
fn build_query_empty_input() {
    assert_eq!(build_query(&[]), "");
}

#[test]
fn build_query_all_empty_values_returns_empty() {
    assert_eq!(build_query(&[("lang", ""), ("r18", "")]), "");
}

#[test]
fn build_query_single_pair() {
    assert_eq!(build_query(&[("lang", "ja")]), "?lang=ja");
}

#[test]
fn build_query_multiple_pairs_joined_with_ampersand() {
    assert_eq!(build_query(&[("lang", "ja"), ("r18", "0")]), "?lang=ja&r18=0");
}

#[test]
fn build_query_filters_empty_values() {
    assert_eq!(build_query(&[("lang", "en"), ("r18", "")]), "?lang=en");
    assert_eq!(build_query(&[("lang", ""), ("r18", "0")]), "?r18=0");
}

#[test]
fn game_page_suffixes_non_r18_no_params() {
    // non-R18 game, nothing incoming → both suffixes empty
    let (back, fwd) = game_page_suffixes(None, false, false);
    assert_eq!(back, "");
    assert_eq!(fwd, "");
}

#[test]
fn game_page_suffixes_r18_game_forces_back_r18() {
    // R18 game with no incoming params → back forces r18=0, fwd empty
    let (back, fwd) = game_page_suffixes(None, true, false);
    assert_eq!(back, "?r18=0");
    assert_eq!(fwd, "");
}

#[test]
fn game_page_suffixes_propagates_incoming_r18() {
    // Non-R18 game, r18=0 incoming → both carry r18=0
    let (back, fwd) = game_page_suffixes(None, false, true);
    assert_eq!(back, "?r18=0");
    assert_eq!(fwd, "?r18=0");
}

#[test]
fn game_page_suffixes_combines_lang_and_r18() {
    // R18 game with lang=ja → both carry lang, back forces r18=0
    let (back, fwd) = game_page_suffixes(Some("ja"), true, false);
    assert_eq!(back, "?lang=ja&r18=0");
    assert_eq!(fwd, "?lang=ja");
}

#[test]
fn extract_uuid_from_user_attachment_url() {
    // Real-shape URL
    assert_eq!(
        extract_user_attachment_uuid(
            "https://github.com/user-attachments/assets/abc-def-123"
        ),
        Some("abc-def-123")
    );
}

#[test]
fn extract_uuid_rejects_non_github_urls() {
    assert_eq!(
        extract_user_attachment_uuid("https://example.com/image.png"),
        None
    );
    assert_eq!(
        extract_user_attachment_uuid("https://raw.githubusercontent.com/user/repo/main/a.png"),
        None
    );
}

#[test]
fn extract_uuid_rejects_malformed_paths() {
    // Extra path segments
    assert_eq!(
        extract_user_attachment_uuid(
            "https://github.com/user-attachments/assets/abc/extra"
        ),
        None
    );
    // Query string
    assert_eq!(
        extract_user_attachment_uuid(
            "https://github.com/user-attachments/assets/abc?x=1"
        ),
        None
    );
    // Fragment
    assert_eq!(
        extract_user_attachment_uuid(
            "https://github.com/user-attachments/assets/abc#frag"
        ),
        None
    );
    // Empty UUID
    assert_eq!(
        extract_user_attachment_uuid("https://github.com/user-attachments/assets/"),
        None
    );
}

#[test]
fn thumb_size_parses_valid_variants() {
    assert_eq!(ThumbSize::parse("ribbon"), Some(ThumbSize::Ribbon));
    assert_eq!(ThumbSize::parse("card"), Some(ThumbSize::Card));
}

#[test]
fn thumb_size_rejects_invalid_variants() {
    assert_eq!(ThumbSize::parse(""), None);
    assert_eq!(ThumbSize::parse("Ribbon"), None); // case-sensitive
    assert_eq!(ThumbSize::parse("thumb"), None);
    assert_eq!(ThumbSize::parse("ribbon/extra"), None);
}

#[test]
fn thumb_size_dimensions() {
    assert_eq!(ThumbSize::Ribbon.dimensions(), (240, 140));
    assert_eq!(ThumbSize::Card.dimensions(), (600, 400));
}

#[test]
fn composite_detection_threshold() {
    assert!(is_composite_dimensions(1600, 400));  // 4:1 → composite
    assert!(is_composite_dimensions(2001, 1000)); // just over 2:1 → composite
    assert!(!is_composite_dimensions(2000, 1000));// exactly 2:1 → not composite
    assert!(!is_composite_dimensions(1280, 720)); // 16:9 → normal
    assert!(!is_composite_dimensions(1000, 1000));// square → normal
    assert!(!is_composite_dimensions(100, 0));    // zero height → guard
}

#[test]
fn resize_thumbnail_preserves_composite_card_no_upscale() {
    // given: typical 1170x216 composite, card target (1600, 400)
    let img = image::DynamicImage::new_rgb8(1170, 216);

    // when: resized
    let resized = resize_thumbnail(&img, ThumbSize::Card);

    // then: source fits within the target envelope — kept as-is (no upscale
    // would introduce interpolation blur before the CSS zoom).
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
    assert_eq!(resized.height(), 166);  // 900 * 216/1170 ≈ 166
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

#[test]
fn encode_path_handles_reserved_chars() {
    // given: a path with a '#' in the title (real case: works/2021/#水卜大作戦【デモ版】)
    // when/then: '#' becomes %23, '/' is preserved, Japanese bytes are percent-encoded
    assert_eq!(encode_path("/works/2021/#title"), "/works/2021/%23title");
    assert_eq!(encode_path("/works/2021/foo?bar"), "/works/2021/foo%3Fbar");
    assert_eq!(encode_path("/works/2021/plain-title"), "/works/2021/plain-title");
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
    assert_eq!(images[0].url, "https://github.com/user-attachments/assets/abc123");
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

    for entry in WalkDir::new(works_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
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
            errors.push(format!("{}: tags field missing from frontmatter", path.display()));
        }

        // released year should match the folder year
        let released = meta.released.as_deref().unwrap_or("");
        let folder_year = path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !released.is_empty() && released != RELEASED_UNKNOWN && !folder_year.is_empty() && !released.starts_with(folder_year) {
            errors.push(format!(
                "{}: released '{}' does not match folder year '{}'",
                path.display(), released, folder_year
            ));
        }

        if !body.contains("<!-- TODO") && !body.contains("src=\"https://github.com/user-attachments/") {
            errors.push(format!("{}: no GitHub image found in body", path.display()));
        }

        if let Some(idx) = meta.thumbnail_index {
            let image_count = extract_all_images(body).len();
            if idx >= image_count {
                errors.push(format!(
                    "{}: thumbnail_index {} out of range (only {} images)",
                    path.display(), idx, image_count
                ));
            }
        }

        let frontmatter_raw = content
            .trim_start()
            .trim_start_matches("---")
            .splitn(2, "\n---")
            .next()
            .unwrap_or("");
        if !frontmatter_raw.lines().any(|l| l.trim_start().starts_with("thumbnail_index:")) {
            errors.push(format!("{}: thumbnail_index field missing from frontmatter", path.display()));
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
    let games = games_map(vec![
        make_game("2024", "Only Game", "Solo", "2024/01/01"),
    ]);
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
    let related = get_related_paths(&index, "Alice, Bob", "/works/2024/Collab", 4, &HashMap::new());

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

fn make_priority_config() -> std::collections::HashMap<String, lightvn_works::TagInfo> {
    // Use the real production config so tests track config changes. If a tag
    // is removed (e.g. "Terrace and Ray"), the relevant priority test should
    // start failing — that's a feature, not a bug.
    load_tag_config(include_str!("../config/tags.yaml"))
}

#[test]
fn priority_r18_always_wins() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&["r18".into()], &cfg), Some("r18"));
    assert_eq!(pick_priority_tag(&["r18".into(), "ai".into()], &cfg), Some("r18"));
    assert_eq!(pick_priority_tag(&["ai".into(), "r18".into()], &cfg), Some("r18"));
    assert_eq!(pick_priority_tag(&["r18".into(), "Terrace and Ray".into()], &cfg), Some("r18"));
}

#[test]
fn priority_terrace_and_ray_second() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&["Terrace and Ray".into()], &cfg), Some("Terrace and Ray"));
    assert_eq!(pick_priority_tag(&["Terrace and Ray".into(), "Spooktober".into()], &cfg), Some("Terrace and Ray"));
    assert_eq!(pick_priority_tag(&["ai".into(), "Terrace and Ray".into()], &cfg), Some("Terrace and Ray"));
}

#[test]
fn priority_other_configured_third() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&["Spooktober".into()], &cfg), Some("Spooktober"));
    assert_eq!(pick_priority_tag(&["ai".into(), "Spooktober".into()], &cfg), Some("Spooktober"));
}

#[test]
fn priority_ai_alone_yields_none() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&["ai".into()], &cfg), None);
}

#[test]
fn priority_unconfigured_yields_none() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&["한국어".into()], &cfg), None);
    assert_eq!(pick_priority_tag(&["ai".into(), "한국어".into()], &cfg), None);
}

#[test]
fn priority_empty_yields_none() {
    let cfg = make_priority_config();
    assert_eq!(pick_priority_tag(&[], &cfg), None);
}

#[test]
fn cards_emit_at_most_one_left_and_one_right_badge() {
    // given: every configured colour-category tag thrown at the system at once
    let cfg = make_priority_config();
    let tags: Vec<String> = vec![
        "r18".into(),
        "ai".into(),
        "Terrace and Ray".into(),
        "Spooktober".into(),
        "한국어".into(),       // unconfigured noise
    ];

    // when: deriving the two badge slots the way the renderers do
    let right_slot = pick_priority_tag(&tags, &cfg);
    let left_slot = tags.iter().any(|t| t.eq_ignore_ascii_case("ai"));

    // then: priority returns a single tag (Option enforces this by type),
    // and AI detection is a single boolean (also type-bounded).
    // R18 wins the priority cascade even with everything else present.
    assert_eq!(right_slot, Some("r18"));
    assert!(left_slot, "AI detected in left slot");

    // total badges rendered = at most 1 (right) + at most 1 (left) = max 2
    let total_badges = right_slot.is_some() as usize + left_slot as usize;
    assert_eq!(total_badges, 2, "exactly two badges rendered for this input");
    assert!(total_badges <= 2, "badge count is bounded at 2");
}

#[test]
fn cards_emit_zero_badges_when_no_tag_qualifies() {
    let cfg = make_priority_config();
    // Only an unconfigured tag; AI absent. Both slots empty.
    let tags: Vec<String> = vec!["한국어".into()];

    let right_slot = pick_priority_tag(&tags, &cfg);
    let left_slot = tags.iter().any(|t| t.eq_ignore_ascii_case("ai"));

    assert_eq!(right_slot, None);
    assert!(!left_slot);
}

#[test]
fn gallery_rows_layout() {
    // given/when/then: max 2 per row. Odd counts produce a trailing [1]; in
    // production the orphan is stripped upstream and promoted to the editor
    // mockup, so this function is typically called with even n.
    assert_eq!(gallery_rows(0), Vec::<usize>::new());
    assert_eq!(gallery_rows(1), vec![1]);
    assert_eq!(gallery_rows(2), vec![2]);
    assert_eq!(gallery_rows(3), vec![2, 1]);
    assert_eq!(gallery_rows(4), vec![2, 2]);
    assert_eq!(gallery_rows(5), vec![2, 2, 1]);
    assert_eq!(gallery_rows(6), vec![2, 2, 2]);
    assert_eq!(gallery_rows(7), vec![2, 2, 2, 1]);
    assert_eq!(gallery_rows(8), vec![2, 2, 2, 2]);
    assert_eq!(gallery_rows(9), vec![2, 2, 2, 2, 1]);
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
    assert!(aliases.get("alice").unwrap().contains(&"アリス".to_string()));
    assert!(aliases.get("alice").unwrap().contains(&"A-chan".to_string()));
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
    let all_paths: Vec<&str> = related.iter().flat_map(|(_, paths)| paths.iter().copied()).collect();
    assert_eq!(all_paths.iter().filter(|p| **p == "/works/2024/Solo").count(), 1);
}

#[test]
fn special_tags_get_colour() {
    // given: tag config with palette and groups
    let yaml = "colours:\n  content: \"#dc2626\"\n  contest: \"#d97706\"\ntags:\n  - colour: content\n    tags: [r18]\n  - colour: contest\n    tags: [Summer Jam]";
    let config = load_tag_config(yaml);
    let tags = vec!["r18".to_string(), "Summer Jam".to_string(), "mystery".to_string()];

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
