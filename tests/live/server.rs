use decay::{
    app_settings::{S3ChecksumMode, get_settings},
    telemetry::{get_telemetry_subscriber, init_telemetry_subscriber},
};
use secrecy::SecretString;
use std::net::TcpListener;
use std::sync::LazyLock;

use crate::config::LiveConfig;

/// A cache server on a free loopback port, writing to the live bucket.
pub struct LiveServer {
    /// Base URL, for example `http://127.0.0.1:53211`.
    pub address: String,
    /// Host and port, for the tests that speak HTTP over a raw socket.
    pub socket_address: String,
}

/// Starts one cache server for the given checksum mode.
#[allow(clippy::let_underscore_future)]
pub fn spawn_server(config: &LiveConfig, checksum_mode: S3ChecksumMode) -> LiveServer {
    LazyLock::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a loopback port");
    let port = listener
        .local_addr()
        .expect("The listener has a local address")
        .port();

    // The endpoint, region and path style come from `get_settings`, so the run
    // exercises the same reading and normalization the server does.
    let mut settings = get_settings();
    settings.s3_access_key = Some(SecretString::from(config.access_key.clone()));
    settings.s3_secret_key = Some(SecretString::from(config.secret_key.clone()));
    settings.s3_bucket_name = config.bucket.clone();
    // The test bucket carries no server-side encryption policy.
    settings.s3_server_side_encryption = None;
    settings.s3_checksum_mode = checksum_mode;
    // A token in the shell must not turn every live request into a 401.
    settings.turbo_token = None;

    let server = decay::startup::run(listener, settings).expect("Could not bind to the listener");
    let _ = tokio::spawn(server);

    LiveServer {
        address: format!("http://127.0.0.1:{port}"),
        socket_address: format!("127.0.0.1:{port}"),
    }
}

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let subscriber_name = "live";
    let version = "0.0.0";
    let filter_level = String::from("debug");

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_telemetry_subscriber(
            subscriber_name,
            subscriber_name.into(),
            version,
            filter_level,
            std::io::stdout,
        );
        init_telemetry_subscriber(subscriber);
    } else {
        let subscriber = get_telemetry_subscriber(
            subscriber_name,
            subscriber_name.into(),
            version,
            filter_level,
            std::io::sink,
        );
        init_telemetry_subscriber(subscriber);
    }
});
