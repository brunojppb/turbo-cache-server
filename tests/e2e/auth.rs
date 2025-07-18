use crate::helpers::{TestApp, TestAppConfig, spawn_app};
#[cfg(test)]
use pretty_assertions::assert_eq;
use reqwest::Response;
use reqwest::header::HeaderMap;

#[tokio::test]
async fn unauthorized_when_token_header_is_missing() {
    let test_app_config = TestAppConfig {
        turbo_token: Some(String::from("turbo-token")),
    };
    let app = spawn_app(Some(test_app_config)).await;

    let response = check_endpoint("/management/health", &app, None).await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn unauthorized_when_token_header_is_invalid() {
    let test_app_config = TestAppConfig {
        turbo_token: Some(String::from("turbo-token")),
    };
    let app = spawn_app(Some(test_app_config)).await;

    let response = check_endpoint(
        "/management/health",
        &app,
        Some(String::from("invalid-token")),
    )
    .await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn authorized_when_token_header_is_valid() {
    let test_app_config = TestAppConfig {
        turbo_token: Some(String::from("valid_token")),
    };
    let app = spawn_app(Some(test_app_config)).await;

    let response = check_endpoint(
        "/management/health",
        &app,
        Some(String::from("valid_token")),
    )
    .await;

    assert_eq!(response.status(), 200);
}

async fn check_endpoint(endpoint: &str, app: &TestApp, turbo_token: Option<String>) -> Response {
    let client = reqwest::Client::new();

    let maybe_headers = match &turbo_token {
        Some(token) => Some(("Authorization", format!("Bearer {token}"))),
        None => None,
    };

    client
        .get(format!("{}{}", &app.address, endpoint))
        .headers(
            maybe_headers
                .map(|(key, value)| {
                    let mut headers = HeaderMap::new();
                    headers.insert(key, value.parse().unwrap());
                    headers
                })
                .unwrap_or(HeaderMap::new()),
        )
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request {}", endpoint))
}
