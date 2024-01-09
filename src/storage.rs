use s3::{creds::Credentials, Bucket, Region};

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
        let bucket = Bucket::new(&settings.s3_bucket_name, region, credentials)
            .expect("Could not create S3 bucket");

        Self { bucket }
    }

    pub async fn get_file(&self, path: &str) -> Option<String> {
        let file = self.bucket.get_object(path).await;

        match file {
            Ok(_s3_data) => Some("All good!".to_owned()),
            Err(_) => None,
        }
    }

    pub async fn put_file(&self, path: &str, data: &[u8]) -> Result<(), &str> {
        match self.bucket.put_object(path, data).await {
            Ok(_response) => Ok(()),
            Err(_e) => Err("Could not upload file"),
        }
    }
}
