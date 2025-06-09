use std::net::TcpListener;

use decay::{
    app_settings::get_settings,
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

    let address = format!("{}:{}", app_settings.host, app_settings.port);
    let listener = TcpListener::bind(address)?;

    decay::startup::run(listener, app_settings)?.await
}
