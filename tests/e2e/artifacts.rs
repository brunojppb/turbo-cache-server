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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-tag", artifact_tag)
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 201);
}

/// Turborepo sends the task run time on upload and reads it back on download to
/// report how much time the cache saved. Without it, every remote hit reports
/// zero. See: https://turborepo.dev/api/remote-cache-spec (PUT /artifacts/{hash})
#[tokio::test]
async fn upload_artifact_forwards_artifact_duration_as_s3_metadata_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("PUT"))
    .and(header("x-amz-meta-x-artifact-duration", "1234"))
    .respond_with(ResponseTemplate::new(201))
    .mount(&app.storage_server)
    .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-duration", "1234")
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 201);
}

/// Turborepo fails the whole cache read on a duration it cannot parse, so the
/// server drops a bad value. The artifact is the payload, so the upload still
/// succeeds.
#[tokio::test]
async fn upload_artifact_drops_malformed_artifact_duration_test() {
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
            app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-duration", "not-a-number")
        .body(file_mock.file_bytes.clone())
        .send()
        .await
        .expect("Failed to PUT artifact to the cache server");

    assert_eq!(response.status(), 201);

    let upload_req = &app.storage_server.received_requests().await.unwrap()[0];
    assert!(
        !upload_req
            .headers
            .contains_key("x-amz-meta-x-artifact-duration")
    );
}

/// A repository with `"signature": true` sends both artifact headers on the
/// same upload, so they must not displace each other in the object metadata.
/// See: https://turborepo.dev/api/remote-cache-spec (PUT /artifacts/{hash})
#[tokio::test]
async fn upload_artifact_forwards_both_artifact_headers_as_s3_metadata_test() {
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
    .and(header("x-amz-meta-x-artifact-duration", "1234"))
    .respond_with(ResponseTemplate::new(201))
    .mount(&app.storage_server)
    .await;

    let response = client
        .put(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .header("Content-Type", "application/octet-stream")
        .header("x-artifact-tag", artifact_tag)
        .header("x-artifact-duration", "1234")
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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

/// The Turborepo client reads `x-artifact-duration` off the download to report
/// the time the cache saved.
/// See: https://turborepo.dev/api/remote-cache-spec (GET /artifacts/{hash})
#[tokio::test]
async fn download_artifact_returns_artifact_duration_from_s3_metadata_test() {
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

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(
        ResponseTemplate::new(200).insert_header("x-amz-meta-x-artifact-duration", "1234"),
    )
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("x-artifact-duration").unwrap(),
        "1234"
    );
}

/// Another writer may share the bucket, so the download path does not trust the
/// stored value. Turborepo fails the whole read on a duration it cannot parse.
#[tokio::test]
async fn download_artifact_omits_malformed_artifact_duration_test() {
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

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(
        ResponseTemplate::new(200).insert_header("x-amz-meta-x-artifact-duration", "not-a-number"),
    )
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 200);
    assert!(response.headers().get("x-artifact-duration").is_none());
}

/// Counterpart to `upload_artifact_forwards_both_artifact_headers_as_s3_metadata_test`.
/// Both headers must come back on the same download.
/// See: https://turborepo.dev/api/remote-cache-spec (GET /artifacts/{hash})
#[tokio::test]
async fn download_artifact_returns_both_artifact_headers_from_s3_metadata_test() {
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

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(
        ResponseTemplate::new(200)
            .insert_header("x-amz-meta-x-artifact-tag", artifact_tag)
            .insert_header("x-amz-meta-x-artifact-duration", "1234"),
    )
    .mount(&app.storage_server)
    .await;

    let response = client
        .get(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to GET artifact from the cache server");

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("x-artifact-tag").unwrap(),
        artifact_tag
    );
    assert_eq!(
        response.headers().get("x-artifact-duration").unwrap(),
        "1234"
    );
}

#[tokio::test]
async fn list_team_artifacts_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/v8/artifacts", app.address))
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
            app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to HEAD and check artifact from cache server");

    assert_eq!(response.status(), 200);
}

