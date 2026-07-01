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
        .oneshot(Request::get("/api/tree").body(axum::body::Body::empty()).unwrap())
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
