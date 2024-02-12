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

#[tracing::instrument(name = "Store cache artifact", skip(storage, body))]
pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Bytes) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(req) {
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
            tracing::error!("Could not store file on S3: {:?}", e);
            HttpResponse::BadRequest().finish()
        }
    }
}

#[tracing::instrument(name = "Read cache artifact", skip(storage))]
pub async fn get_file(req: HttpRequest, storage: Data<Storage>) -> impl Responder {
    let artifact_info = match ArtifactRequest::from(req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    let stream = match storage.get_file(&artifact_info.file_path()).await {
        Some(response) => response.bytes.map(Result::<Bytes, Error>::Ok),
        None => return HttpResponse::NotFound().finish(),
    };

    HttpResponse::Ok().streaming(stream)
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

    fn from(req: HttpRequest) -> Option<Self> {
        let hash = match req.match_info().get("hash") {
            Some(h) => h.to_owned(),
            None => return None,
        };

        let query_string =
            Query::<HashMap<String, String>>::from_query(req.query_string()).unwrap();
        let default_team_name = "no_team".to_owned();
        let team = query_string
            .get("slug")
            .unwrap_or(&default_team_name)
            .to_string();

        Some(ArtifactRequest { hash, team })
    }
}
