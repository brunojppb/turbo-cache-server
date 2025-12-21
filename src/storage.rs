use s3::{Bucket, Region, creds::Credentials, request::ResponseDataStream};

use crate::app_settings::{AppSettings, S3ServerSideEncryption};

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
    pub async fn get_file(&self, path: &str) -> Option<ResponseDataStream> {
        let maybe_file = self.bucket.get_object_stream(path).await;
        maybe_file.ok()
    }

    /// Stores the given data in the S3 bucket under the given path
    pub async fn put_file(&self, path: &str, data: &[u8]) -> Result<(), String> {
        let response = match self.server_side_encryption {
            Some(encryption) => {
                self.bucket
                    .put_object_builder(path, data)
                    .with_server_side_encryption(encryption)
                    .expect("Invalid server-side encryption header value")
                    .execute()
                    .await
            }
            None => self.bucket.put_object(path, data).await,
        };

        match response {
            Ok(_response) => Ok(()),
            Err(e) => Err(format!("Could not upload file: {e}")),
        }
    }

    /// Checks whether the given file path exists on the S3 bucket
    pub async fn file_exists(&self, path: &str) -> bool {
        self.bucket.head_object(path).await.is_ok()
    }
}
