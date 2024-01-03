use s3::{creds::Credentials, Bucket, Region};

pub struct Storage {
    bucket: Bucket,
}

impl Storage {
    pub fn new() -> Self {
        let credentials = Credentials::new(
            Some("ShwLIHVR2zCgA8qoiftf"),
            Some("GYe6lj85PPofETRMUMghy2DCQhrW1bjSvi6Ep24k"),
            None,
            None,
            None,
        )
        .expect("Could not create S3 credentials");
        let region = Region::Custom {
            region: "eu-central-1".to_owned(),
            endpoint: "http://localhost:9000".to_owned(),
        };
        let bucket = Bucket::new("turbo", region, credentials).expect("Could not create S3 bucket");

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
