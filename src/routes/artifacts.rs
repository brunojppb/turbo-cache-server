use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    web::{Bytes, Data, Payload, Query},
};
use futures::StreamExt;
use serde::Serialize;
use tokio_util::io::StreamReader;

use crate::storage::{Storage, StorageError};

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

/// The artifact headers Turborepo sends on upload and expects back on download.
/// They travel as S3 user metadata.
#[derive(Debug, Default, PartialEq, Eq)]
struct ArtifactMetadata {
    tag: Option<String>,
    duration: Option<u64>,
}

impl ArtifactMetadata {
    /// Reads the artifact headers from an upload request.
    fn from_request(req: &HttpRequest) -> Self {
        let header = |name: &str| req.headers().get(name).and_then(|value| value.to_str().ok());

        Self {
            tag: header(ARTIFACT_TAG_HEADER).map(str::to_owned),
            duration: header(ARTIFACT_DURATION_HEADER).and_then(parse_duration),
        }
    }

    /// Reads the artifact headers from the metadata stored on the S3 object.
    fn from_storage(mut metadata: HashMap<String, String>) -> Self {
        Self {
            tag: metadata.remove(ARTIFACT_TAG_HEADER),
            duration: metadata
                .remove(ARTIFACT_DURATION_HEADER)
                .as_deref()
                .and_then(parse_duration),
        }
    }

    /// The key-value pairs to persist as S3 user metadata.
    fn into_storage(self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        if let Some(tag) = self.tag {
            metadata.insert(ARTIFACT_TAG_HEADER.to_owned(), tag);
        }

        if let Some(duration) = self.duration {
            metadata.insert(ARTIFACT_DURATION_HEADER.to_owned(), duration.to_string());
        }

        metadata
    }

    /// Copies the artifact headers onto a download response.
    fn apply(&self, builder: &mut HttpResponseBuilder) {
        if let Some(tag) = &self.tag {
            builder.insert_header((ARTIFACT_TAG_HEADER, tag.as_str()));
        }

        if let Some(duration) = self.duration {
            builder.insert_header((ARTIFACT_DURATION_HEADER, duration.to_string()));
        }
    }
}

/// How much of a rejected duration reaches the log.
const MAX_LOGGED_DURATION_CHARS: usize = 32;

/// Turborepo fails the whole cache read on a duration it cannot parse, so a
/// value the server cannot vouch for is dropped instead of passed on.
fn parse_duration(raw: &str) -> Option<u64> {
    match raw.parse::<u64>() {
        Ok(duration) => Some(duration),
        Err(_) => {
            // A client picks this value and can repeat it on every request, so
            // it stays off the warning path and never reaches the log in full.
            let value: String = raw.chars().take(MAX_LOGGED_DURATION_CHARS).collect();
            tracing::debug!(value = %value, "Ignoring malformed x-artifact-duration");
            None
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

#[tracing::instrument(name = "Check artifact", skip(req, storage))]
pub async fn head_check_file(req: HttpRequest, storage: Data<Storage>) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    match storage.head_file(&artifact_info.file_path()).await {
        Ok(metadata) => {
            let mut builder = HttpResponse::Ok();
            ArtifactMetadata::from_storage(metadata).apply(&mut builder);
            builder.finish()
        }
        Err(StorageError::NotFound) => HttpResponse::NotFound().finish(),
        Err(error) => {
            tracing::error!(error = %error, "Could not check artifact on the bucket");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(name = "Store artifact", skip(storage, body))]
pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Payload) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::BadRequest().finish(),
    };

    let metadata = ArtifactMetadata::from_request(&req).into_storage();

    let io_stream = body.map(|chunk| chunk.map_err(std::io::Error::other));
    let mut reader = StreamReader::new(io_stream);

    match storage
        .put_file_stream(&artifact_info.file_path(), &mut reader, &metadata)
        .await
    {
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

#[tracing::instrument(name = "Read artifact", skip(storage))]
pub async fn get_file(req: HttpRequest, storage: Data<Storage>) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    let file_path = artifact_info.file_path();

    let (maybe_response, metadata) = tokio::join!(
        storage.get_file(&file_path),
        storage.get_metadata(&file_path),
    );

    let response = match maybe_response {
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

    ArtifactMetadata::from_storage(metadata).apply(&mut builder);

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

struct ArtifactRequest {
    hash: String,
    team: String,
}

impl ArtifactRequest {
    /// File path as represented in the S3 storage
    fn file_path(&self) -> String {
        format!("/{}/{}", self.team, self.hash)
    }

    fn from(req: &HttpRequest) -> Option<Self> {
        let hash = {
            let h = req.match_info().get("hash")?;
            h.to_owned()
        };

        let team = extract_team_from_req(req);

        Some(ArtifactRequest { hash, team })
    }
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
            ArtifactMetadata::from_request(&req),
            ArtifactMetadata {
                tag: Some(TAG.to_owned()),
                duration: Some(1234),
            }
        );
    }

    #[test]
    fn reads_no_artifact_headers_from_a_bare_request() {
        let req = TestRequest::default().to_http_request();

        assert_eq!(
            ArtifactMetadata::from_request(&req),
            ArtifactMetadata::default()
        );
    }

    /// Turborepo fails the whole cache read on a duration it cannot parse, so a
    /// bad value never reaches the bucket.
    #[test]
    fn drops_a_malformed_duration_from_the_request() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_DURATION_HEADER, "not-a-number"))
            .to_http_request();

        assert_eq!(ArtifactMetadata::from_request(&req).duration, None);
    }

    /// The bucket holds the parsed number, not the bytes the client sent.
    #[test]
    fn writes_the_parsed_duration_to_storage() {
        let req = TestRequest::default()
            .insert_header((ARTIFACT_DURATION_HEADER, "007"))
            .to_http_request();

        let metadata = ArtifactMetadata::from_request(&req).into_storage();

        assert_eq!(
            metadata.get(ARTIFACT_DURATION_HEADER).map(String::as_str),
            Some("7")
        );
    }

    #[test]
    fn reads_both_artifact_headers_from_storage() {
        let stored = HashMap::from([
            (ARTIFACT_TAG_HEADER.to_owned(), TAG.to_owned()),
            (ARTIFACT_DURATION_HEADER.to_owned(), "1234".to_owned()),
        ]);

        assert_eq!(
            ArtifactMetadata::from_storage(stored),
            ArtifactMetadata {
                tag: Some(TAG.to_owned()),
                duration: Some(1234),
            }
        );
    }

    /// Another writer may share the bucket, so the read path does not trust the
    /// stored value either.
    #[test]
    fn drops_a_malformed_duration_from_storage() {
        let stored = HashMap::from([(
            ARTIFACT_DURATION_HEADER.to_owned(),
            "not-a-number".to_owned(),
        )]);

        assert_eq!(ArtifactMetadata::from_storage(stored).duration, None);
    }
}
