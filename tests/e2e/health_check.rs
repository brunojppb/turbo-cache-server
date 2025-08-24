use reqwest::Response;

use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_test() {
    let app = spawn_app(None).await;

    let response = check_endpoint("/management/health", &app).await;

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

async fn check_endpoint(endpoint: &str, app: &crate::helpers::TestApp) -> Response {
    let client = reqwest::Client::new();

    client
        .get(format!("{}{}", &app.address, endpoint))
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request {endpoint}"))
}
