use std::collections::HashMap;
use std::fmt;

use s3::{Bucket, Region, creds::Credentials, error::S3Error, request::ResponseDataStream};
use secrecy::ExposeSecret;
use tokio::io::AsyncRead;

use crate::app_settings::{AppSettings, S3ServerSideEncryption};
use crate::domain::{ArtifactId, ArtifactMetadata};

const SSE_HEADER: http::HeaderName = http::HeaderName::from_static("x-amz-server-side-encryption");
const ARTIFACT_TAG: &str = "x-artifact-tag";
const ARTIFACT_DURATION: &str = "x-artifact-duration";

pub type ArtifactStream = ResponseDataStream;

fn object_path(id: &ArtifactId) -> String {
    format!("/{}/{}", id.team, id.hash)
}

pub(crate) fn metadata_from_storage(mut values: HashMap<String, String>) -> ArtifactMetadata {
    ArtifactMetadata {
        tag: values.remove(ARTIFACT_TAG),
        duration: values
            .remove(ARTIFACT_DURATION)
            .as_deref()
            .and_then(ArtifactMetadata::parse_duration),
    }
}

pub(crate) fn metadata_to_storage(metadata: &ArtifactMetadata) -> HashMap<String, String> {
    let mut values = HashMap::new();
    if let Some(tag) = &metadata.tag {
        values.insert(ARTIFACT_TAG.to_owned(), tag.clone());
    }
    if let Some(duration) = metadata.duration {
        values.insert(ARTIFACT_DURATION.to_owned(), duration.to_string());
    }
    values
}

#[derive(Debug)]
pub enum StorageError {
    /// The bucket answered, but holds no object under that path.
    NotFound,
    /// The bucket could not be reached, or rejected the request.
    Unreachable(S3Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "no such object in the bucket"),
            Self::Unreachable(error) => write!(f, "S3 request failed: {error}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotFound => None,
            Self::Unreachable(error) => Some(error),
        }
    }
}

impl From<S3Error> for StorageError {
    fn from(error: S3Error) -> Self {
        match error {
            S3Error::HttpFailWithBody(404, _) => Self::NotFound,
            other => Self::Unreachable(other),
        }
    }
}

pub struct Storage {
    bucket: Box<Bucket>,
    server_side_encryption: Option<S3ServerSideEncryption>,
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("bucket_name", &self.bucket.name)
            .field("region", &self.bucket.region)
            .field("server_side_encryption", &self.server_side_encryption)
            .finish_non_exhaustive()
    }
}

impl Storage {
    pub fn new(settings: &AppSettings) -> Self {
        let region = match &settings.s3_endpoint {
            Some(endpoint) => Region::Custom {
                endpoint: endpoint.clone(),
                region: settings.s3_region.clone(),
            },
            None => settings
                .s3_region
                .parse()
                .expect("AWS region should be present"),
        };

        let credentials = match (&settings.s3_access_key, &settings.s3_secret_key) {
            (Some(access_key), Some(secret_key)) => Credentials::new(
                Some(access_key.expose_secret()),
                Some(secret_key.expose_secret()),
                None,
                None,
                None,
            )
            .unwrap(),
            // If your Credentials are handled via IAM policies and allow
            // your network to access S3 directly without any credentials setup
            // Then no need to setup credentials at all. Defaults should be fine
            _ => Credentials::default().expect("Could not use default AWS credentials"),
        };

        let mut bucket = Bucket::new(&settings.s3_bucket_name, region, credentials)
            .expect("Could not create a S3 bucket");

        if settings.s3_use_path_style {
            bucket.set_path_style()
        }

        Self {
            bucket,
            server_side_encryption: settings.s3_server_side_encryption,
        }
    }

    /// Streams the file from the S3 bucket
    #[tracing::instrument(name = "get S3 file", skip(id))]
    pub async fn get_file(&self, id: &ArtifactId) -> Result<ArtifactStream, StorageError> {
        let file = self.bucket.get_object_stream(object_path(id)).await?;
        Ok(file)
    }

