use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    web::{Bytes, Data, Query},
};
use futures::StreamExt;
use serde::Serialize;

use crate::storage::Storage;

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
        true => HttpResponse::Ok().finish(),
        false => HttpResponse::NotFound().finish(),
    }
}

#[tracing::instrument(name = "Store artifact", skip(storage, body))]
pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Bytes) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::BadRequest().finish(),
    };

    let metadata = req
        .headers()
        .get(ARTIFACT_TAG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|tag| HashMap::from([(ARTIFACT_TAG_HEADER.to_owned(), tag.to_owned())]));

    match storage
        .put_file(&artifact_info.file_path(), &body, metadata.as_ref())
        .await
    {
        Ok(_) => {
            let artifact = Artifact {
                filename: artifact_info.hash.clone(),
            };

            HttpResponse::Created().json(artifact)
        }
        Err(error) => {
            tracing::error!("Could not store file error={}", error);
            HttpResponse::BadRequest().finish()
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
    let Some(response) = maybe_response else {
        return HttpResponse::NotFound().finish();
    };

    let stream = response.bytes.map(|maybe_chunk| match maybe_chunk {
        Ok(bytes) => Result::<Bytes, actix_web::error::Error>::Ok(bytes),
        Err(error) => {
            tracing::error!(error = error.to_string(), "Chunk stream error");
            Result::<Bytes, actix_web::error::Error>::Err(actix_web::error::ErrorBadRequest(
                "Error while streaming artifact",
            ))
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
        let hash = match req.match_info().get("hash") {
            Some(h) => h.to_owned(),
            None => return None,
        };

        let team = extract_team_from_req(req);

        Some(ArtifactRequest { hash, team })
    }
}

const DUMMY_CACHE_STATUS: CacheStatus = CacheStatus { status: "enabled" };

pub async fn artifacts_status() -> impl Responder {
    HttpResponse::Ok().json(DUMMY_CACHE_STATUS)
}
