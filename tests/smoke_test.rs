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
