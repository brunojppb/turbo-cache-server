use s3::{creds::Credentials, request::ResponseDataStream, Bucket, Region};

use crate::app_settings::AppSettings;

pub struct Storage {
    bucket: Bucket,
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
            _ => Credentials::default().unwrap(),
        };

        let mut bucket = Bucket::new(&settings.s3_bucket_name, region, credentials)
            .expect("Could not create a S3 bucket");

        if settings.s3_use_path_style {
            bucket.set_path_style()
        }

        Self { bucket }
    }

    pub async fn get_file(&self, path: &str) -> Option<ResponseDataStream> {
        match self.bucket.get_object_stream(path).await {
            Ok(file_stream) => Some(file_stream),
            Err(_) => None,
        }
    }

    pub async fn put_file(&self, path: &str, data: &[u8]) -> Result<(), String> {
        match self.bucket.put_object(path, data).await {
            Ok(_response) => Ok(()),
            Err(e) => Err(format!("Could not upload file: {}", e)),
        }
    }
}
