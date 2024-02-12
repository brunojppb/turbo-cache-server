use std::net::TcpListener;

use decay::{
    app_settings::get_settings,
    storage::Storage,
    telemetry::{get_subscriber, init_subscriber},
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv::dotenv().ok();
    // The worker guard from the subscriber needs to be held
    // until main stops, otherwise our non-blocking logging layer
    // will be dropped early and logs won't be written to a file
    let subscriber = get_subscriber(PKG_NAME, "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let app_settings = get_settings();
    let address = format!("127.0.0.1:{}", app_settings.port);
    let listener = TcpListener::bind(address)?;
    let storage = Storage::new(&app_settings);
    decay::startup::run(listener, storage)?.await
}
