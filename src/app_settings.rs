use std::env;

#[derive(Clone)]
pub struct AppSettings {
    pub port: u16,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_region: String,
    pub s3_endpoint: String,
    pub s3_bucket_name: String,
}

pub fn get_settings() -> AppSettings {
    let port = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse::<u16>()
        .expect("Could not read PORT from env");

    let s3_access_key = env::var("S3_ACCESS_KEY").expect("Could not read S3_ACCESS_KEY from env");
    let s3_secret_key = env::var("S3_SECRET_KEY").expect("Could not read S3_SECRET_KEY from env");
    // @TODO: Revisit these defaults. We should probably bail early instead
    let s3_region = env::var("S3_REGION").unwrap_or("eu-central-1".to_owned());
    let s3_endpoint = env::var("S3_ENDPOINT").unwrap_or("http://localhost:9000".to_owned());
    let s3_bucket_name = env::var("S3_BUCKET_NAME").unwrap_or("turbo".to_owned());
    AppSettings {
        port,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_endpoint,
        s3_bucket_name,
    }
}
