use std::net::TcpListener;

use decay::{
    app_settings::get_settings,
    storage::Storage,
    telemetry::{get_subscriber, init_subscriber},
};
use once_cell::sync::Lazy;

#[tokio::test]
async fn health_check_test() {
    let app = spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/management/health", &app.address))
        .send()
        .await
        .expect("Failed to request /management/health");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

pub struct TestApp {
    pub address: String,
}

static TRACING: Lazy<()> = Lazy::new(|| {
    let subscriber_name = "test".into();
    let filter_level = "debug".into();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

#[allow(clippy::let_underscore_future)]
fn spawn_app() -> TestApp {
    dotenv::dotenv().ok();
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to local address");
    let port = listener.local_addr().unwrap().port();
    let app_settings = get_settings();
    let storage = Storage::new(&app_settings);
    let server = decay::startup::run(listener, storage).expect("Could not bind to listener");
    let _ = tokio::spawn(server);

    let address = format!("http://127.0.0.1:{}", port);
    TestApp { address }
}
