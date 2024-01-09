use std::collections::HashMap;

use actix_web::{
    web::{Bytes, Data, Query},
    HttpRequest, HttpResponse, Responder,
};
use serde::Serialize;

use crate::storage::Storage;

#[derive(Serialize)]
struct Artifact {
    filename: String,
}

pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Bytes) -> impl Responder {
    let artifact_info = match artifact_info_from_req(req) {
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
            println!("Something went wrong {}", e);
            HttpResponse::BadRequest().finish()
        }
    }
}

pub async fn get_file(req: HttpRequest, storage: Data<Storage>) -> impl Responder {
    let artifact_info = match artifact_info_from_req(req) {
        Some(info) => info,
        None => return HttpResponse::NotFound().finish(),
    };

    match storage.get_file(&artifact_info.file_path()).await {
        Some(file) => HttpResponse::Ok().body(file.to_vec()),
        None => HttpResponse::NotFound().finish(),
    }
}

struct ArtifactRequest {
    hash: String,
    team: String,
}

impl ArtifactRequest {
    fn file_path(&self) -> String {
        format!("/{}/{}", self.team, self.hash)
    }
}

fn artifact_info_from_req(req: HttpRequest) -> Option<ArtifactRequest> {
    let hash = match req.match_info().get("hash") {
        Some(h) => h.to_owned(),
        None => return None,
    };

    let query_string = Query::<HashMap<String, String>>::from_query(req.query_string()).unwrap();
    let default_team_name = "no_team".to_owned();
    let team = query_string
        .get("slug")
        .unwrap_or(&default_team_name)
        .to_string();

    Some(ArtifactRequest { hash, team })
}
