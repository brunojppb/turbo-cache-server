use std::collections::HashMap;
use std::fmt;

use bytes::Bytes;
use futures::Stream;
use tokio::io::AsyncRead;

use crate::app_settings::AppSettings;
use crate::storage::upload::{UploadError, Uploader};

mod client;
mod upload;

#[derive(Debug)]
pub enum StorageError {
    /// The bucket answered, but holds no object under that path.
    NotFound,
    /// The body was at or above the part size with no Content-Length.
    LengthRequired,
    /// The bucket could not be reached, or rejected the request.
    Unreachable(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "no such object in the bucket"),
            Self::LengthRequired => {
                write!(f, "uploads at or above the part size need a Content-Length")
            }
            Self::Unreachable(error) => write!(f, "S3 request failed: {error}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotFound | Self::LengthRequired => None,
            Self::Unreachable(error) => Some(error.as_ref()),
        }
    }
}

impl From<UploadError> for StorageError {
    fn from(error: UploadError) -> Self {
        match error {
            UploadError::LengthRequired => Self::LengthRequired,
            UploadError::Failed(error) => Self::Unreachable(error),
        }
    }
}

pub struct Storage {
    s3: aws_sdk_s3::Client,
    uploader: Uploader,
    bucket: String,
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("bucket_name", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl Storage {
    pub fn new(settings: &AppSettings) -> Self {
        let config = client::build_config(settings);
        let s3 = aws_sdk_s3::Client::from_conf(config.clone().build());
        let uploader = Uploader::new(
            s3.clone(),
            config,
            settings.s3_bucket_name.clone(),
            settings.s3_server_side_encryption,
        );

        Self {
            s3,
            uploader,
            bucket: settings.s3_bucket_name.clone(),
        }
    }

    /// Streams the file from the S3 bucket
    #[tracing::instrument(name = "get S3 file")]
    pub async fn get_file(
        &self,
        path: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, std::io::Error>> + use<>, StorageError> {
        match self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(Self::key(path))
            .send()
            .await
        {
            Ok(object) => Ok(tokio_util::io::ReaderStream::new(
                object.body.into_async_read(),
            )),
            Err(error) => {
                // A 404 with no XML body (S3 sends one, but the wire contract
                // doesn't guarantee it) leaves the SDK unable to tell NoSuchKey
                // apart from any other not-found response, so the HTTP status
                // is checked too.
                let not_found = error.as_service_error().is_some_and(|e| e.is_no_such_key())
                    || error
                        .raw_response()
                        .is_some_and(|response| response.status().as_u16() == 404);

                if not_found {
                    Err(StorageError::NotFound)
                } else {
                    Err(StorageError::Unreachable(Box::new(error)))
                }
            }
        }
    }

    /// Returns the user metadata stored on the S3 object, if present.
    #[tracing::instrument(name = "get S3 object metadata")]
    pub async fn get_metadata(&self, path: &str) -> Option<HashMap<String, String>> {
        match self
            .s3
            .head_object()
            .bucket(&self.bucket)
            .key(Self::key(path))
            .send()
            .await
        {
            Ok(head) => head.metadata,
            Err(error) => {
                tracing::warn!(error = %error, path, "HEAD request failed, omitting object metadata");
                None
            }
        }
    }

    /// Streams the given data to the S3 bucket under the given path.
    /// When `metadata` is provided, each key-value pair is persisted as S3 user
    /// metadata (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
    #[tracing::instrument(name = "put S3 file stream", skip(reader))]
    pub async fn put_file_stream<R>(
        &self,
        path: &str,
        reader: R,
        content_length: Option<u64>,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<(), StorageError>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
    {
        self.uploader
            .put(Self::key(path), reader, content_length, metadata)
            .await?;
        Ok(())
    }

    /// Checks whether the given file path exists on the S3 bucket
    #[tracing::instrument(name = "check if S3 file exists")]
    pub async fn file_exists(&self, path: &str) -> Result<bool, StorageError> {
        match self
            .s3
            .head_object()
            .bucket(&self.bucket)
            .key(Self::key(path))
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(error) => {
                if error.as_service_error().is_some_and(|e| e.is_not_found()) {
                    Ok(false)
                } else {
                    Err(StorageError::Unreachable(Box::new(error)))
                }
            }
        }
    }

    /// S3 keys hold no leading slash; callers pass file paths that do, as a
    /// holdover from the previous S3 client's URL convention.
    fn key(path: &str) -> &str {
        path.trim_start_matches('/')
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
            s3_checksum_mode: crate::app_settings::S3ChecksumMode::WhenRequired,
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

    /// Storage ends up in tracing spans, which record it through `Debug`.
    #[test]
    fn debug_output_hides_s3_credentials_after_the_sdk_swap() {
        let storage = Storage::new(&settings_with_credentials());

        let debug_output = format!("{storage:?}");

        assert!(!debug_output.contains("super-secret-access-key"));
        assert!(!debug_output.contains("super-secret-secret-key"));
        assert!(debug_output.contains("turbo"), "bucket name should show");
    }
}