/// The Turborepo client reads the artifact headers off the exists check as well
/// as the download. See: https://turborepo.dev/api/remote-cache-spec
/// (HEAD /artifacts/{hash})
#[tokio::test]
async fn artifact_exists_returns_artifact_metadata_test() {
    let app = spawn_app(None).await;

    let client = reqwest::Client::new();
    let file_mock = TurboArtifactFileMock::new();
    let artifact_tag = "v=1:sha256:abc123";

    Mock::given(path(format!(
        "/{}/{}/{}",
        app.bucket_name, file_mock.team, file_mock.file_hash
    )))
    .and(method("HEAD"))
    .respond_with(
        ResponseTemplate::new(200)
            .insert_header("x-amz-meta-x-artifact-tag", artifact_tag)
            .insert_header("x-amz-meta-x-artifact-duration", "1234"),
    )
    .mount(&app.storage_server)
    .await;

    let response = client
        .head(format!(
            "{}/v8/artifacts/{}?slug={}",
            app.address, file_mock.file_hash, file_mock.team
        ))
        .send()
        .await
        .expect("Failed to HEAD and check artifact from cache server");

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("x-artifact-duration").unwrap(),
        "1234"
    );
    assert_eq!(
        response.headers().get("x-artifact-tag").unwrap(),
        artifact_tag
    );
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
            app.address, file_mock.file_hash, file_mock.team
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
            app.address, file_mock.file_hash, file_mock.team
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
        .get(format!("{}/v8/artifacts/status", app.address))
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request /v8/artifacts/status"));

    assert!(response.status().is_success());

    let response_text = response.text().await.unwrap();
    assert!(response_text.contains("\"status\""));
    assert!(response_text.contains("\"enabled\""));
}

const MULTIPART_BOUNDARY: usize = 8 * 1024 * 1024;
const MULTIPART_TAG: &str = "v=1:sha256:abc123";

/// Issue #620: metadata belongs on initiation once an upload becomes multipart.
/// Exercise both sides of that boundary and verify all bytes arrive in order.
#[tokio::test]
async fn upload_preserves_bytes_and_metadata_across_the_multipart_boundary_test() {
    for size in [
        0,
        17,
        MULTIPART_BOUNDARY - 1,
        MULTIPART_BOUNDARY,
        MULTIPART_BOUNDARY + 17,
    ] {
        let app = spawn_app(None).await;
        let file_mock = TurboArtifactFileMock::new();
        mount_upload_responses(&app, &file_mock).await;
        let body = artifact_body(size);

        let response = reqwest::Client::new()
            .put(format!(
                "{}/v8/artifacts/{}?slug={}",
                app.address, file_mock.file_hash, file_mock.team
            ))
            .header("Content-Type", "application/octet-stream")
            .header("x-artifact-tag", MULTIPART_TAG)
            .header("x-artifact-duration", "1234")
            .body(body.clone())
            .send()
            .await
            .expect("Failed to PUT artifact to the cache server");

        assert_eq!(response.status(), 201, "body size: {size}");
        assert_uploaded_bytes_and_metadata(&app, &body).await;
    }
}

/// A chunked HTTP/1.1 request has no Content-Length. It must retain streaming
/// support when it crosses the multipart boundary, including metadata.
#[tokio::test]
async fn upload_without_content_length_preserves_bytes_and_metadata_test() {
    for size in [
        0,
        17,
        MULTIPART_BOUNDARY - 1,
        MULTIPART_BOUNDARY,
        MULTIPART_BOUNDARY + 17,
    ] {
        let app = spawn_app(None).await;
        let file_mock = TurboArtifactFileMock::new();
        mount_upload_responses(&app, &file_mock).await;
        let body = artifact_body(size);
        let body_to_send = body.clone();
        let host = app.address.strip_prefix("http://").unwrap().to_owned();
        let request_path = format!(
            "/v8/artifacts/{}?slug={}",
            file_mock.file_hash, file_mock.team
        );

        let status = tokio::task::spawn_blocking(move || {
            use std::io::{BufRead, BufReader, Write};
            let mut stream = start_chunked_upload(&host, &request_path, &body_to_send);
            stream
                .write_all(b"0\r\n\r\n")
                .expect("Failed to end request body");
            let mut status_line = String::new();
            BufReader::new(stream)
                .read_line(&mut status_line)
                .expect("Failed to read response status");
            status_line
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse::<u16>()
                .unwrap()
        })
        .await
        .expect("The blocking socket task panicked");

        assert_eq!(status, 201, "body size: {size}");
        assert_uploaded_bytes_and_metadata(&app, &body).await;
    }
}

