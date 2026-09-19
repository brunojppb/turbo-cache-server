use decay::app_settings::normalize_endpoint;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variables every live run needs.
const REQUIRED: [&str; 5] = [
    "S3_ENDPOINT",
    "S3_ACCESS_KEY",
    "S3_SECRET_KEY",
    "S3_BUCKET_NAME",
    "S3_REGION",
];

/// How the live tests reach the S3-compatible store under test.
pub struct LiveConfig {
    /// The `S3_ENDPOINT` value, as the server normalizes it.
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub use_path_style: bool,
    /// Scopes every key this run writes, so a rerun never reads stale objects.
    pub run_id: String,
}

impl LiveConfig {
    /// Reads the live settings from the environment.
    pub fn from_env() -> Self {
        let missing: Vec<&str> = REQUIRED
            .into_iter()
            .filter(|name| read(name).is_none())
            .collect();

        assert!(
            missing.is_empty(),
            "The live tests need these environment variables: {}. \
             Point them at a running S3-compatible store, for example \
             S3_ENDPOINT=http://127.0.0.1:9000 S3_USE_PATH_STYLE=true.",
            missing.join(", ")
        );

        let required = |name: &str| read(name).expect("checked above");

        Self {
            endpoint: normalize_endpoint(&required("S3_ENDPOINT"))
                .unwrap_or_else(|error| panic!("{error}")),
            access_key: required("S3_ACCESS_KEY"),
            secret_key: required("S3_SECRET_KEY"),
            bucket: required("S3_BUCKET_NAME"),
            region: required("S3_REGION"),
            use_path_style: read("S3_USE_PATH_STYLE")
                .is_some_and(|value| value == "true" || value == "1"),
            run_id: run_id(),
        }
    }

    /// Turborepo team slug for this run. It is also the key prefix in the bucket.
    pub fn team(&self) -> String {
        format!("live-{}", self.run_id)
    }
}

fn read(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("The clock is set before 1970")
        .as_nanos();

    format!("{nanos:x}-{}", std::process::id())
}
