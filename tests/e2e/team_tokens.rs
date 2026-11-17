use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;

use decay::app_settings::get_settings;
use pretty_assertions::assert_eq;
use reqwest::{Method, Response};
use sha2::{Digest, Sha256};
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{TOKENS_RELOAD_INTERVAL, TestApp, TestAppConfig, spawn_app};

const ALPHA_TOKEN: &str = "alpha-token";
const BETA_TOKEN: &str = "beta-token";

/// The content of a team file: the token's SHA-256 as lowercase hex, with the
/// trailing newline `sha256sum` and most editors leave.
fn hash_line(token: &str) -> String {
    let hex: String = Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hex}\n")
}

/// A fresh tokens directory with one file per `(team, token)`.
fn tokens_dir(name: &str, teams: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "decay-e2e-team-tokens-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (team, token) in teams {
        fs::write(dir.join(team), hash_line(token)).unwrap();
    }
    dir
}

async fn spawn_with_teams(name: &str) -> (TestApp, PathBuf) {
    let dir = tokens_dir(name, &[("alpha", ALPHA_TOKEN), ("beta", BETA_TOKEN)]);
    let app = spawn_app(Some(TestAppConfig {
        turbo_tokens_dir: Some(dir.clone()),
        ..Default::default()
    }))
    .await;
    (app, dir)
}

async fn request(
    app: &TestApp,
    method: Method,
    endpoint: &str,
    auth_header: Option<String>,
) -> Response {
    let client = reqwest::Client::new();
    let mut request = client.request(method, format!("{}{}", app.address, endpoint));
    if let Some(value) = auth_header {
        request = request.header("Authorization", value);
    }
    request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to request {endpoint}"))
}

fn bearer(token: &str) -> Option<String> {
    Some(format!("Bearer {token}"))
}

async fn mount_bucket_object(app: &TestApp, team: &str, hash: &str) {
    Mock::given(path(format!("/{}/{team}/{hash}", app.bucket_name)))
        .and(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("artifact"))
        .mount(&app.storage_server)
        .await;
}

#[tokio::test]
async fn reads_an_artifact_with_the_token_of_its_team() {
    let (app, _) = spawn_with_teams("read").await;
    mount_bucket_object(&app, "alpha", "abc123").await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(ALPHA_TOKEN),
    )
    .await;

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "artifact");
}

#[tokio::test]
async fn stores_an_artifact_under_the_team_the_token_was_checked_for() {
    let (app, _) = spawn_with_teams("store").await;

    Mock::given(path(format!("/{}/alpha/abc123", app.bucket_name)))
        .and(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.storage_server)
        .await;

    // `slug` wins over `teamId` for both the check and the object key.
    let response = reqwest::Client::new()
        .put(format!(
            "{}/v8/artifacts/abc123?teamId=beta&slug=alpha",
            app.address
        ))
        .header("Authorization", format!("Bearer {ALPHA_TOKEN}"))
        .body("artifact")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
}

