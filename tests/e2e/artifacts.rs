use pretty_assertions::assert_eq;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{header, method, path, query_param},
};

use crate::helpers::{TestAppConfig, TurboArtifactFileMock, spawn_app};

#[tokio::test]
async fn upload_artifact_to_s3_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("PUT"))
    .respond_with(ResponseTemplate::new(201))
    .mount(&app.storage_server)
    .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to POST artifact to the cache server");

    let upload_req = &app.storage_server.received_requests().await.unwrap()[0];

    // Make sure the uploaded binary is exactly what has been uploaded to S3
    assert!(upload_req.body == file_mock.file_bytes);
    assert!(response.status() == 201);
}

/// When Turborepo is configured with `"signature": true` (turbo.json), the CLI
/// computes an HMAC-SHA256 of each artifact and sends it as the `x-artifact-tag`
/// header on PUT. The server must persist this value so it can be returned on GET,
/// allowing the client to verify artifact integrity. Without it, every download
/// fails signature verification and is treated as a cache miss.
/// See: https://turborepo.dev/api/remote-cache-spec (PUT /artifacts/{hash})
#[tokio::test]
async fn upload_artifact_forwards_artifact_tag_as_s3_metadata_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();
    let artifact_tag = "v=1:sha256:abc123";

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("PUT"))
    .and(header("x-amz-meta-x-artifact-tag", artifact_tag))
    .respond_with(ResponseTemplate::new(201))
    .mount(&app.storage_server)
    .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-tag", artifact_tag)
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 201);
}

#[tokio::test]
async fn upload_artifact_returns_server_error_when_s3_fails_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("PUT"))
    .respond_with(ResponseTemplate::new(403).set_body_string("<Error>AccessDenied</Error>"))
    .mount(&app.storage_server)
    .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 500);
}

/// The case from issue #615: nothing answers on the configured endpoint, so the
/// request never reaches a bucket.
#[tokio::test]
async fn upload_artifact_returns_server_error_when_s3_is_unreachable_test() {
    let app = spawn_app(Some(TestAppConfig {
        s3_endpoint: Some("http://127.0.0.1:1".to_owned()),
        ..Default::default()
    }))
    .await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn download_artifact_returns_server_error_when_s3_fails_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .respond_with(ResponseTemplate::new(500).set_body_string("<Error>InternalError</Error>"))
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 500);
}

/// A cache miss must stay a 404 so Turborepo rebuilds instead of failing.
#[tokio::test]
async fn download_missing_artifact_returns_not_found_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .respond_with(ResponseTemplate::new(404))
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn download_artifact_from_s3_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("GET"))
    .respond_with(ResponseTemplate::new(200).set_body_bytes(file_mock.file_bytes.clone()))
    .mount(&app.storage_server)
    .await;

    // HEAD mock with no artifact-tag metadata
    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert!(response.status() == 200);
    assert!(response.text().await.unwrap().as_bytes() == file_mock.file_bytes);
}

/// Counterpart to `upload_artifact_forwards_artifact_tag_as_s3_metadata_test`.
/// On GET, the server must return the `x-artifact-tag` header that was stored
/// during upload so the Turborepo client can verify the artifact signature.
/// See: https://turborepo.dev/api/remote-cache-spec (GET /artifacts/{hash})
#[tokio::test]
async fn download_artifact_returns_artifact_tag_from_s3_metadata_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();
    let artifact_tag = "v=1:sha256:abc123";

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("GET"))
    .respond_with(ResponseTemplate::new(200).set_body_bytes(file_mock.file_bytes.clone()))
    .mount(&app.storage_server)
    .await;

    // HEAD response with x-amz-meta-x-artifact-tag
    // as the x-amz-meta* is prepended for user-defined metadata
    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(
        ResponseTemplate::new(200).insert_header("x-amz-meta-x-artifact-tag", artifact_tag),
    )
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("x-artifact-tag").unwrap(),
        artifact_tag
    );
}

