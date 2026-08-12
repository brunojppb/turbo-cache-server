use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ServerSideEncryption;
use aws_sdk_s3_transfer_manager as transfer_manager;
use tokio::io::{AsyncRead, AsyncReadExt};
use transfer_manager::io::adapters::TokioIo;
use transfer_manager::io::{InputStream, SizeHint};
use transfer_manager::types::{ConcurrencyMode, MemoryBudgetConfig, PartSize};

use crate::app_settings::S3ServerSideEncryption;

/// Bodies below this size go out as a single PutObject. At or above it, the
/// transfer manager runs a multipart upload.
pub(crate) const PART_SIZE: u64 = 8 * 1024 * 1024;

/// Parts held in memory per upload. With PART_SIZE that caps one upload at
/// 32 MiB.
const PARTS_IN_FLIGHT: usize = 4;

/// Ceiling on part data the transfer manager buffers for multipart uploads.
/// The transfer manager otherwise reserves a share of detected RAM, which is
/// wrong for a CI container. The per-request head buffer (up to PART_SIZE) is
/// separate and additional, since it fills before the size decision runs.
const MEMORY_BUDGET: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum UploadError {
    /// A body at or above PART_SIZE arrived without a Content-Length, so the
    /// transfer manager cannot be told the size it needs up front.
    LengthRequired,
    Failed(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthRequired => {
                write!(f, "uploads at or above the part size need a Content-Length")
            }
            Self::Failed(error) => write!(f, "S3 upload failed: {error}"),
        }
    }
}

impl std::error::Error for UploadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LengthRequired => None,
            Self::Failed(error) => Some(error.as_ref()),
        }
    }
}

pub(crate) struct Uploader {
    s3: aws_sdk_s3::Client,
    transfer: transfer_manager::Client,
    bucket: String,
    server_side_encryption: Option<S3ServerSideEncryption>,
}

impl Uploader {
    pub(crate) fn new(
        s3: aws_sdk_s3::Client,
        config: aws_sdk_s3::config::Builder,
        bucket: String,
        server_side_encryption: Option<S3ServerSideEncryption>,
    ) -> Self {
        let transfer = transfer_manager::Client::new(
            transfer_manager::Config::builder()
                .s3_config(transfer_manager::config::S3ClientConfig::new(config))
                .part_size(PartSize::Target(PART_SIZE))
                .concurrency(ConcurrencyMode::Explicit(PARTS_IN_FLIGHT))
                .memory_budget(MemoryBudgetConfig::Limit(MEMORY_BUDGET))
                .build(),
        );

        Self {
            s3,
            transfer,
            bucket,
            server_side_encryption,
        }
    }

    fn encryption(&self) -> Option<ServerSideEncryption> {
        self.server_side_encryption
            .map(|encryption| ServerSideEncryption::from(encryption.as_str()))
    }

    /// Streams the body to the bucket, keeping `metadata` as S3 user metadata.
    pub(crate) async fn put<R>(
        &self,
        path: &str,
        reader: R,
        content_length: Option<u64>,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<(), UploadError>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
    {
        let mut reader = reader;
        let mut head = Vec::new();
        (&mut reader)
            .take(PART_SIZE)
            .read_to_end(&mut head)
            .await
            .map_err(|error| UploadError::Failed(Box::new(error)))?;

        if (head.len() as u64) < PART_SIZE {
            self.s3
                .put_object()
                .bucket(&self.bucket)
                .key(path)
                .set_metadata(metadata.cloned())
                .set_server_side_encryption(self.encryption())
                .body(ByteStream::from(head))
                .send()
                .await
                .map_err(|error| UploadError::Failed(Box::new(error)))?;

            return Ok(());
        }

        let length = content_length.ok_or(UploadError::LengthRequired)?;
        // Replays the bytes already read, then continues with the rest.
        let body = Cursor::new(head).chain(reader);
        let stream = InputStream::from_part_stream(TokioIo::new(body, SizeHint::exact(length)));

        self.transfer
            .upload()
            .bucket(&self.bucket)
            .key(path)
            .set_metadata(metadata.cloned())
            .set_server_side_encryption(self.encryption())
            .body(stream)
            .initiate()
            .map_err(|error| UploadError::Failed(Box::new(error)))?
            .join()
            .await
            .map_err(|error| UploadError::Failed(Box::new(error)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::{AppSettings, S3ChecksumMode};
    use wiremock::matchers::{any, method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    const TAG: &str = "v=1:sha256:abc123";

    fn uploader(endpoint: &str) -> Uploader {
        let settings = AppSettings {
            host: "127.0.0.1".to_owned(),
            port: 8000,
            s3_access_key: Some("access".into()),
            s3_secret_key: Some("secret".into()),
            s3_endpoint: Some(endpoint.to_owned()),
            s3_use_path_style: true,
            s3_region: "eu-central-1".to_owned(),
            s3_bucket_name: "turbo".to_owned(),
            s3_server_side_encryption: None,
            s3_checksum_mode: S3ChecksumMode::WhenRequired,
            turbo_token: None,
        };

        let config = super::super::client::build_config(&settings);
        let s3 = aws_sdk_s3::Client::from_conf(config.clone().build());

        Uploader::new(s3, config, "turbo".to_owned(), None)
    }

    fn tag_metadata() -> HashMap<String, String> {
        HashMap::from([("x-artifact-tag".to_owned(), TAG.to_owned())])
    }

    async fn upload(
        server: &MockServer,
        size: usize,
        send_length: bool,
    ) -> Result<(), UploadError> {
        let reader = std::io::Cursor::new(vec![7u8; size]);
        let length = if send_length { Some(size as u64) } else { None };

        uploader(&server.uri())
            .put("team/hash", reader, length, Some(&tag_metadata()))
            .await
    }

    fn header(request: &Request, name: &str) -> Option<String> {
        request
            .headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned())
    }

    async fn mount_single_part(server: &MockServer) {
        Mock::given(method("PUT"))
            .and(path("/turbo/team/hash"))
            .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-single\""))
            .mount(server)
            .await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .mount(server)
            .await;
    }

    async fn mount_multipart(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/turbo/team/hash"))
            .and(query_param("uploads", ""))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult><Bucket>turbo</Bucket><Key>team/hash</Key><UploadId>upload-1</UploadId></InitiateMultipartUploadResult>"#,
                "application/xml",
            ))
            .mount(server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/turbo/team/hash"))
            .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-part\""))
            .mount(server)
            .await;

