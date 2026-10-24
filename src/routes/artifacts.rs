use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    web::{Bytes, Data, Payload, Query},
};
use futures::StreamExt;
use serde::Serialize;
use tokio_util::io::StreamReader;

use crate::{
    artifact_cache::ArtifactCache,
    domain::{ArtifactId, ArtifactMetadata},
    storage::StorageError,
};

/// When Turborepo is configured with `"signature": true` (turbo.json), the CLI
/// computes an HMAC-SHA256 of each artifact and sends it as the `x-artifact-tag`
/// header on PUT. The server persists this value as S3 object metadata and returns
/// it on GET so the client can verify artifact integrity. Without it, every
/// download fails signature verification and is treated as a cache miss.
/// See: https://turborepo.dev/api/remote-cache-spec
const ARTIFACT_TAG_HEADER: &str = "x-artifact-tag";

/// Turborepo sends the task run time on PUT and reads it back on GET and HEAD to
/// report how much time the cache saved. Without it, every remote cache hit
/// reports zero time saved.
/// See: https://turborepo.dev/api/remote-cache-spec
const ARTIFACT_DURATION_HEADER: &str = "x-artifact-duration";

/// Reads the artifact headers from an upload request.
fn metadata_from_request(req: &HttpRequest) -> ArtifactMetadata {
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

/// Copies stored metadata onto a download or existence response.
fn apply_metadata(metadata: &ArtifactMetadata, builder: &mut HttpResponseBuilder) {
    if let Some(tag) = &metadata.tag {
        builder.insert_header((ARTIFACT_TAG_HEADER, tag.as_str()));
    }
    if let Some(duration) = metadata.duration {
        builder.insert_header((ARTIFACT_DURATION_HEADER, duration.to_string()));
    }
}

/// How much of a rejected duration reaches the log.
const MAX_LOGGED_DURATION_CHARS: usize = 32;

fn parse_duration(raw: &str) -> Option<u64> {
    let parsed = ArtifactMetadata::parse_duration(raw);
    if parsed.is_none() {
        let value: String = raw.chars().take(MAX_LOGGED_DURATION_CHARS).collect();
        tracing::debug!(value = %value, "Ignoring malformed x-artifact-duration");
    }
    parsed
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
#[tracing::instrument(name = "List team artifacts")]
pub async fn post_list_team_artifacts() -> impl Responder {
    tracing::info!("Listing team artifacts");

    HttpResponse::Ok().json(&EMPTY_HASHES)
}

#[tracing::instrument(name = "Check artifact", skip(req, cache))]
pub async fn head_check_file(req: HttpRequest, cache: Data<ArtifactCache>) -> impl Responder {
    let artifact_info = match artifact_id_from_request(&req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    match cache.head(&artifact_info).await {
        Ok(metadata) => {
            let mut builder = HttpResponse::Ok();
            apply_metadata(&metadata, &mut builder);
            builder.finish()
        }
        Err(StorageError::NotFound) => HttpResponse::NotFound().finish(),
        Err(error) => {
            tracing::error!(error = %error, "Could not check artifact on the bucket");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(name = "Store artifact", skip(req, cache, body))]
pub async fn put_file(
    req: HttpRequest,
    cache: Data<ArtifactCache>,
    body: Payload,
) -> impl Responder {
    let artifact_info = match artifact_id_from_request(&req) {
        Some(info) => info,
        None => return HttpResponse::BadRequest().finish(),
    };

    let metadata = metadata_from_request(&req);

    let io_stream = body.map(|chunk| chunk.map_err(std::io::Error::other));
    let mut reader = StreamReader::new(io_stream);

    match cache.put(&artifact_info, &mut reader, &metadata).await {
        Ok(_) => {
            let artifact = Artifact {
                filename: artifact_info.hash.clone(),
            };

            HttpResponse::Created().json(artifact)
        }
        Err(error) => {
            tracing::error!(error = %error, "Could not store artifact on the bucket");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(name = "Read artifact", skip(req, cache))]
pub async fn get_file(req: HttpRequest, cache: Data<ArtifactCache>) -> impl Responder {
    let artifact_info = match artifact_id_from_request(&req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    let result = cache.get(&artifact_info).await;

    let (response, metadata) = match result {
        Ok(response) => response,
        Err(StorageError::NotFound) => return HttpResponse::NotFound().finish(),
        Err(error) => {
            tracing::error!(error = %error, "Could not read artifact from the bucket");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let stream = response.bytes.map(|maybe_chunk| match maybe_chunk {
        Ok(bytes) => Result::<Bytes, actix_web::error::Error>::Ok(bytes),
        Err(error) => {
            tracing::error!(error = error.to_string(), "Chunk stream error");
            Result::<Bytes, actix_web::error::Error>::Err(
                actix_web::error::ErrorInternalServerError("Error while streaming artifact"),
            )
        }
    });

    let mut builder = HttpResponse::Ok();

    apply_metadata(&metadata, &mut builder);

    builder.streaming(stream)
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

fn artifact_id_from_request(req: &HttpRequest) -> Option<ArtifactId> {
    let hash = req.match_info().get("hash")?.to_owned();
    Some(ArtifactId {
        team: extract_team_from_req(req),
        hash,
    })
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
    fn selects_the_same_namespace_for_each_artifact_method() {
        for method in [
            actix_web::http::Method::PUT,
            actix_web::http::Method::GET,
            actix_web::http::Method::HEAD,
        ] {
            let req = TestRequest::with_uri("/v8/artifacts/abc123?teamId=fallback&slug=preferred")
                .method(method)
                .param("hash", "abc123")
                .to_http_request();
            let id = artifact_id_from_request(&req).unwrap();
            assert_eq!(id.team, "preferred");
            assert_eq!(id.hash, "abc123");
        }
    }

    #[test]
    fn falls_back_to_team_id_then_default_namespace() {
        let team_id = TestRequest::with_uri("/v8/artifacts/abc123?teamId=chosen")
            .param("hash", "abc123")
            .to_http_request();
        let no_team = TestRequest::with_uri("/v8/artifacts/abc123")
            .param("hash", "abc123")
            .to_http_request();

        assert_eq!(artifact_id_from_request(&team_id).unwrap().team, "chosen");
        assert_eq!(artifact_id_from_request(&no_team).unwrap().team, "no_team");
    }

    #[test]
    fn reads_both_artifact_headers_from_the_request() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_TAG_HEADER, TAG))
            .insert_header((ARTIFACT_DURATION_HEADER, "1234"))
            .to_http_request();

        assert_eq!(
            metadata_from_request(&req),
            ArtifactMetadata {
                tag: Some(TAG.to_owned()),
                duration: Some(1234),
            }
        );
    }

    #[test]
    fn reads_no_artifact_headers_from_a_bare_request() {
        let req = TestRequest::default().to_http_request();

        assert_eq!(metadata_from_request(&req), ArtifactMetadata::default());
    }

    /// Turborepo fails the whole cache read on a duration it cannot parse, so a
    /// bad value never reaches the bucket.
    #[test]
    fn drops_a_malformed_duration_from_the_request() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_DURATION_HEADER, "not-a-number"))
            .to_http_request();

        assert_eq!(metadata_from_request(&req).duration, None);
    }
}
