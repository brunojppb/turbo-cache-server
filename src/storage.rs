use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;

use bytes::Bytes;
use futures::Stream;
use s3::{Bucket, Region, creds::Credentials, error::S3Error};
use secrecy::ExposeSecret;
use tokio::io::AsyncRead;

use crate::app_settings::{AppSettings, S3ServerSideEncryption};
use crate::domain::CacheError;
use crate::usecases::ArtifactStore;

const SSE_HEADER: http::HeaderName = http::HeaderName::from_static("x-amz-server-side-encryption");

impl From<S3Error> for CacheError {
    fn from(error: S3Error) -> Self {
        match error {
            S3Error::HttpFailWithBody(404, _) => Self::NotFound,
            other => Self::StoreUnavailable(Box::new(other)),
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
}

impl ArtifactStore for Storage {
    // The s3 crate already returns its stream boxed; this adds no boxing.
    type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, S3Error>> + Send>>;
    type StreamError = S3Error;

    #[tracing::instrument(name = "get S3 file")]
    async fn get(&self, path: &str) -> Result<Self::ByteStream, CacheError> {
        let response = self
            .bucket
            .get_object_stream(path)
            .await
            .map_err(CacheError::from)?;
        Ok(response.bytes)
    }

    #[tracing::instrument(name = "head S3 file")]
    async fn head(&self, path: &str) -> Result<HashMap<String, String>, CacheError> {
        let (head_result, _status) = self
            .bucket
            .head_object(path)
            .await
            .map_err(CacheError::from)?;
        Ok(head_result.metadata.unwrap_or_default())
    }

    /// Each metadata key-value pair is persisted as S3 user metadata
    /// (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
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
        let mut builder = self.bucket.put_object_stream_builder(path);

        if let Some(encryption) = self.server_side_encryption {
            builder = builder
                .with_header(SSE_HEADER, encryption.as_str())
                .expect("Invalid server-side encryption header value");
        }

        for (key, value) in &metadata {
            builder = builder
                .with_metadata(key, value)
                .expect("Invalid metadata value");
        }

        builder
            .execute_stream(reader)
            .await
            .map_err(CacheError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CacheError;

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

    #[test]
    fn maps_a_404_to_not_found() {
        let error = S3Error::HttpFailWithBody(404, "no such key".to_owned());

        assert!(matches!(CacheError::from(error), CacheError::NotFound));
    }

    #[test]
    fn maps_other_failures_to_store_unavailable() {
        let error = S3Error::HttpFailWithBody(500, "boom".to_owned());

        assert!(matches!(
            CacheError::from(error),
            CacheError::StoreUnavailable(_)
        ));
    }
}
