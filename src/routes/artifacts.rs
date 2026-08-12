use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    web::{Bytes, Data, Payload, Query},
};
use futures::StreamExt;
use serde::Serialize;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::StreamReader;

use crate::storage::{Storage, StorageError};

/// When Turborepo is configured with `"signature": true` (turbo.json), the CLI
/// computes an HMAC-SHA256 of each artifact and sends it as the `x-artifact-tag`
/// header on PUT. The server persists this value as S3 object metadata and returns
/// it on GET so the client can verify artifact integrity. Without it, every
/// download fails signature verification and is treated as a cache miss.
/// See: https://turborepo.dev/api/remote-cache-spec
const ARTIFACT_TAG_HEADER: &str = "x-artifact-tag";

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

    let metadata = req
        .headers()
        .get(ARTIFACT_TAG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|tag| HashMap::from([(ARTIFACT_TAG_HEADER.to_owned(), tag.to_owned())]));

    let content_length = req
        .headers()
        .get(actix_web::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let reader = send_reader(body);

    match storage
        .put_file_stream(
            &artifact_info.file_path(),
            reader,
            content_length,
            metadata.as_ref(),
        )
        .await
    {
        Ok(_) => {
            let artifact = Artifact {
                filename: artifact_info.hash.clone(),
            };

            HttpResponse::Created().json(artifact)
        }
        Err(StorageError::LengthRequired) => {
            tracing::warn!("Artifact at or above the part size arrived without a Content-Length");
            HttpResponse::build(actix_web::http::StatusCode::LENGTH_REQUIRED).finish()
        }
        Err(error) => {
            tracing::error!(error = %error, "Could not store artifact on the bucket");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// Chunks in flight between the request body and the S3 upload. Bounds the
/// bridge's memory to this many payload chunks.
const PAYLOAD_CHANNEL_DEPTH: usize = 4;

/// Bridges the request body, which is not `Send`, to a reader the AWS transfer
/// manager accepts.
fn send_reader(
    mut body: Payload,
) -> StreamReader<ReceiverStream<Result<Bytes, std::io::Error>>, Bytes> {
    let (tx, rx) = tokio::sync::mpsc::channel(PAYLOAD_CHANNEL_DEPTH);

    actix_web::rt::spawn(async move {
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(std::io::Error::other);
            let failed = chunk.is_err();
            // A send error means the upload dropped the reader, so stop reading.
            if tx.send(chunk).await.is_err() {
                break;
            }
            if failed {
                break;
            }
        }
    });

    StreamReader::new(ReceiverStream::new(rx))
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

    let stream = response.map(|maybe_chunk| match maybe_chunk {
        Ok(bytes) => Result::<Bytes, actix_web::error::Error>::Ok(bytes),
        Err(error) => {
            tracing::error!(error = error.to_string(), "Chunk stream error");
            Result::<Bytes, actix_web::error::Error>::Err(
                actix_web::error::ErrorInternalServerError("Error while streaming artifact"),
            )
        }
    });

    let mut builder = HttpResponse::Ok();

    if let Some(tag) = metadata.as_ref().and_then(|m| m.get(ARTIFACT_TAG_HEADER)) {
        builder.insert_header((ARTIFACT_TAG_HEADER, tag.as_str()));
    }

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
