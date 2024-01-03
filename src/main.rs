use std::net::TcpListener;

use decay::app_settings::get_settings;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let app_settings = get_settings();
    let address = format!("127.0.0.1:{}", app_settings.port);
    let listener = TcpListener::bind(address)?;
    decay::startup::run(listener)?.await
}
