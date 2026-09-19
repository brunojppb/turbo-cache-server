//! Round trips against a real S3-compatible store.
//!
//! Every test is ignored, so `cargo test` stays offline. Start a store, then run
//! `cargo live-test` with `S3_ENDPOINT`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`,
//! `S3_BUCKET_NAME`, `S3_REGION` and `S3_USE_PATH_STYLE` set.

mod config;
mod raw_http;
mod s3;
mod server;

use aws_sdk_s3::Client;
use decay::app_settings::S3ChecksumMode;
use decay::domain::{ARTIFACT_DURATION_HEADER, ARTIFACT_TAG_HEADER};
use pretty_assertions::assert_eq;
use std::future::Future;
use std::time::{Duration, Instant};

use config::LiveConfig;
use raw_http::{AbandonedUpload, DURATION};
use server::{LiveServer, spawn_server};

/// Sizes around the 8 MiB part boundary, where single and multipart uploads meet.
const SIZES: [usize; 7] = [
    0, 17, 8_388_607, 8_388_608, 8_388_609, 8_715_039, 16_777_233,
];

/// Body of the upload the disconnect test never finishes.
const ABANDONED_SIZE: usize = 40 * 1024 * 1024;

/// How long the disconnect test waits for the store to catch up.
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one HTTP request to the cache server may take.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
enum Framing {
    FixedLength,
    Chunked,
}

impl Framing {
    fn label(self) -> &'static str {
        match self {
            Self::FixedLength => "fixed-length",
            Self::Chunked => "chunked",
        }
    }
}

/// One cache server, one S3 client and one key prefix, for a single test.
struct LiveTest {
    config: LiveConfig,
    s3: Client,
    server: LiveServer,
    mode: &'static str,
    http: reqwest::Client,
}

impl LiveTest {
    async fn start(checksum_mode: S3ChecksumMode) -> Self {
        let config = LiveConfig::from_env();
        let s3 = s3::client(&config);
        s3::ensure_bucket(&s3, &config.bucket, &config.region).await;
        let server = spawn_server(&config, checksum_mode);

        Self {
            config,
            s3,
            server,
            mode: checksum_mode.as_str(),
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("Failed to build the HTTP client"),
        }
    }

    fn url(&self, hash: &str) -> String {
        format!("{}{}", self.server.address, self.path(hash))
    }

    fn path(&self, hash: &str) -> String {
        format!("/v8/artifacts/{hash}?slug={}", self.config.team())
    }

    fn key(&self, hash: &str) -> String {
        format!("{}/{hash}", self.config.team())
    }

    async fn clean_up(&self) {
        s3::clean_prefix(&self.s3, &self.config.bucket, &self.config.team()).await;
    }
}

#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn fixed_length_uploads_round_trip_when_checksums_are_required() {
    round_trips(S3ChecksumMode::WhenRequired, Framing::FixedLength).await;
}

#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn chunked_uploads_round_trip_when_checksums_are_required() {
    round_trips(S3ChecksumMode::WhenRequired, Framing::Chunked).await;
}

#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn fixed_length_uploads_round_trip_when_checksums_are_supported() {
    round_trips(S3ChecksumMode::WhenSupported, Framing::FixedLength).await;
}

#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn chunked_uploads_round_trip_when_checksums_are_supported() {
    round_trips(S3ChecksumMode::WhenSupported, Framing::Chunked).await;
}

/// A hash the bucket never held reads back as a cache miss.
#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn a_missing_artifact_answers_404() {
    let test = LiveTest::start(S3ChecksumMode::WhenRequired).await;

    let response = test
        .http
        .get(test.url("never-uploaded"))
        .send()
        .await
        .expect("Failed to GET the missing artifact");

    assert_eq!(response.status().as_u16(), 404);
}

