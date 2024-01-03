use s3::{creds::Credentials, Bucket, Region};

pub struct Storage {
    bucket: Bucket,
}

impl Storage {
    pub fn new() -> Self {
        let credentials = Credentials::default().expect("Could not create S3 credentials");
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
}