    /// Returns the user metadata stored on the S3 object, or reports that the
    /// object is missing.
    #[tracing::instrument(name = "head S3 file", skip(id))]
    pub async fn head_file(&self, id: &ArtifactId) -> Result<ArtifactMetadata, StorageError> {
        let (head_result, _status) = self.bucket.head_object(object_path(id)).await?;
        Ok(metadata_from_storage(
            head_result.metadata.unwrap_or_default(),
        ))
    }

    /// Streams the given data to the S3 bucket under the given path.
    /// Each metadata key-value pair is persisted as S3 user metadata
    /// (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
    #[tracing::instrument(name = "put S3 file stream", skip(id, reader, metadata))]
    pub async fn put_file_stream<R>(
        &self,
        id: &ArtifactId,
        reader: &mut R,
        metadata: &ArtifactMetadata,
    ) -> Result<(), StorageError>
    where
        R: AsyncRead + Unpin,
    {
        let path = object_path(id);
        let mut builder = self.bucket.put_object_stream_builder(path);

        if let Some(encryption) = self.server_side_encryption {
            builder = builder
                .with_header(SSE_HEADER, encryption.as_str())
                .expect("Invalid server-side encryption header value");
        }

        for (key, value) in metadata_to_storage(metadata) {
            builder = builder
                .with_metadata(&key, &value)
                .expect("Invalid metadata value");
        }

        builder.execute_stream(reader).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_path_keeps_team_and_hash_in_the_existing_layout() {
        let id = ArtifactId {
            team: "my-team".to_owned(),
            hash: "abc123".to_owned(),
        };
        assert_eq!(object_path(&id), "/my-team/abc123");
    }

    /// The bucket holds the parsed number, not the bytes the client sent.
    #[test]
    fn writes_the_parsed_duration_to_storage() {
        let metadata = ArtifactMetadata {
            duration: ArtifactMetadata::parse_duration("007"),
            ..ArtifactMetadata::default()
        };

        let stored = metadata_to_storage(&metadata);

        assert_eq!(stored.get(ARTIFACT_DURATION).map(String::as_str), Some("7"));
    }

    #[test]
    fn reads_both_artifact_headers_from_storage() {
        let tag = "v=1:sha256:abc123";
        let stored = HashMap::from([
            (ARTIFACT_TAG.to_owned(), tag.to_owned()),
            (ARTIFACT_DURATION.to_owned(), "1234".to_owned()),
        ]);

        assert_eq!(
            metadata_from_storage(stored),
            ArtifactMetadata {
                tag: Some(tag.to_owned()),
                duration: Some(1234),
            }
        );
    }

    /// Another writer may share the bucket, so the read path does not trust the
    /// stored value either.
    #[test]
    fn drops_a_malformed_duration_from_storage() {
        let stored = HashMap::from([(ARTIFACT_DURATION.to_owned(), "not-a-number".to_owned())]);

        assert_eq!(metadata_from_storage(stored).duration, None);
    }

    fn settings_with_credentials() -> AppSettings {
        AppSettings {
            host: "127.0.0.1".to_owned(),
            port: 8000,
            s3_access_key: Some("super-secret-access-key".into()),
            s3_secret_key: Some("super-secret-secret-key".into()),
            s3_endpoint: Some("http://localhost:9000".to_owned()),
            s3_use_path_style: true,
            s3_region: "eu-central-1".to_owned(),
            s3_bucket_name: "turbo".to_owned(),
            s3_server_side_encryption: None,
            turbo_token: None,
        }
    }

    /// Storage ends up in tracing spans, which record it through `Debug`.
    #[test]
    fn debug_output_hides_s3_credentials() {
        let storage = Storage::new(&settings_with_credentials());

        let debug_output = format!("{storage:?}");

        assert!(!debug_output.contains("super-secret-access-key"));
        assert!(!debug_output.contains("super-secret-secret-key"));
    }
}
