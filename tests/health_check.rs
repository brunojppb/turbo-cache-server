#[tokio::test]
async fn health_check_test() {
    spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get("http://127.0.0.1:8000/management/health")
        .send()
        .await
        .expect("Failed to request /management/health");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() {
    let server = decay::run().expect("Failed to bind to local address");

    let _ = tokio::spawn(server);
}
