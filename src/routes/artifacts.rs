use std::collections::HashMap;
use std::fmt;
use std::future::{Ready, ready};

use actix_web::{
    FromRequest, HttpRequest, HttpResponse, HttpResponseBuilder, Responder, ResponseError, dev,
    http::{Method, StatusCode},
    web::{Data, Payload, Query},
};
use futures::StreamExt;
use serde::Serialize;
use tokio_util::io::StreamReader;

use crate::domain::{
    ARTIFACT_DURATION_HEADER, ARTIFACT_TAG_HEADER, ArtifactId, ArtifactMetadata, CacheError,
    artifact::parse_duration,
};
use crate::storage::Storage;
use crate::usecases::ArtifactCache;

/// The concrete cache the server wires up in `startup::run`.
pub type Cache = ArtifactCache<Storage>;

/// Reads the artifact headers from an upload request.
fn metadata_from_headers(req: &HttpRequest) -> ArtifactMetadata {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };

    ArtifactMetadata {
        tag: header(ARTIFACT_TAG_HEADER).map(str::to_owned),
        duration: header(ARTIFACT_DURATION_HEADER).and_then(parse_duration),
    }
}

/// Copies the artifact headers onto a download response.
fn apply_metadata_headers(metadata: &ArtifactMetadata, builder: &mut HttpResponseBuilder) {
    if let Some(tag) = &metadata.tag {
        builder.insert_header((ARTIFACT_TAG_HEADER, tag.as_str()));
    }

    if let Some(duration) = metadata.duration {
        builder.insert_header((ARTIFACT_DURATION_HEADER, duration.to_string()));
    }
}

/// Rejection for a request whose path holds no artifact hash.
#[derive(Debug)]
pub struct InvalidArtifactPath {
    status: StatusCode,
}

impl fmt::Display for InvalidArtifactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request path holds no artifact hash")
    }
}

impl ResponseError for InvalidArtifactPath {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::new(self.status)
    }
}

impl FromRequest for ArtifactId {
    type Error = InvalidArtifactPath;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let id = req
            .match_info()
            .get("hash")
            .filter(|hash| !hash.is_empty())
            .map(|hash| ArtifactId {
                team: extract_team_from_req(req),
                hash: hash.to_owned(),
            });

        match id {
            Some(id) => ready(Ok(id)),
            // Matches the pre-refactor handlers: PUT answered 400, GET and HEAD 404.
            None => {
                let status = if req.method() == Method::PUT {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::NOT_FOUND
                };

                ready(Err(InvalidArtifactPath { status }))
            }
        }
    }
}

#[derive(Serialize)]
struct Artifact {
    filename: String,
}

#[derive(Serialize)]
struct PostTeamArtifactsResponse {
    hashes: Vec<String>,
}

#[derive(Serialize)]
struct CacheStatus {
    status: &'static str,
}

const EMPTY_HASHES: PostTeamArtifactsResponse = PostTeamArtifactsResponse { hashes: vec![] };

/// As of now, we do not need to list all artifacts for a given
/// team. This seems to be an Admin endpoint for Vercel to map/reduce
/// on the artifacts for a given team and report metrics.
#[tracing::instrument(name = "List team artifacts", skip(req))]
pub async fn post_list_team_artifacts(req: HttpRequest) -> impl Responder {
    let team = extract_team_from_req(&req);

    tracing::info!(team = team, "Listing team artifacts");

    HttpResponse::Ok().json(&EMPTY_HASHES)
}

#[tracing::instrument(name = "Check artifact", skip(cache))]
pub async fn head_check_file(
    id: ArtifactId,
    cache: Data<Cache>,
) -> Result<HttpResponse, CacheError> {
    let metadata = cache.check(&id).await?;

    let mut builder = HttpResponse::Ok();
    apply_metadata_headers(&metadata, &mut builder);

    Ok(builder.finish())
}

#[tracing::instrument(name = "Store artifact", skip(req, cache, body))]
pub async fn put_file(
    req: HttpRequest,
    id: ArtifactId,
    cache: Data<Cache>,
    body: Payload,
) -> Result<HttpResponse, CacheError> {
    let metadata = metadata_from_headers(&req);

    let io_stream = body.map(|chunk| chunk.map_err(std::io::Error::other));
    let mut reader = StreamReader::new(io_stream);

    cache.store(&id, metadata, &mut reader).await?;

    let artifact = Artifact {
        filename: id.hash.clone(),
    };

    Ok(HttpResponse::Created().json(artifact))
}

