use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_test() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/management/health", &app.address))
        .send()
        .await
        .expect("Failed to request /management/health");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
