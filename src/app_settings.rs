use std::env;

#[derive(Clone)]
pub struct AppSettings {
    pub port: u16,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_endpoint: Option<String>,
    /// if your S3-compatible store does not support requests
    /// like https://bucket.hostname.domain/. Setting `s3_use_path_style`
    /// to true configures the S3 client to make requests like
    /// https://hostname.domain/bucket instead.
    pub s3_use_path_style: bool,
    pub s3_region: String,
    pub s3_bucket_name: String,
}

pub fn get_settings() -> AppSettings {
    let port = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse::<u16>()
        .expect("Could not read PORT from env");

    let s3_access_key = env::var("S3_ACCESS_KEY").ok();
    let s3_secret_key = env::var("S3_SECRET_KEY").ok();
    let s3_region = env::var("S3_REGION").unwrap_or("eu-central-1".to_owned());
    let s3_endpoint = env::var("S3_ENDPOINT").ok();
    let s3_use_path_style = env::var("S3_USE_PATH_STYLE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    // by default,we scope Turborepo artifacts using the "TURBO_TEAM" name sent by turborepo
    // which creates a folder within the S3 bucket and uploads everything under that.
    let s3_bucket_name = env::var("S3_BUCKET_NAME").unwrap_or("turbo".to_owned());
    AppSettings {
        port,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_endpoint,
        s3_bucket_name,
        s3_use_path_style,
    }
}