/// Dropping a client connection midway through a chunked body must abort the
/// in-progress S3 upload instead of completing a truncated artifact.
#[tokio::test]
async fn interrupted_chunked_upload_aborts_multipart_test() {
    let app = spawn_app(None).await;
    let file_mock = TurboArtifactFileMock::new();
    mount_upload_responses(&app, &file_mock).await;
    let host = app.address.strip_prefix("http://").unwrap().to_owned();
    let request_path = format!(
        "/v8/artifacts/{}?slug={}",
        file_mock.file_hash, file_mock.team
    );
    let stream = tokio::task::spawn_blocking(move || {
        start_chunked_upload(&host, &request_path, &artifact_body(MULTIPART_BOUNDARY))
    })
    .await
    .expect("The blocking socket task panicked");

    // Keep the socket open until S3 has accepted a part. There is deliberately
    // no terminating zero chunk, so disconnecting is a body error, not EOF.
    wait_for_s3_method(&app, "PUT").await;
    drop(stream);
    wait_for_s3_method(&app, "DELETE").await;

    let requests = app.storage_server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method == "DELETE")
            .count(),
        1
    );
    assert!(
        !requests.iter().any(|request| {
            request.method == "POST" && request.url.query_pairs().any(|(key, _)| key == "uploadId")
        }),
        "a disconnected upload must not be completed"
    );
}

fn artifact_body(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index % 251) as u8).collect()
}

async fn mount_upload_responses(app: &crate::helpers::TestApp, file: &TurboArtifactFileMock) {
    let object_path = format!("/{}/{}/{}", app.bucket_name, file.team, file.file_hash);
    Mock::given(path(object_path.clone()))
        .and(method("POST"))
        .and(query_param("uploads", ""))
        .and(header("x-amz-meta-x-artifact-tag", MULTIPART_TAG))
        .and(header("x-amz-meta-x-artifact-duration", "1234"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<InitiateMultipartUploadResult><UploadId>upload-1</UploadId></InitiateMultipartUploadResult>"#,
            "application/xml",
        ))
        .mount(&app.storage_server)
        .await;
    Mock::given(path(object_path.clone()))
        .and(method("PUT"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-part\""))
        .mount(&app.storage_server)
        .await;
    Mock::given(path(object_path.clone()))
        .and(method("POST"))
        .and(query_param("uploadId", "upload-1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<CompleteMultipartUploadResult><ETag>"etag-complete"</ETag></CompleteMultipartUploadResult>"#,
            "application/xml",
        ))
        .mount(&app.storage_server)
        .await;
    Mock::given(path(object_path))
        .and(method("DELETE"))
        .and(query_param("uploadId", "upload-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&app.storage_server)
        .await;
}

async fn assert_uploaded_bytes_and_metadata(app: &crate::helpers::TestApp, body: &[u8]) {
    let requests = app.storage_server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["x-amz-meta-x-artifact-tag"],
        MULTIPART_TAG
    );
    assert_eq!(
        requests[0].headers["x-amz-meta-x-artifact-duration"],
        "1234"
    );
    let mut uploaded = Vec::new();
    let parts: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "PUT")
        .collect();
    for (index, part) in parts.iter().enumerate() {
        uploaded.extend_from_slice(&part.body);
        if body.len() >= MULTIPART_BOUNDARY {
            assert!(
                part.url.query_pairs().any(|(key, value)| {
                    key == "partNumber" && value == (index + 1).to_string()
                })
            );
            assert!(part.body.len() <= MULTIPART_BOUNDARY);
        }
    }
    assert!(
        uploaded == body,
        "uploaded bytes differ for body size {}",
        body.len()
    );
    if body.len() < MULTIPART_BOUNDARY {
        assert_eq!(requests.len(), 1, "small uploads need only PutObject");
    } else {
        assert_eq!(requests.len(), parts.len() + 2);
        assert_eq!(requests.first().unwrap().method, "POST");
        let completion = requests.last().unwrap();
        assert_eq!(completion.method, "POST");
        let completion_body = std::str::from_utf8(&completion.body).unwrap();
        for index in 1..=parts.len() {
            assert!(completion_body.contains(&format!("<PartNumber>{index}</PartNumber>")));
        }
    }
}

async fn wait_for_s3_method(app: &crate::helpers::TestApp, expected: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if app
                .storage_server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|request| request.method == expected)
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timed out waiting for an S3 request");
}

/// Starts a real HTTP/1.1 chunked request and leaves it open. The caller may
/// finish the body with a zero chunk or disconnect to test cancellation.
fn start_chunked_upload(host: &str, request_path: &str, body: &[u8]) -> std::net::TcpStream {
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::Duration;

    let mut stream = TcpStream::connect(host).expect("Failed to connect to the cache server");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let headers = format!(
        "PUT {request_path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\nx-artifact-tag: {MULTIPART_TAG}\r\nx-artifact-duration: 1234\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(headers.as_bytes())
        .expect("Failed to write request headers");
    for chunk in body.chunks(64 * 1024) {
        stream
            .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
            .unwrap();
        stream
            .write_all(chunk)
            .expect("Failed to write request body");
        stream.write_all(b"\r\n").unwrap();
    }
    stream
}
