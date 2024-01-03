use std::net::TcpListener;

#[tokio::test]
async fn health_check_test() {
    let address = spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/management/health", address))
        .send()
        .await
        .expect("Failed to request /management/health");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[allow(clippy::let_underscore_future)]
fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to local address");
    let port = listener.local_addr().unwrap().port();
    let server = decay::startup::run(listener).expect("Could not bind to listener");
    let _ = tokio::spawn(server);

    format!("http://127.0.0.1:{}", port)
}
