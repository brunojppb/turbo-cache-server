// Only adds system CA roots to `rust-s3`'s rustls client.
use reqwest_native_roots as _;

pub mod app_settings;
pub mod artifact_cache;
pub mod auth;
pub mod domain;
pub mod http_span;
pub mod routes;
pub mod startup;
pub mod storage;
pub mod telemetry;
