use std::net::TcpListener;

use decay::{
    app_settings::get_settings,
    storage::Storage,
    telemetry::{get_telemetry_subscriber, init_telemetry_subscriber},
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv::dotenv().ok();

    // Initialise our logger and telemetry stack
    // for entire lifecycle of our web server
    let subscriber = get_telemetry_subscriber(PKG_NAME, "info".into(), std::io::stdout);
    init_telemetry_subscriber(subscriber);

    let app_settings = get_settings();
    let address = format!("127.0.0.1:{}", app_settings.port);
    let listener = TcpListener::bind(address)?;
    let storage = Storage::new(&app_settings);
    decay::startup::run(listener, storage)?.await
}
