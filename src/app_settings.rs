use std::env;

#[derive(Clone)]
pub struct AppSettings {
    pub port: u16,
}

pub fn get_settings() -> AppSettings {
    let port = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse::<u16>()
        .expect("Could not read PORT from env");

    AppSettings { port }
}
