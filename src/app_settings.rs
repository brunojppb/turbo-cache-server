use std::env;

#[derive(Clone)]
pub struct AppSettings {
    pub port: u16,
    pub s3_access_key: String,
    pub s3_secret_key: String,
}

pub fn get_settings() -> AppSettings {
    let port = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse::<u16>()
        .expect("Could not read PORT from env");

    let s3_access_key = env::var("S3_ACCESS_KEY").expect("Could not read S3_ACCESS_KEY from env");

    let s3_secret_key = env::var("S3_SECRET_KEY").expect("Could not read S3_SECRET_KEY from env");

    AppSettings {
        port,
        s3_access_key,
        s3_secret_key,
    }
}
