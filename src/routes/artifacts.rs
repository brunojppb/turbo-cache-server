use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    web::{Bytes, Data, Payload, Query},
};
use futures::StreamExt;
use serde::Serialize;
use tokio_util::io::StreamReader;

use crate::domain::{
    ARTIFACT_DURATION_HEADER, ARTIFACT_TAG_HEADER, ArtifactId, ArtifactMetadata,
    artifact::parse_duration,
};
use crate::storage::{Storage, StorageError};

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
    let artifact_id = match artifact_id_from_req(&req) {
        Some(id) => id,
        None => return HttpResponse::NotFound().finish(),
    };

    match storage.head_file(&artifact_id.object_path()).await {
        Ok(metadata) => {
            let mut builder = HttpResponse::Ok();
            apply_metadata_headers(&ArtifactMetadata::from_key_values(metadata), &mut builder);
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
    let artifact_id = match artifact_id_from_req(&req) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().finish(),
    };

    let metadata = metadata_from_headers(&req).into_key_values();

    let io_stream = body.map(|chunk| chunk.map_err(std::io::Error::other));
    let mut reader = StreamReader::new(io_stream);

    match storage
        .put_file_stream(&artifact_id.object_path(), &mut reader, &metadata)
        .await
    {
        Ok(_) => {
            let artifact = Artifact {
                filename: artifact_id.hash.clone(),
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
    let artifact_id = match artifact_id_from_req(&req) {
        Some(id) => id,
        None => return HttpResponse::NotFound().finish(),
    };

    let file_path = artifact_id.object_path();

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

    apply_metadata_headers(&ArtifactMetadata::from_key_values(metadata), &mut builder);

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

fn artifact_id_from_req(req: &HttpRequest) -> Option<ArtifactId> {
    let hash = req.match_info().get("hash")?.to_owned();
    let team = extract_team_from_req(req);

    Some(ArtifactId { team, hash })
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
}
