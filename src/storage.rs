use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;

use aws_sdk_s3::error::DisplayErrorContext;
use bytes::Bytes;
use futures::{Stream, stream};
use tokio::io::AsyncRead;

use crate::app_settings::AppSettings;
use crate::domain::CacheError;
use crate::storage::upload::{UploadError, Uploader};
use crate::usecases::ArtifactStore;

mod client;
mod upload;

impl From<UploadError> for CacheError {
    fn from(error: UploadError) -> Self {
        Self::StoreUnavailable(Box::new(error))
    }
}

/// A failed S3 request, keeping the S3 code, message, and HTTP status.
#[derive(Debug)]
struct StoreError(Box<dyn std::error::Error + Send + Sync>);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `SdkError` prints "service error" and hides the S3 code, message, and
        // HTTP status in its source chain.
        write!(f, "{}", DisplayErrorContext(self.0.as_ref()))
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// Reports a failed S3 request.
fn store_unavailable(error: impl std::error::Error + Send + Sync + 'static) -> CacheError {
    CacheError::StoreUnavailable(Box::new(StoreError(Box::new(error))))
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
        let s3 = aws_sdk_s3::Client::from_conf(config.build());
        let uploader = Uploader::new(
            s3.clone(),
            settings.s3_bucket_name.clone(),
            settings.s3_server_side_encryption,
        );

        Self {
            s3,
            uploader,
            bucket: settings.s3_bucket_name.clone(),
        }
    }

    /// The object key for an artifact path.
    fn key(path: &str) -> &str {
        path.strip_prefix('/').unwrap_or(path)
    }
}

impl ArtifactStore for Storage {
    type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;
    type StreamError = std::io::Error;

    /// Streams the file from the S3 bucket
    #[tracing::instrument(name = "get S3 file")]
    async fn get(&self, path: &str) -> Result<Self::ByteStream, CacheError> {
        match self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(Self::key(path))
            .send()
            .await
        {
            // Yield the chunks the SDK already produced, so no byte is copied
            // into a second buffer on the way out. A failed body never polls
            // again, so the error ends the stream.
            Ok(object) => Ok(Box::pin(stream::unfold(
                Some(object.body),
                |state| async move {
                    let mut body = state?;
                    match body.next().await? {
                        Ok(chunk) => Some((Ok(chunk), Some(body))),
                        Err(error) => {
                            tracing::warn!(error = %error, "S3 download body failed");
                            Some((Err(std::io::Error::other(error)), None))
                        }
                    }
                },
            ))),
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
                    Err(CacheError::NotFound)
                } else {
                    Err(store_unavailable(error))
                }
            }
        }
    }

    #[tracing::instrument(name = "head S3 file")]
    async fn head(&self, path: &str) -> Result<HashMap<String, String>, CacheError> {
        match self
            .s3
            .head_object()
            .bucket(&self.bucket)
            .key(Self::key(path))
            .send()
            .await
        {
            Ok(head) => Ok(head.metadata.unwrap_or_default()),
            Err(error) => {
                let not_found = error.as_service_error().is_some_and(|e| e.is_not_found())
                    || error
                        .raw_response()
                        .is_some_and(|response| response.status().as_u16() == 404);
                if not_found {
                    Err(CacheError::NotFound)
                } else {
                    Err(store_unavailable(error))
                }
            }
        }
    }

    #[tracing::instrument(name = "put S3 file stream", skip(reader, metadata))]
    async fn put<R>(
        &self,
        path: &str,
        reader: &mut R,
        metadata: HashMap<String, String>,
    ) -> Result<(), CacheError>
    where
        R: AsyncRead + Unpin,
    {
        self.uploader
            .put(Self::key(path), reader, Some(&metadata))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn key_strips_one_leading_slash() {
        assert_eq!(Storage::key("/team/hash"), "team/hash");
        assert_eq!(Storage::key("//team/hash"), "/team/hash");
        assert_eq!(Storage::key("team/hash"), "team/hash");
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
        assert!(debug_output.contains("turbo"), "bucket name should show");
    }

    /// An `SdkError` keeps the S3 code out of its own `Display`, and a refusal
    /// must not read as a cache miss.
    #[tokio::test]
    async fn get_errors_report_the_s3_error_code() {
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(403).set_body_raw(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>"#,
                "application/xml",
            ))
            .mount(&server)
            .await;
        let mut settings = settings_with_credentials();
        settings.s3_endpoint = Some(server.uri());

        let error = Storage::new(&settings)
            .get("/team/hash")
            .await
            .err()
            .expect("the GET must fail");

        let message = error.to_string();
        assert!(message.contains("AccessDenied"), "message was: {message}");
        assert!(
            matches!(error, CacheError::StoreUnavailable(_)),
            "a refused GET is not a cache miss: {error:?}"
        );
    }
}
