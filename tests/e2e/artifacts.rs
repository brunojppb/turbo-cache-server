use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, TurboArtifactFileMock};

#[tokio::test]
async fn upload_artifact_to_s3_test() {
    let app = spawn_app().await;

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

#[tokio::test]
async fn download_artifact_from_s3_test() {
    let app = spawn_app().await;

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

#[tokio::test]
async fn list_team_artifacts_test() {
    let app = spawn_app().await;

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
    let app = spawn_app().await;

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
        .expect("Failed to HEAD and check artifact from Sake");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn artifact_does_not_exist_test() {
    let app = spawn_app().await;

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
        .expect("Failed to HEAD and check artifact from Sake");

    assert_eq!(response.status(), 404);
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
