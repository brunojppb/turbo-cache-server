use std::env;

#[derive(Clone)]
pub struct AppSettings {
    /// Host where to bind the server to
    /// Defaults to the loopback address
    pub host: String,
    pub port: u16,
    /// The maximum size allowed for payloads
    /// uploaded by Turborepo. Defaults to 100MB.
    pub max_payload_size_in_bytes: usize,
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

    let host = env::var("HOST").unwrap_or("127.0.0.1".to_owned());

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

    let payload_in_mb = env::var("MAX_PAYLOAD_SIZE_IN_MB").unwrap_or("100".to_string());

    let max_payload_size_in_bytes = payload_in_mb
        .parse::<usize>()
        .map(|size_in_mb| size_in_mb * 1024 * 1024)
        .unwrap_or_else(|_| {
            panic!("Invalid value given for MAX_PAYLOAD_SIZE_IN_MB: \"{payload_in_mb}\"",)
        });

    AppSettings {
        host,
        port,
        max_payload_size_in_bytes,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_endpoint,
        s3_bucket_name,
        s3_use_path_style,
    }
}
