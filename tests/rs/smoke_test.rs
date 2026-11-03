use axum::http::{Request, StatusCode};
use lightvn_works::app::build_app;
use lightvn_works::{encode_path, Nav};
use tower::ServiceExt;

#[tokio::test]
async fn home_page_returns_200() {
    // given: the app
    let app = build_app();

    // when: requesting /
    let response = app
        .oneshot(Request::get("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    // then: 200 OK
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn api_tree_returns_200() {
    // given: the app
    let app = build_app();

    // when: requesting /api/tree
    let response = app
        .oneshot(
            Request::get("/api/tree")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 OK
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn game_page_returns_200() {
    // given: the app and a known game
    let app = build_app();

    // when: requesting a game page
    let response = app
        .oneshot(
            Request::get("/works/2024/42%20Hallows%20Street")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 OK
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn game_page_not_found_returns_404() {
    // given: the app
    let app = build_app();

    // when: requesting a non-existent game
    let response = app
        .oneshot(
            Request::get("/works/2024/nonexistent")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 404
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn game_page_rejects_dotdot_in_title() {
    // given: the app
    let app = build_app();

    // when: requesting a title containing ".."
    let response = app
        .oneshot(
            Request::get("/works/2024/a..b")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: the path-traversal guard turns it away as a plain 404
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn game_page_rejects_oversized_title() {
    // given: the app and a title over the 300-char limit
    let app = build_app();
    let long_title = "a".repeat(301);

    // when: requesting it
    let response = app
        .oneshot(
            Request::get(format!("/works/2024/{}", long_title))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: the length guard turns it away as a plain 404
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn thumb_unknown_uuid_returns_404() {
    // given: the app
    let app = build_app();

    // when: requesting a thumbnail for a UUID not in the index (valid size)
    let response = app
        .oneshot(
            Request::get("/thumb/not-a-real-uuid/card")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 404 — the whitelist blocks proxying arbitrary URLs
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn thumb_invalid_size_returns_404() {
    // given: the app
    let app = build_app();

    // when: requesting a thumbnail with an unknown size variant
    let response = app
        .oneshot(
            Request::get("/thumb/any-uuid/enormous")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 404
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sitemap_returns_xml() {
    // given: the app
    let app = build_app();

    // when: requesting the sitemap
    let response = app
        .oneshot(
            Request::get("/sitemap.xml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 with an XML content type
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("xml"), "content-type was {}", ct);
}

#[tokio::test]
async fn game_page_has_canonical_and_absolute_og_url() {
    // given: the app and a known game
    let app = build_app();

    // when: requesting the game page
    let response = app
        .oneshot(
            Request::get("/works/2024/42%20Hallows%20Street")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then: a self-referencing canonical + absolute og:url are present
    assert!(html.contains("rel=\"canonical\""));
    assert!(html.contains("property=\"og:url\""));
    assert!(html.contains("href=\"http"));
}

#[tokio::test]
async fn game_page_shows_dev_example_as_is() {
    // given: a game whose last image is a real editor screenshot (dev_example_index: 5)
    let app = build_app();
    let shot = "ed7b767f-9815-4f4b-8872-e7595187af7a";

    // when: requesting its page
    let response = app
        .oneshot(
            Request::get(
                "/works/2026/%E3%83%8F%E3%83%83%E3%83%94%E3%83%BC%E3%82%A8%E3%83%B3%E3%83%89%E3%81%AF%E5%BE%85%E3%81%A3%E3%81%A6%E3%81%8F%E3%82%8C%E3%81%AA%E3%81%84",
            )
            .body(axum::body::Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then:
    // - the screenshot is shown unframed, exactly once, with its size reserved
    // - no editor frame is drawn around it
    let tag_start = html
        .find(&format!(
            "class=\"dev-example\" src=\"https://github.com/user-attachments/assets/{}\"",
            shot
        ))
        .expect("dev example rendered as-is");
    let tag = &html[tag_start..tag_start + html[tag_start..].find('>').unwrap()];
    assert!(tag.contains(" width=\"") && tag.contains(" height=\""));
    assert_eq!(html.matches(shot).count(), 1);
    assert!(!html.contains("editor-mockup-frame"));
}

async fn get_html(uri: &str) -> String {
    let response = build_app()
        .oneshot(Request::get(uri).body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8_lossy(&body).into_owned()
}

#[rstest::rstest]
#[case::english_first(Some("en-US,en;q=0.9,ja;q=0.8"), "/", "en")]
#[case::japanese_first(Some("ja-JP,ja;q=0.9,en;q=0.8"), "/", "ja")]
#[case::button_overrides_browser(Some("en-US"), "/?lang=ja", "ja")]
#[case::no_header(None, "/", "en")]
#[tokio::test]
async fn home_page_language_follows_detect_lang(
    #[case] accept: Option<&str>,
    #[case] uri: &str,
    #[case] expected: &str,
) {
    // given: a browser language preference and maybe a ?lang= choice
    let mut req = Request::get(uri);
    if let Some(a) = accept {
        req = req.header("accept-language", a);
    }

    // when: requesting the homepage
    let response = build_app()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let vary = response
        .headers()
        .get("vary")
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then:
    // - the server decides the language and hands it to home.js
    // - caches are told the page depends on Accept-Language
    assert!(html.contains(&format!("<html lang=\"{}\">", expected)));
    assert!(html.contains(&format!("var PAGE_LANG = \"{}\";", expected)));
    assert_eq!(vary.as_deref(), Some("Accept-Language"));
}

#[tokio::test]
async fn game_page_matches_english_first_browser() {
    // given: an English-first browser that also lists Japanese
    let req = Request::get("/works/2024/42%20Hallows%20Street")
        .header("accept-language", "en-US,en;q=0.9,ja;q=0.8");

    // when: opening a game page
    let response = build_app()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let vary = response
        .headers()
        .get("vary")
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then: English, like the homepage it was clicked from
    assert!(html.contains("<html lang=\"en\">"));
    assert_eq!(vary.as_deref(), Some("Accept-Language"));
}

#[tokio::test]
async fn game_page_creator_links_keep_lang() {
    // given: a game page opened with ?lang=ja
    let uri = format!("{}?lang=ja", encode_path("/works/2015/ハイケイ"));

    // when: rendering it
    let html = get_html(&uri).await;

    // then: the creator link carries the language, so the creator page stays Japanese
    let creator = format!(
        "href=\"{}?lang=ja\"",
        Nav::default().creator_href("ものもの")
    );
    assert!(html.contains(&creator), "missing {creator}");
}

#[tokio::test]
async fn r18_game_tag_links_keep_the_work_visible() {
    // given: an R18 work, visited with Hide R18 on
    let uri = encode_path("/works/2015/弟とケンカしましたっ！");

    // when: rendering its page
    let html = get_html(&uri).await;

    // then: tag links back to the homepage show R18, like the breadcrumb does
    assert!(html.contains("href=\"/?r18=0&amp;search=tag%3Ar18\""));
    assert!(html.contains("href=\"/?r18=0\""));
}

#[tokio::test]
async fn creator_page_back_link_keeps_explicit_lang() {
    // given: a creator page opened with ?lang=en
    let uri = format!("{}?lang=en", Nav::default().creator_href("ものもの"));

    // when: rendering it
    let html = get_html(&uri).await;

    // then: "all works" returns to an English homepage
    assert!(html.contains("<a href=\"/?lang=en\">"));
}

#[tokio::test]
async fn homepage_and_game_page_agree_on_work_url() {
    // given: a title with characters JS and Rust used to encode differently
    let path = "/works/2026/Fair's Fair! Prologue";
    let href = Nav::default().work_href(path);

    // when: rendering the homepage and the work's own page
    let home = get_html("/").await;
    let game = get_html(&href).await;

    // then: the homepage's link is the work page's canonical URL
    assert_eq!(href, "/works/2026/Fair%27s%20Fair%21%20Prologue");
    assert!(home.contains(&format!("\"href\":\"{}\"", href)));
    assert!(game.contains(&format!("{}\" />", href)) || game.contains(&format!("{}\">", href)));
}

#[tokio::test]
async fn not_found_page_never_echoes_path() {
    // given: a work path carrying markup
    let uri = "/works/2024/%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";

    // when: requesting it
    let html = get_html(uri).await;

    // then: the path appears nowhere, escaped or not
    assert!(!html.contains("img src=x"));
    assert!(!html.contains("onerror"));
}

#[rstest::rstest]
#[case::unknown_work("/works/2024/nope")]
#[case::unknown_creator("/creator/nobody")]
#[case::blocked_path("/works/2024/a..b")]
#[case::unknown_url("/no-such-page")]
#[tokio::test]
async fn every_miss_gets_the_same_404_page(#[case] uri: &str) {
    // given: a request for something that isn't there, from a Japanese browser with ?lang= unset
    let req = Request::get(uri).header("accept-language", "ja");

    // when: requesting it
    let response = build_app()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let vary = response
        .headers()
        .get("vary")
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then:
    // - one page, one status, in the visitor's language
    // - not indexed, and caches keep languages apart
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("<html lang=\"ja\">"));
    assert!(html.contains("ページが見つかりません"));
    assert!(html.contains("noindex"));
    assert_eq!(vary.as_deref(), Some("Accept-Language"));
}

#[rstest::rstest]
#[case::home("/index.html")]
#[case::game("/game.html")]
#[case::creator("/creator.html")]
#[case::not_found("/404.html")]
#[tokio::test]
async fn templates_are_not_served_raw(#[case] uri: &str) {
    // given: the URL a template would have if it sat in public/

    // when: requesting it
    let html = get_html(uri).await;

    // then: the shared 404 page, never unfilled {{placeholders}}
    assert!(html.contains("noindex"));
    assert!(!html.contains("{{"));
}

#[rstest::rstest]
#[case::home_en("/", "en-US", "日本語")]
#[case::home_ja("/", "ja", "English")]
#[case::game_en("/works/2024/42%20Hallows%20Street", "en-US", "日本語")]
#[case::creator_ja("/creator/%E3%82%82%E3%81%AE%E3%82%82%E3%81%AE", "ja", "English")]
#[case::not_found_ja("/nope", "ja", "English")]
#[tokio::test]
async fn language_button_label_comes_from_the_server(
    #[case] uri: &str,
    #[case] accept: &str,
    #[case] label: &str,
) {
    // given: a page and the visitor's browser language
    let req = Request::get(uri).header("accept-language", accept);

    // when: rendering it
    let response = build_app()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then: the button already names the other language, before any script runs
    assert!(html.contains(&format!(
        "<button class=\"lang-toggle\" id=\"lang-toggle\">{}</button>",
        label
    )));
}

#[tokio::test]
async fn not_found_back_link_keeps_state() {
    // given: a missing work, visited with ?lang=en and R18 shown
    let uri = "/works/2024/nope?lang=en&r18=0";

    // when: requesting it
    let html = get_html(uri).await;

    // then: the way back keeps both, in English
    assert!(html.contains("<a href=\"/?lang=en&amp;r18=0\">"));
    assert!(html.contains("Page not found"));
}

#[tokio::test]
async fn game_page_drops_unknown_lang_param() {
    // given: a game page with markup in ?lang=
    let uri =
        "/works/2024/42%20Hallows%20Street?lang=%22%3E%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";

    // when: requesting it
    let html = get_html(uri).await;

    // then:
    // - the value never reaches the page
    // - links carry no lang param at all
    assert!(!html.contains("onerror"));
    assert!(!html.contains("lang=%22"));
    assert!(!html.contains("?lang="));
}

#[tokio::test]
async fn home_has_absolute_og_image_and_canonical() {
    // given: the app
    let app = build_app();

    // when: requesting the home page
    let response = app
        .oneshot(Request::get("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    // then:
    // - og:image is absolute (starts with http), not the relative "/lvn_icon.webp"
    // - a canonical link is present
    assert!(html.contains("property=\"og:image\""));
    assert!(html.contains("http") && html.contains("/lvn_icon.webp"));
    assert!(html.contains("rel=\"canonical\""));
}

#[tokio::test]
async fn responses_carry_security_headers() {
    // given: the app
    let app = build_app();

    // when: requesting any page
    let response = app
        .oneshot(Request::get("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    // then: the baseline security headers are present
    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert!(headers.get("referrer-policy").is_some());
    // CSP present, framing closed, and img-src still allows the GitHub S3
    // redirect hop (dropping it would silently break every hero/gallery image)
    let csp = headers
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(csp.contains("github-production-user-asset-6210df.s3.amazonaws.com"));
    // CSP points violation reports at the report handler
    assert!(csp.contains("report-uri /api/csp-report"));
}

#[tokio::test]
async fn thumb_stats_returns_json() {
    // given: the app
    let app = build_app();

    // when: requesting the thumbnail-proxy stats endpoint
    let response = app
        .oneshot(
            Request::get("/api/thumb-stats")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 with a JSON body carrying the metric fields
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("hit_ratio"));
    assert!(text.contains("cache_entries"));
    assert!(text.contains("warm"));
}

#[tokio::test]
async fn csp_report_accepts_post() {
    // given: the app and a sample violation report
    let app = build_app();
    let report = r#"{"csp-report":{"blocked-uri":"https://evil.example/x"}}"#;

    // when: a browser POSTs it to the report endpoint
    let response = app
        .oneshot(
            Request::post("/api/csp-report")
                .header("content-type", "application/csp-report")
                .body(axum::body::Body::from(report))
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 204 No Content (the raw body is read, not the JSON extractor)
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn feed_returns_atom() {
    // given: the app
    let app = build_app();

    // when: requesting the feed
    let response = app
        .oneshot(
            Request::get("/feed.xml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 with an Atom content type and at least one entry
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("atom+xml"), "content-type was {}", ct);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("<feed"));
    assert!(text.contains("<entry>"));
}

#[tokio::test]
async fn home_advertises_feed() {
    // given: the app
    let app = build_app();

    // when: requesting the home page
    let response = app
        .oneshot(Request::get("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    // then: the head links to the Atom feed
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("application/atom+xml"));
    assert!(html.contains("/feed.xml"));
}

#[tokio::test]
async fn creator_page_lists_works() {
    // given: the app and a known creator (Regen Radikaler → the Oscillatus works)
    let app = build_app();

    // when: requesting their creator page
    let response = app
        .oneshot(
            Request::get("/creator/Regen%20Radikaler")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 with the creator name and their works as cards
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Regen Radikaler"));
    assert!(html.contains("Oscillatus"));
    assert!(html.contains("more-creator-card"));
    // hub links (their HP/Twitter recur across games) + "active since" the earliest release
    assert!(html.contains("extra-link"));
    assert!(html.contains("Star since"));
    // latest-work hero + language toggle
    assert!(html.contains("creator-hero"));
    assert!(html.contains("Latest"));
    assert!(html.contains("lang-toggle"));
    // share button (reused game-page pattern)
    assert!(html.contains("share-btn"));
    // OG image is the newest work's hero art, not the generic site icon
    assert!(html.contains("og:image\" content=\"https://github.com/user-attachments"));
}

#[tokio::test]
async fn creator_page_localizes_to_japanese() {
    // given: the app
    let app = build_app();

    // when: requesting a creator page with ?lang=ja
    let response = app
        .oneshot(
            Request::get("/creator/Regen%20Radikaler?lang=ja")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 and the chrome is Japanese (最新作 = Latest, 作品 = works)
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("最新作"));
    assert!(html.contains("作品"));
}

#[tokio::test]
async fn creator_hero_ignores_unknown_dates() {
    // given: Sumica, who has an undated 2014 work plus dated works through 2018
    let app = build_app();

    // when: requesting their creator page
    let response = app
        .oneshot(
            Request::get("/creator/Sumica")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: the newest *dated* work (弟みくじ, 2018) is the hero — above the grid —
    // rather than the undated 2014 work sorting to the top
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    let hero_pos = html.find("弟みくじ").expect("2018 work should appear");
    let grid_pos = html
        .find("more-works-heading")
        .expect("more-works grid present");
    assert!(
        hero_pos < grid_pos,
        "the newest dated work should be the hero, above the grid"
    );
}

#[tokio::test]
async fn creator_page_unknown_returns_404() {
    // given: the app
    let app = build_app();

    // when: requesting a creator that doesn't exist
    let response = app
        .oneshot(
            Request::get("/creator/nobody-here-at-all")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 404
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sitemap_includes_creator_urls() {
    // given: the app
    let app = build_app();

    // when: requesting the sitemap
    let response = app
        .oneshot(
            Request::get("/sitemap.xml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: it lists creator pages too
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("/creator/"));
}

#[tokio::test]
async fn robots_points_to_sitemap() {
    // given: the app
    let app = build_app();

    // when: requesting robots.txt
    let response = app
        .oneshot(
            Request::get("/robots.txt")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 and the body references the sitemap
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("Sitemap:"));
    assert!(text.contains("/sitemap.xml"));
}

#[tokio::test]
async fn healthz_returns_ok_uncached() {
    // given: the app
    let app = build_app();

    // when: requesting /healthz
    let response = app
        .oneshot(
            Request::get("/healthz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // then: 200 OK, and no layer downgraded it to a cacheable response
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
}