#[tokio::test]
async fn list_team_artifacts_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/v8/artifacts", &app.address))
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request /v8/artifacts"));

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn artifact_exists_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    mock_s3_head_req(&app, &file_mock, 200).await;

    let response = client
        .head(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to HEAD and check artifact from cache server");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn artifact_does_not_exist_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    mock_s3_head_req(&app, &file_mock, 404).await;

    let response = client
        .head(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to HEAD and check artifact from cache server");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn artifact_check_returns_server_error_when_s3_fails_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    mock_s3_head_req(&app, &file_mock, 503).await;

    let response = client
        .head(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to HEAD and check artifact from cache server");

    assert_eq!(response.status(), 500);
}

/// A head request must be performed to the S3 bucket
/// to check whether the artifact exists
async fn mock_s3_head_req(
    app: &crate::helpers::TestApp,
    file_mock: &crate::helpers::TurboArtifactFileMock,
    response_code: u16,
) {
    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(ResponseTemplate::new(response_code))
    .mount(&app.storage_server)
    .await;
}

#[tokio::test]
async fn artifacts_status_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/v8/artifacts/status", &app.address))
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request /v8/artifacts/status"));

    assert!(response.status().is_success());

    let response_text = response.text().await.unwrap();
    assert!(response_text.contains("\"status\""));
    assert!(response_text.contains("\"enabled\""));
}

/// Issue #620: at 8 MiB the upload becomes multipart, and S3 accepts user
/// metadata only when the multipart upload starts. Signed caches break without
/// it, because the client cannot verify the artifact it downloads.
#[tokio::test]
async fn upload_at_the_multipart_boundary_sets_metadata_on_initiation_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();
    let artifact_tag = "v=1:sha256:abc123";
    let object_path = format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    );

    Mock::given(path(object_path.clone()))
        .and(method("POST"))
        .and(query_param("uploads", ""))
        .and(header("x-amz-meta-x-artifact-tag", artifact_tag))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult><UploadId>upload-1</UploadId></InitiateMultipartUploadResult>"#,
            "application/xml",
        ))
        .mount(&app.storage_server)
        .await;

    Mock::given(path(object_path.clone()))
        .and(method("PUT"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-part\""))
        .mount(&app.storage_server)
        .await;

    Mock::given(path(object_path))
        .and(method("POST"))
        .and(query_param("uploadId", "upload-1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult><ETag>"etag-complete"</ETag></CompleteMultipartUploadResult>"#,
            "application/xml",
        ))
        .mount(&app.storage_server)
        .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            &app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-tag", artifact_tag)
        .body(vec![7u8; 8 * 1024 * 1024])
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 201);
}

/// A body at the multipart boundary cannot be sized without a Content-Length,
/// so the server says so rather than guessing or buffering it all.
///
/// `reqwest` only builds an unsized streaming body behind its `stream`
/// feature, which this crate does not enable, so the request is written by
/// hand over a raw TCP socket with `Transfer-Encoding: chunked` and no
/// `Content-Length` header — the same wire shape an HTTP/1.1 client without a
/// known body size would send.
#[tokio::test]
async fn upload_at_the_multipart_boundary_without_content_length_is_rejected_test() {
    let app = spawn_app(None).await;

    let file_mock = TurboArtifactFileMock::new();
    let host = app
        .address
        .strip_prefix("http://")
        .expect("app address must be an http URL")
        .to_owned();
    let request_line = format!(
        "PUT /v8/artifacts/{}?slug={} HTTP/1.1\r\n",
        file_mock.file_hash, file_mock.team
    );

    let status = tokio::task::spawn_blocking(move || {
        put_chunked_body_without_content_length(&host, &request_line, 8 * 1024 * 1024)
    })
    .await
    .expect("the blocking socket task panicked");

    assert_eq!(status, 411);
    assert!(
        app.storage_server
            .received_requests()
            .await
            .unwrap()
            .is_empty(),
        "the server must not call S3 for an unsized body"
    );
}

/// Sends `PUT {request_line}` with a single chunked-encoded body of
/// `body_len` bytes and no `Content-Length` header, then returns the response
/// status code. The trailing chunk framing is written best-effort, because the
/// server may answer and close the connection as soon as it has read enough
/// of the body to decide, before the framing completes.
fn put_chunked_body_without_content_length(host: &str, request_line: &str, body_len: usize) -> u16 {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect(host).expect("Failed to connect to the cache server");

    let headers = format!(
        "{request_line}Host: {host}\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(headers.as_bytes())
        .expect("Failed to write request headers");
    stream
        .write_all(format!("{body_len:x}\r\n").as_bytes())
        .expect("Failed to write chunk size");
    stream
        .write_all(&vec![7u8; body_len])
        .expect("Failed to write chunk body");
    let _ = stream.write_all(b"\r\n0\r\n\r\n");

    let mut status_line = String::new();
    BufReader::new(stream)
        .read_line(&mut status_line)
        .expect("Failed to read the response status line");

    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("Failed to parse the HTTP status code")
}
