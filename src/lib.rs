// Only enables OpenSSL's `vendored` build for `rust-s3`'s native-tls stack on Linux.
use openssl as _;

pub mod app_settings;
pub mod artifact_cache;
pub mod auth;
pub mod domain;
pub mod http_span;
pub mod routes;
pub mod startup;
pub mod storage;
pub mod telemetry;