#[tracing::instrument(name = "Read artifact", skip(cache))]
pub async fn get_file(id: ArtifactId, cache: Data<Cache>) -> Result<HttpResponse, CacheError> {
    let (body, metadata) = cache.fetch(&id).await?;

    let stream = body.map(|maybe_chunk| {
        maybe_chunk.map_err(|error| {
            tracing::error!(error = %error, "Chunk stream error");
            actix_web::error::ErrorInternalServerError("Error while streaming artifact")
        })
    });

    let mut builder = HttpResponse::Ok();
    apply_metadata_headers(&metadata, &mut builder);

    Ok(builder.streaming(stream))
}

fn extract_team_from_req(req: &HttpRequest) -> String {
    let query_string = Query::<HashMap<String, String>>::from_query(req.query_string()).unwrap();
    let default_team_name = "no_team".to_owned();
    query_string
        .get("slug")
        .or_else(|| query_string.get("teamId"))
        .unwrap_or(&default_team_name)
        .to_string()
}

const DUMMY_CACHE_STATUS: CacheStatus = CacheStatus { status: "enabled" };

pub async fn artifacts_status() -> impl Responder {
    HttpResponse::Ok().json(DUMMY_CACHE_STATUS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use pretty_assertions::assert_eq;

    const TAG: &str = "v=1:sha256:abc123";

    #[test]
    fn reads_both_artifact_headers_from_the_request() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_TAG_HEADER, TAG))
            .insert_header((ARTIFACT_DURATION_HEADER, "1234"))
            .to_http_request();

        assert_eq!(
            metadata_from_headers(&req),
            ArtifactMetadata {
                tag: Some(TAG.to_owned()),
                duration: Some(1234),
            }
        );
    }

    #[test]
    fn reads_no_artifact_headers_from_a_bare_request() {
        let req = TestRequest::default().to_http_request();

        assert_eq!(metadata_from_headers(&req), ArtifactMetadata::default());
    }

    /// Turborepo fails the whole cache read on a duration it cannot parse, so a
    /// bad value never reaches the bucket.
    #[test]
    fn drops_a_malformed_duration_from_the_request() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_DURATION_HEADER, "not-a-number"))
            .to_http_request();

        assert_eq!(metadata_from_headers(&req).duration, None);
    }

    /// The bucket holds the parsed number, not the bytes the client sent.
    #[test]
    fn writes_the_parsed_duration_to_storage() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_DURATION_HEADER, "007"))
            .to_http_request();

        let metadata = metadata_from_headers(&req).into_key_values();

        assert_eq!(
            metadata.get(ARTIFACT_DURATION_HEADER).map(String::as_str),
            Some("7")
        );
    }

    #[tokio::test]
    async fn extracts_the_artifact_id_from_path_and_query() {
        let req = TestRequest::with_uri("/v8/artifacts/abc123?slug=my-team")
            .param("hash", "abc123")
            .to_http_request();

        let id = ArtifactId::from_request(&req, &mut dev::Payload::None)
            .await
            .unwrap();

        assert_eq!(id.team, "my-team");
        assert_eq!(id.hash, "abc123");
    }

    #[tokio::test]
    async fn falls_back_to_no_team_without_a_team_query() {
        let req = TestRequest::with_uri("/v8/artifacts/abc123")
            .param("hash", "abc123")
            .to_http_request();

        let id = ArtifactId::from_request(&req, &mut dev::Payload::None)
            .await
            .unwrap();

        assert_eq!(id.team, "no_team");
    }

    /// Matches the pre-refactor handlers: PUT answered 400, GET and HEAD 404.
    #[tokio::test]
    async fn rejects_a_missing_hash_with_the_method_status() {
        let req = TestRequest::default().method(Method::PUT).to_http_request();
        let error = ArtifactId::from_request(&req, &mut dev::Payload::None)
            .await
            .unwrap_err();
        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);

        let req = TestRequest::default().to_http_request();
        let error = ArtifactId::from_request(&req, &mut dev::Payload::None)
            .await
            .unwrap_err();
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }
}
