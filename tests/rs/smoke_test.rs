use axum::http::{Request, StatusCode};
use lightvn_works::app::build_app;
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

    // then: 400 — the path-traversal guard rejects it
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    // then: 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
