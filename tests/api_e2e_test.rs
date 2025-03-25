use graph::{
    api::{create_router},
    Config,
};
use axum::{
    body::{Body},
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use bytes::Bytes;
use serde_json::Value;
use tower::ServiceExt;

// Helper function to read body into bytes
async fn read_body(body: Body) -> Bytes {
    body.collect().await.unwrap().to_bytes()
}

// Create a test app instance
fn test_app() -> axum::Router {
    let _config = Config::for_testing();
    create_router()
}

#[tokio::test]
async fn test_health_check() {
    let app = test_app();
    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_body(response.into_body()).await;
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_version() {
    let app = test_app();
    let response = app
        .oneshot(Request::builder().uri("/version").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_body(response.into_body()).await;
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["version"].is_string());
} 