#[tokio::test]
async fn accepts_the_team_as_team_id() {
    let (app, _) = spawn_with_teams("team-id").await;
    mount_bucket_object(&app, "beta", "abc123").await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?teamId=beta",
        bearer(BETA_TOKEN),
    )
    .await;

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn forbidden_when_the_token_is_for_another_team() {
    let (app, _) = spawn_with_teams("other-team").await;

    for method in [Method::GET, Method::PUT, Method::HEAD] {
        let response = request(
            &app,
            method.clone(),
            "/v8/artifacts/abc123?slug=alpha",
            bearer(BETA_TOKEN),
        )
        .await;
        assert_eq!(response.status(), 403, "{method}");
    }

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(BETA_TOKEN),
    )
    .await;
    assert_eq!(
        response.text().await.unwrap(),
        r#"{"error":"TURBO_TOKEN is not for this team"}"#
    );

    // Nothing reached the bucket.
    assert!(
        app.storage_server
            .received_requests()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn forbidden_when_the_request_names_no_team() {
    let (app, _) = spawn_with_teams("no-team").await;

    for endpoint in ["/v8/artifacts/abc123", "/v8/artifacts/status"] {
        let response = request(&app, Method::GET, endpoint, bearer(ALPHA_TOKEN)).await;
        assert_eq!(response.status(), 403, "{endpoint}");
    }
    assert!(
        app.storage_server
            .received_requests()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn forbidden_when_the_team_has_no_token_file() {
    let (app, _) = spawn_with_teams("unknown-team").await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=gamma",
        bearer(ALPHA_TOKEN),
    )
    .await;

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn unauthorized_when_the_token_is_missing() {
    let (app, _) = spawn_with_teams("missing").await;

    let response = request(&app, Method::GET, "/v8/artifacts/abc123?slug=alpha", None).await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn unauthorized_when_the_token_is_unknown() {
    let (app, _) = spawn_with_teams("unknown").await;

    for endpoint in ["/v8/artifacts/abc123?slug=alpha", "/v8/artifacts/abc123"] {
        let response = request(&app, Method::GET, endpoint, bearer("not-a-token")).await;
        assert_eq!(response.status(), 401, "{endpoint}");
    }
}

#[tokio::test]
async fn unauthorized_with_the_plain_hash_as_token() {
    let (app, _) = spawn_with_teams("hash-as-token").await;

    let hash = hash_line(ALPHA_TOKEN);
    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(hash.trim_end()),
    )
    .await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn health_check_needs_no_token() {
    let (app, _) = spawn_with_teams("health").await;

    let response = request(&app, Method::GET, "/management/health", None).await;

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn accepts_a_team_added_after_start() {
    let (app, dir) = spawn_with_teams("added").await;
    mount_bucket_object(&app, "gamma", "abc123").await;

    fs::write(dir.join("gamma"), hash_line("gamma-token")).unwrap();
    tokio::time::sleep(TOKENS_RELOAD_INTERVAL * 5).await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=gamma",
        bearer("gamma-token"),
    )
    .await;
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn rejects_a_token_after_it_is_rotated() {
    let (app, dir) = spawn_with_teams("rotated").await;

    fs::write(dir.join("alpha"), hash_line("new-alpha-token")).unwrap();
    tokio::time::sleep(TOKENS_RELOAD_INTERVAL * 5).await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(ALPHA_TOKEN),
    )
    .await;
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn keeps_the_last_tokens_when_the_directory_becomes_unreadable() {
    let (app, dir) = spawn_with_teams("unreadable").await;
    mount_bucket_object(&app, "alpha", "abc123").await;

    fs::remove_dir_all(&dir).unwrap();
    tokio::time::sleep(TOKENS_RELOAD_INTERVAL * 5).await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(ALPHA_TOKEN),
    )
    .await;
    assert_eq!(response.status(), 200);
}

fn start(turbo_token: Option<&str>, turbo_tokens_dir: Option<PathBuf>) -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut settings = get_settings();
    settings.turbo_token = turbo_token.map(Into::into);
    settings.turbo_tokens_dir = turbo_tokens_dir;
    decay::startup::run(listener, settings).map(drop)
}

#[tokio::test]
async fn refuses_to_start_with_both_a_shared_token_and_a_tokens_dir() {
    let dir = tokens_dir("both", &[("alpha", ALPHA_TOKEN)]);

    let error = start(Some("shared-token"), Some(dir)).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        error
            .to_string()
            .contains("TURBO_TOKEN and TURBO_TOKENS_DIR")
    );
}

#[tokio::test]
async fn refuses_to_start_when_the_tokens_dir_cannot_be_read() {
    let dir = tokens_dir("absent", &[]).join("does-not-exist");

    let error = start(None, Some(dir)).unwrap_err();

    assert!(error.to_string().contains("TURBO_TOKENS_DIR"), "{error}");
}

#[tokio::test]
async fn starts_with_an_empty_tokens_dir_and_rejects_every_token() {
    let dir = tokens_dir("empty", &[]);
    let app = spawn_app(Some(TestAppConfig {
        turbo_tokens_dir: Some(dir),
        ..Default::default()
    }))
    .await;

    let response = request(
        &app,
        Method::GET,
        "/v8/artifacts/abc123?slug=alpha",
        bearer(ALPHA_TOKEN),
    )
    .await;
    assert_eq!(response.status(), 401);
}