        Mock::given(method("POST"))
            .and(path("/turbo/team/hash"))
            .and(query_param("uploadId", "upload-1"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult><Location>http://localhost/turbo/team/hash</Location><Bucket>turbo</Bucket><Key>team/hash</Key><ETag>"etag-complete"</ETag></CompleteMultipartUploadResult>"#,
                "application/xml",
            ))
            .mount(server)
            .await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .mount(server)
            .await;
    }

    /// The shape the existing e2e suite asserts: one PUT carrying the raw body.
    #[tokio::test]
    async fn tiny_upload_stays_a_single_put_object() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;

        upload(&server, 12, true).await.expect("upload failed");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method.as_str(), "PUT");
        assert_eq!(
            header(&requests[0], "x-amz-meta-x-artifact-tag").as_deref(),
            Some(TAG)
        );
        assert_eq!(requests[0].body, vec![7u8; 12], "body must be sent raw");
    }

    /// 8,388,607 bytes: the size that already worked.
    #[tokio::test]
    async fn one_byte_below_the_part_size_stays_a_single_put_object() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;

        upload(&server, (PART_SIZE - 1) as usize, true)
            .await
            .expect("upload failed");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body.len(), (PART_SIZE - 1) as usize);
    }

    /// 8,388,608 bytes: the size that lost the tag in issue #620.
    #[tokio::test]
    async fn exactly_the_part_size_carries_the_tag_into_multipart() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;

        upload(&server, PART_SIZE as usize, true)
            .await
            .expect("upload failed");

        let requests = server.received_requests().await.unwrap();
        let initiate = requests
            .iter()
            .find(|r| r.method.as_str() == "POST" && r.url.query() == Some("uploads"))
            .expect("no CreateMultipartUpload request");

        assert_eq!(
            header(initiate, "x-amz-meta-x-artifact-tag").as_deref(),
            Some(TAG),
            "issue #620: the tag must be set when the multipart upload starts"
        );

        let uploaded: usize = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT")
            .map(|r| r.body.len())
            .sum();
        assert_eq!(uploaded, PART_SIZE as usize, "all bytes must reach S3");
    }

    /// 8,715,039 bytes: the third size in the issue report.
    #[tokio::test]
    async fn above_the_part_size_carries_the_tag_and_completes() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;

        upload(&server, 8_715_039, true)
            .await
            .expect("upload failed");

        let requests = server.received_requests().await.unwrap();
        let initiate = requests
            .iter()
            .find(|r| r.method.as_str() == "POST" && r.url.query() == Some("uploads"))
            .expect("no CreateMultipartUpload request");
        assert_eq!(
            header(initiate, "x-amz-meta-x-artifact-tag").as_deref(),
            Some(TAG)
        );

        let uploaded: usize = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT")
            .map(|r| r.body.len())
            .sum();
        assert_eq!(uploaded, 8_715_039);

        assert!(
            requests
                .iter()
                .any(|r| r.method.as_str() == "POST" && r.url.query() == Some("uploadId=upload-1")),
            "expected CompleteMultipartUpload"
        );
    }

    /// A small body needs no Content-Length, because it never goes multipart.
    #[tokio::test]
    async fn small_upload_without_content_length_succeeds() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;

        upload(&server, 12, false).await.expect("upload failed");

        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn large_upload_without_content_length_is_length_required() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;

        let result = upload(&server, PART_SIZE as usize, false).await;

        assert!(matches!(result, Err(UploadError::LengthRequired)));
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "must fail before touching S3"
        );
    }
}
