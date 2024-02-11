use std::net::TcpListener;

use decay::{app_settings::get_settings, storage::Storage};
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv::dotenv().ok();
    // `init` calls `set_logger`, so this is all we need to do.
    // We are falling back to printing all logs at info-level or above
    // if the RUST_LOG environment variable has not been set.
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let app_settings = get_settings();
    let address = format!("127.0.0.1:{}", app_settings.port);
    let listener = TcpListener::bind(address)?;
    let storage = Storage::new(&app_settings);
    decay::startup::run(listener, storage)?.await
}
