use std::net::TcpListener;

use decay::{app_settings::get_settings, storage::Storage};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv::dotenv().ok();
    let app_settings = get_settings();
    let address = format!("127.0.0.1:{}", app_settings.port);
    let listener = TcpListener::bind(address)?;
    let storage = Storage::new(&app_settings);
    decay::startup::run(listener, storage)?.await
}
