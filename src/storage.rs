use std::collections::HashMap;
use std::fmt;

use s3::{Bucket, Region, creds::Credentials, error::S3Error, request::ResponseDataStream};
use secrecy::ExposeSecret;
use tokio::io::AsyncRead;

use crate::app_settings::{AppSettings, S3ServerSideEncryption};

const SSE_HEADER: http::HeaderName = http::HeaderName::from_static("x-amz-server-side-encryption");

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
    #[tracing::instrument(name = "get S3 file")]
    pub async fn get_file(&self, path: &str) -> Result<ResponseDataStream, StorageError> {
        let file = self.bucket.get_object_stream(path).await?;
        Ok(file)
    }

    /// Returns the user metadata stored on the S3 object, if present.
    #[tracing::instrument(name = "get S3 object metadata")]
    pub async fn get_metadata(&self, path: &str) -> Option<HashMap<String, String>> {
        let (head_result, _status) = match self.bucket.head_object(path).await {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(error = %error, path, "HEAD request failed, omitting object metadata");
                return None;
            }
        };
        head_result.metadata
    }

    /// Streams the given data to the S3 bucket under the given path.
    /// When `metadata` is provided, each key-value pair is persisted as S3 user
    /// metadata (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
    #[tracing::instrument(name = "put S3 file stream", skip(reader))]
    pub async fn put_file_stream<R>(
        &self,
        path: &str,
        reader: &mut R,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<(), StorageError>
    where
        R: AsyncRead + Unpin,
    {
        let mut builder = self.bucket.put_object_stream_builder(path);

        if let Some(encryption) = self.server_side_encryption {
            builder = builder
                .with_header(SSE_HEADER, encryption.as_str())
                .expect("Invalid server-side encryption header value");
        }

        if let Some(metadata) = metadata {
            for (key, value) in metadata {
                builder = builder
                    .with_metadata(key, value)
                    .expect("Invalid metadata value");
            }
        }

        builder.execute_stream(reader).await?;
        Ok(())
    }

    /// Checks whether the given file path exists on the S3 bucket
    #[tracing::instrument(name = "check if S3 file exists")]
    pub async fn file_exists(&self, path: &str) -> Result<bool, StorageError> {
        match self.bucket.head_object(path).await {
            Ok(_) => Ok(true),
            Err(error) => match StorageError::from(error) {
                StorageError::NotFound => Ok(false),
                error => Err(error),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
