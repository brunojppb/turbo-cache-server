use s3::{creds::Credentials, request::ResponseData, Bucket, Region};

use crate::app_settings::AppSettings;

pub struct Storage {
    bucket: Bucket,
}

impl Storage {
    pub fn new(settings: &AppSettings) -> Self {
        let credentials = Credentials::new(
            Some(&settings.s3_access_key),
            Some(&settings.s3_secret_key),
            None,
            None,
            None,
        )
        .expect("Could not create S3 credentials");
        let region = Region::Custom {
            region: settings.s3_region.clone(),
            endpoint: settings.s3_endpoint.clone(),
        };
        let mut bucket = Bucket::new(&settings.s3_bucket_name, region, credentials)
            .expect("Could not create S3 bucket");

        bucket.set_path_style();

        Self { bucket }
    }

    pub async fn get_file(&self, path: &str) -> Option<ResponseData> {
        let file = self.bucket.get_object(path).await;

        match file {
            Ok(s3_data) => Some(s3_data),
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
