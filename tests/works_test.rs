use lightvn_works::{parse_frontmatter, extract_first_image, extract_all_images, strip_img_tags, html_escape, build_creator_index, get_related_games_by_creator, split_creators, get_i18n, gallery_rows, RELEASED_UNKNOWN};
use std::path::Path;
use walkdir::WalkDir;

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

    // when: extracting the first image
    let img = extract_first_image(md);

    // then: the GitHub image URL is returned
    assert_eq!(
        img.as_deref(),
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
    assert_eq!(images[0], "https://github.com/user-attachments/assets/abc123");
    assert_eq!(images.len(), 2);
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
    let entries = vec![
        ("Alice".to_string(), "Game A".to_string(), "/works/2024/Game A".to_string(), None, "2024/01/01".to_string(), vec![]),
        ("Alice".to_string(), "Game B".to_string(), "/works/2024/Game B".to_string(), None, "2024/06/01".to_string(), vec![]),
        ("Bob".to_string(), "Game C".to_string(), "/works/2024/Game C".to_string(), None, "2024/03/01".to_string(), vec![]),
    ];

    // when: building the creator index
    let index = build_creator_index(&entries);

    // then: 2 creators, Alice has 2 games, Bob has 1
    assert_eq!(index.len(), 2);
    assert_eq!(index.get("alice").unwrap().len(), 2);
    assert_eq!(index.get("bob").unwrap().len(), 1);
}

#[test]
fn creator_index_excludes_current_game() {
    // given: creator with 3 games
    let entries = vec![
        ("Alice".to_string(), "Game A".to_string(), "/works/2024/Game A".to_string(), None, "2024/01/01".to_string(), vec![]),
        ("Alice".to_string(), "Game B".to_string(), "/works/2024/Game B".to_string(), None, "2024/06/01".to_string(), vec![]),
        ("Alice".to_string(), "Game C".to_string(), "/works/2024/Game C".to_string(), None, "2024/12/01".to_string(), vec![]),
    ];
    let index = build_creator_index(&entries);

    // when: getting related games for Game A
    let related = get_related_games_by_creator(&index, "Alice", "/works/2024/Game A", 4);

    // then: returns 1 creator group with 2 games, not including Game A
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, "Alice");
    assert_eq!(related[0].1.len(), 2);
    assert!(related[0].1.iter().all(|g| g.path != "/works/2024/Game A"));
}

#[test]
fn creator_index_single_game_creator() {
    // given: creator with only 1 game
    let entries = vec![
        ("Solo".to_string(), "Only Game".to_string(), "/works/2024/Only Game".to_string(), None, "2024/01/01".to_string(), vec![]),
    ];
    let index = build_creator_index(&entries);

    // when: getting related games
    let related = get_related_games_by_creator(&index, "Solo", "/works/2024/Only Game", 4);

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
    let entries = vec![
        ("Alice, Bob".to_string(), "Collab Game".to_string(), "/works/2024/Collab".to_string(), None, "2024/05/01".to_string(), vec![]),
        ("Bob".to_string(), "Solo Game".to_string(), "/works/2024/Solo".to_string(), None, "2024/02/01".to_string(), vec![]),
    ];
    let index = build_creator_index(&entries);

    // when: getting related games for the collab game
    let related = get_related_games_by_creator(&index, "Alice, Bob", "/works/2024/Collab", 4);

    // then: Bob's section shows Solo Game
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, "Bob");
    assert_eq!(related[0].1[0].title, "Solo Game");
}

#[test]
fn i18n_json_parses_both_languages() {
    // given: i18n.json exists and is loaded

    // when: loading English and Japanese strings
    let en = get_i18n("en");
    let ja = get_i18n("ja");

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

#[test]
fn gallery_rows_layout() {
    // given/when/then: verify row splits for all counts 1-9
    assert_eq!(gallery_rows(0), Vec::<usize>::new());
    assert_eq!(gallery_rows(1), vec![1]);
    assert_eq!(gallery_rows(2), vec![2]);
    assert_eq!(gallery_rows(3), vec![3]);
    assert_eq!(gallery_rows(4), vec![2, 2]);
    assert_eq!(gallery_rows(5), vec![3, 2]);
    assert_eq!(gallery_rows(6), vec![3, 3]);
    assert_eq!(gallery_rows(7), vec![3, 2, 2]);
    assert_eq!(gallery_rows(8), vec![3, 3, 2]);
    assert_eq!(gallery_rows(9), vec![3, 3, 3]);
}
