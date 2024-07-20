use std::collections::HashMap;

use actix_web::{
    web::{Bytes, Data, Query},
    Error, HttpRequest, HttpResponse, Responder,
};
use futures::StreamExt;
use serde::Serialize;

use crate::storage::Storage;

#[derive(Serialize)]
struct Artifact {
    filename: String,
}

#[derive(Serialize)]
struct PostTeamArtifactsResponse {
    hashes: Vec<String>,
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

#[tracing::instrument(name = "Check artifact presence", skip(req, storage))]
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

#[tracing::instrument(name = "Store turbo artifact", skip(storage, body))]
pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Bytes) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::BadRequest().finish(),
    };
    match storage.put_file(&artifact_info.file_path(), &body).await {
        Ok(_) => {
            let artifact = Artifact {
                filename: artifact_info.hash.clone(),
            };

            HttpResponse::Created().json(artifact)
        }
        Err(e) => {
            eprintln!("Something went wrong {}", e);
            HttpResponse::BadRequest().finish()
        }
    }
}

#[tracing::instrument(name = "Read turbo artifact", skip(storage))]
pub async fn get_file(req: HttpRequest, storage: Data<Storage>) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(&req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    let stream = match storage.get_file(&artifact_info.file_path()).await {
        Some(response) => response.bytes.map(Result::<Bytes, Error>::Ok),
        None => return HttpResponse::NotFound().finish(),
    };

    HttpResponse::Ok().streaming(stream)
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
