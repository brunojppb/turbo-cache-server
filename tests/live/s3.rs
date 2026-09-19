use aws_sdk_s3::Client;
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
};
use aws_sdk_s3::error::DisplayErrorContext;
use aws_sdk_s3::types::{BucketLocationConstraint, CreateBucketConfiguration};
use std::collections::HashMap;

use crate::config::LiveConfig;

/// What the store holds for one object.
pub struct StoredObject {
    pub size: u64,
    pub content_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Builds a client for the same store the cache server writes to.
///
/// Checksums stay off, so a store that rejects the checksum headers fails the
/// server under test and not the test's own bookkeeping.
pub fn client(config: &LiveConfig) -> Client {
    let sdk_config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .endpoint_url(config.endpoint.clone())
        .force_path_style(config.use_path_style)
        .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
        .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
        .credentials_provider(Credentials::new(
            config.access_key.clone(),
            config.secret_key.clone(),
            None,
            None,
            "turbo-cache-server-live-tests",
        ))
        .build();

    Client::from_conf(sdk_config)
}

/// Creates the bucket unless it is already there.
pub async fn ensure_bucket(client: &Client, bucket: &str, region: &str) {
    let mut request = client.create_bucket().bucket(bucket);

    // us-east-1 is the only region S3 rejects as an explicit location constraint.
    if region != "us-east-1" {
        request = request.create_bucket_configuration(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(region))
                .build(),
        );
    }

    let Err(error) = request.send().await else {
        return;
    };

    let already_there = error.as_service_error().is_some_and(|service| {
        service.is_bucket_already_exists() || service.is_bucket_already_owned_by_you()
    }) || error
        .raw_response()
        .is_some_and(|response| response.status().as_u16() == 409);

    if already_there {
        return;
    }

    // The run may use a bucket someone made by hand, without s3:CreateBucket.
    if let Err(head) = client.head_bucket().bucket(bucket).send().await {
        panic!(
            "Failed to create the bucket {bucket}: {}. Reading it failed too: {}",
            DisplayErrorContext(&error),
            DisplayErrorContext(&head)
        );
    }
}

/// Reads the size and user metadata the store holds for one key.
pub async fn stored_object(client: &Client, bucket: &str, key: &str) -> StoredObject {
    let head = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "Failed to read {key} from {bucket}: {}",
                DisplayErrorContext(&error)
            )
        });

    StoredObject {
        size: head.content_length().unwrap_or_default() as u64,
        content_type: head.content_type().map(str::to_owned),
        metadata: head.metadata().cloned().unwrap_or_default(),
    }
}

/// Reports whether the store holds no object under the key.
pub async fn object_is_absent(client: &Client, bucket: &str, key: &str) -> bool {
    match client.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => false,
        Err(error) => {
            let missing = error.as_service_error().is_some_and(|e| e.is_not_found())
                || error
                    .raw_response()
                    .is_some_and(|response| response.status().as_u16() == 404);

            assert!(
                missing,
                "Failed to look up {key} in {bucket}: {}",
                DisplayErrorContext(&error)
            );
            true
        }
    }
}

/// Lists the keys of the multipart uploads still in progress under the prefix.
pub async fn multipart_upload_keys(client: &Client, bucket: &str, prefix: &str) -> Vec<String> {
    let uploads = client
        .list_multipart_uploads()
        .bucket(bucket)
        .prefix(prefix)
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "Failed to list the multipart uploads of {bucket}: {}",
                DisplayErrorContext(&error)
            )
        });

    uploads
        .uploads()
        .iter()
        .filter_map(|upload| upload.key().map(str::to_owned))
        .collect()
}

/// Removes one object. A key that is already gone is not an error.
pub async fn delete_object(client: &Client, bucket: &str, key: &str) {
    let _ = client.delete_object().bucket(bucket).key(key).send().await;
}

/// Removes every object and every in-progress multipart upload under the prefix.
pub async fn clean_prefix(client: &Client, bucket: &str, prefix: &str) {
    if let Ok(listing) = client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(prefix)
        .send()
        .await
    {
        for key in listing.contents().iter().filter_map(|object| object.key()) {
            delete_object(client, bucket, key).await;
        }
    }

    let Ok(uploads) = client
        .list_multipart_uploads()
        .bucket(bucket)
        .prefix(prefix)
        .send()
        .await
    else {
        return;
    };

    for upload in uploads.uploads() {
        let (Some(key), Some(id)) = (upload.key(), upload.upload_id()) else {
            continue;
        };

        let _ = client
            .abort_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(id)
            .send()
            .await;
    }
}
