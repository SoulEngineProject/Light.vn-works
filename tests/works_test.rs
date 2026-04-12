use lightvn_works::{parse_frontmatter, extract_first_image, strip_img_tags, html_escape};
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

        if meta.creator.is_none() {
            errors.push(format!("{}: creator field missing from frontmatter", path.display()));
        }
        if meta.released.is_none() {
            errors.push(format!("{}: released field missing from frontmatter", path.display()));
        }

        if !body.contains("src=\"https://github.com/user-attachments/") {
            errors.push(format!("{}: no GitHub image found in body", path.display()));
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
