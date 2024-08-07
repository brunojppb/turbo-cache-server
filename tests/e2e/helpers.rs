use decay::{
    app_settings::get_settings,
    telemetry::{get_telemetry_subscriber, init_telemetry_subscriber},
};
use dotenv::dotenv;
use std::net::TcpListener;
use std::sync::LazyLock;

use wiremock::MockServer;

pub struct TestApp {
    /// Address where our app will be listening to HTTP requests.
    /// Commonly using 127.0.0.1 during local tests.
    pub address: String,
    /// Intercept and mock S3 provider APIs
    pub storage_server: MockServer,
    pub bucket_name: String,
}

#[allow(clippy::let_underscore_future)]
pub async fn spawn_app() -> TestApp {
    dotenv().ok();

    LazyLock::force(&TRACING);

    let storage_server = MockServer::start().await;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to local address");
    let port = listener.local_addr().unwrap().port();
    let mut app_settings = get_settings();
    let bucket_name = "mock_bucket".to_owned();

    app_settings.s3_endpoint = Some(storage_server.uri());
    app_settings.s3_use_path_style = true;
    app_settings.s3_bucket_name.clone_from(&bucket_name);

    let server = decay::startup::run(listener, app_settings).expect("Could not bind to listener");
    let _ = tokio::spawn(server);

    let address = format!("http://127.0.0.1:{}", port);
    TestApp {
        address,
        storage_server,
        bucket_name,
    }
}

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let subscriber_name = "test";
    let filter_level = String::from("debug");

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_telemetry_subscriber(subscriber_name, filter_level, std::io::stdout);
        init_telemetry_subscriber(subscriber);
    } else {
        let subscriber = get_telemetry_subscriber(subscriber_name, filter_level, std::io::sink);
        init_telemetry_subscriber(subscriber);
    }
});

/// Helper mock for simulating arbitrary artifacts being
/// uploaded from Turborepo to our cache server
pub(crate) struct TurboArtifactFileMock {
    /// Hash generated by turborepo. used as a filename
    pub file_hash: String,
    /// Bytes representing the uploaded binary file
    pub file_bytes: Vec<u8>,
    /// Team given as the `slug` query string
    pub team: String,
}

impl TurboArtifactFileMock {
    pub(crate) fn new() -> Self {
        Self {
            file_hash: "mock-file-hash".to_owned(),
            file_bytes: Vec::from("file-content".as_bytes()),
            team: "mock-team".to_owned(),
        }
    }
}
