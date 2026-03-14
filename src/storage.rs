use std::collections::HashMap;

use s3::{Bucket, Region, creds::Credentials, request::ResponseDataStream};

use crate::app_settings::{AppSettings, S3ServerSideEncryption};

#[derive(Debug)]
pub struct Storage {
    bucket: Box<Bucket>,
    server_side_encryption: Option<S3ServerSideEncryption>,
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
            (Some(access_key), Some(secret_key)) => {
                Credentials::new(Some(access_key), Some(secret_key), None, None, None).unwrap()
            }
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
    pub async fn get_file(&self, path: &str) -> Option<ResponseDataStream> {
        let maybe_file = self.bucket.get_object_stream(path).await;
        maybe_file.ok()
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

    /// Stores the given data in the S3 bucket under the given path.
    /// When `metadata` is provided, each key-value pair is persisted as S3 user
    /// metadata (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
    #[tracing::instrument(name = "put S3 file")]
    pub async fn put_file(
        &self,
        path: &str,
        data: &[u8],
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<(), String> {
        let mut builder = self.bucket.put_object_builder(path, data);

        if let Some(encryption) = self.server_side_encryption {
            builder = builder
                .with_server_side_encryption(encryption)
                .expect("Invalid server-side encryption header value");
        }

        if let Some(metadata) = metadata {
            for (key, value) in metadata {
                builder = builder
                    .with_metadata(key, value)
                    .expect("Invalid metadata value");
            }
        }

        match builder.execute().await {
            Ok(_response) => Ok(()),
            Err(e) => Err(format!("Could not upload file: {e}")),
        }
    }

    /// Checks whether the given file path exists on the S3 bucket
    #[tracing::instrument(name = "check if S3 file exists")]
    pub async fn file_exists(&self, path: &str) -> bool {
        self.bucket.head_object(path).await.is_ok()
    }
}
