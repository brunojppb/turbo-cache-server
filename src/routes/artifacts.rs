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
    fn from_storage(metadata: &HashMap<String, String>) -> Self {
        Self {
            tag: metadata.get(ARTIFACT_TAG_HEADER).cloned(),
            duration: metadata
                .get(ARTIFACT_DURATION_HEADER)
                .map(String::as_str)
                .and_then(parse_duration),
        }
    }

    /// The key-value pairs to persist as S3 user metadata.
    fn to_storage(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        if let Some(tag) = &self.tag {
            metadata.insert(ARTIFACT_TAG_HEADER.to_owned(), tag.clone());
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

/// Turborepo fails the whole cache read on a duration it cannot parse, so a
/// value the server cannot vouch for is dropped instead of passed on.
fn parse_duration(raw: &str) -> Option<u64> {
    match raw.parse::<u64>() {
        Ok(duration) => Some(duration),
        Err(_) => {
            tracing::warn!(value = raw, "Ignoring malformed x-artifact-duration");
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

    match storage.file_exists(&artifact_info.file_path()).await {
        Ok(true) => HttpResponse::Ok().finish(),
        Ok(false) => HttpResponse::NotFound().finish(),
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

    let metadata = ArtifactMetadata::from_request(&req).to_storage();

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

    ArtifactMetadata::from_storage(&metadata).apply(&mut builder);

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