/// Dropping the client mid-body must abort the multipart upload the server
/// started, and must not leave a truncated artifact behind.
#[tokio::test]
#[ignore = "needs a live S3-compatible store; run with `cargo live-test`"]
async fn a_dropped_chunked_upload_leaves_no_multipart_upload() {
    let test = LiveTest::start(S3ChecksumMode::WhenRequired).await;
    let hash = "dropped-stream";
    let key = test.key(hash);
    let prefix = test.config.team();

    let upload = AbandonedUpload::start(
        &test.server.socket_address,
        &test.path(hash),
        "signature-dropped-stream",
        artifact_body(ABANDONED_SIZE),
    )
    .await;

    wait_until("the multipart upload to start", CLEANUP_TIMEOUT, || async {
        s3::multipart_upload_keys(&test.s3, &test.config.bucket, &prefix)
            .await
            .contains(&key)
    })
    .await;

    upload.disconnect().await;

    wait_until(
        "the multipart upload to be aborted",
        CLEANUP_TIMEOUT,
        || async {
            !s3::multipart_upload_keys(&test.s3, &test.config.bucket, &prefix)
                .await
                .contains(&key)
        },
    )
    .await;

    assert!(
        s3::object_is_absent(&test.s3, &test.config.bucket, &key).await,
        "a dropped upload must not leave an object at {key}"
    );

    test.clean_up().await;
}

async fn round_trips(checksum_mode: S3ChecksumMode, framing: Framing) {
    let test = LiveTest::start(checksum_mode).await;

    for size in SIZES {
        round_trip(&test, framing, size).await;
    }

    test.clean_up().await;
}

async fn round_trip(test: &LiveTest, framing: Framing, size: usize) {
    let label = format!("{} {} {size} bytes", test.mode, framing.label());
    println!("live: checking {label}");

    let hash = format!("{}-{}-{size}", test.mode, framing.label());
    let tag = format!("signature-{hash}");
    let url = test.url(&hash);
    let body = artifact_body(size);

    let status = match framing {
        Framing::FixedLength => {
            raw_http::put_fixed_length(&test.http, &url, &tag, body.clone()).await
        }
        Framing::Chunked => {
            raw_http::put_chunked(
                &test.server.socket_address,
                &test.path(&hash),
                &tag,
                body.clone(),
            )
            .await
        }
    };
    assert_eq!(status, 201, "PUT of {label}");

    let response = test
        .http
        .get(&url)
        .send()
        .await
        .unwrap_or_else(|error| panic!("Failed to GET {label}: {error}"));
    assert_eq!(response.status().as_u16(), 200, "GET of {label}");
    assert_eq!(
        header(&response, ARTIFACT_TAG_HEADER),
        Some(tag.clone()),
        "GET tag of {label}"
    );
    assert_eq!(
        header(&response, ARTIFACT_DURATION_HEADER),
        Some(DURATION.to_owned()),
        "GET duration of {label}"
    );
    let downloaded = response
        .bytes()
        .await
        .unwrap_or_else(|error| panic!("Failed to read the body of {label}: {error}"));
    assert!(
        downloaded.as_ref() == body.as_slice(),
        "GET of {label} returned {} bytes that differ from the upload",
        downloaded.len()
    );

    let response = test
        .http
        .head(&url)
        .send()
        .await
        .unwrap_or_else(|error| panic!("Failed to HEAD {label}: {error}"));
    assert_eq!(response.status().as_u16(), 200, "HEAD of {label}");
    assert_eq!(
        header(&response, ARTIFACT_TAG_HEADER),
        Some(tag.clone()),
        "HEAD tag of {label}"
    );
    assert_eq!(
        header(&response, ARTIFACT_DURATION_HEADER),
        Some(DURATION.to_owned()),
        "HEAD duration of {label}"
    );
    assert!(
        response
            .bytes()
            .await
            .expect("Failed to read the HEAD body")
            .is_empty(),
        "HEAD of {label} must answer without a body"
    );

    let key = test.key(&hash);
    let stored = s3::stored_object(&test.s3, &test.config.bucket, &key).await;
    assert_eq!(stored.size, size as u64, "stored size of {label}");
    assert_eq!(
        stored.content_type.as_deref(),
        Some("application/octet-stream"),
        "stored content type of {label}"
    );
    assert_eq!(
        stored.metadata.get(ARTIFACT_TAG_HEADER).cloned(),
        Some(tag),
        "stored tag of {label}"
    );
    assert_eq!(
        stored.metadata.get(ARTIFACT_DURATION_HEADER).cloned(),
        Some(DURATION.to_owned()),
        "stored duration of {label}"
    );

    s3::delete_object(&test.s3, &test.config.bucket, &key).await;
}

fn header(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// Deterministic bytes, so an upload and a download compare without hashing.
fn artifact_body(size: usize) -> Vec<u8> {
    (0..size).map(|index| index as u8).collect()
}

async fn wait_until<F, Fut>(what: &str, timeout: Duration, mut ready: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if ready().await {
            return;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    panic!("Waited {timeout:?} for {what}");
}
