use std::collections::HashMap;
use std::fmt;
use std::io;
use std::time::Duration;

use aws_sdk_s3::config::RequestChecksumCalculation;
use aws_sdk_s3::error::DisplayErrorContext;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{
    ChecksumAlgorithm, CompletedMultipartUpload, CompletedPart, ServerSideEncryption,
};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::app_settings::S3ServerSideEncryption;

/// Bodies below this size use PutObject; larger bodies use sequential parts.
/// At most one part sits in memory, and SDK retries replay that part.
pub(crate) const PART_SIZE: u64 = 8 * 1024 * 1024;
const MAX_PARTS: i32 = 10_000;
const ABORT_TIMEOUT: Duration = Duration::from_secs(10);
const CONTENT_TYPE: &str = "application/octet-stream";

#[derive(Debug)]
pub(crate) enum UploadError {
    Failed(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // `SdkError` prints "service error" and hides the S3 code, message,
            // and HTTP status in its source chain.
            Self::Failed(error) => {
                write!(
                    f,
                    "S3 upload failed: {}",
                    DisplayErrorContext(error.as_ref())
                )
            }
        }
    }
}

impl std::error::Error for UploadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error.as_ref()),
        }
    }
}

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> UploadError {
    UploadError::Failed(Box::new(error))
}

/// Owns the server-side upload until completion. Cancellation drops this guard
/// and schedules bounded cleanup, without keeping the request body alive.
struct MultipartUpload {
    s3: aws_sdk_s3::Client,
    bucket: String,
    key: String,
    id: Option<String>,
}

impl Drop for MultipartUpload {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else { return };
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                let s3 = self.s3.clone();
                let bucket = self.bucket.clone();
                let key = self.key.clone();
                runtime.spawn(async move { abort(&s3, &bucket, &key, &id).await });
            }
            // Outside a runtime nothing can send the abort, so the parts stay in
            // the bucket until its lifecycle rule removes them.
            Err(_) => tracing::warn!(
                bucket = %self.bucket,
                key = %self.key,
                upload_id = %id,
                "Leaked an unfinished multipart upload"
            ),
        }
    }
}

async fn abort(s3: &aws_sdk_s3::Client, bucket: &str, key: &str, id: &str) {
    let request = s3
        .abort_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(id)
        .send();
    match tokio::time::timeout(ABORT_TIMEOUT, request).await {
        Ok(Ok(_)) => {}
        // `NoSuchUpload` is the completion whose response never arrived: the
        // upload is already finished, so there is nothing left to clean up.
        Ok(Err(error))
            if error
                .as_service_error()
                .is_some_and(|e| e.is_no_such_upload()) =>
        {
            tracing::debug!(
                bucket,
                key,
                upload_id = id,
                "Multipart upload already gone; nothing to abort"
            )
        }
        Ok(Err(error)) => tracing::warn!(
            bucket,
            key,
            upload_id = id,
            error = %DisplayErrorContext(&error),
            "Could not abort multipart upload"
        ),
        Err(_) => tracing::warn!(
            bucket,
            key,
            upload_id = id,
            "Timed out aborting multipart upload"
        ),
    }
}

pub(crate) struct Uploader {
    s3: aws_sdk_s3::Client,
    bucket: String,
    server_side_encryption: Option<S3ServerSideEncryption>,
}

impl Uploader {
    pub(crate) fn new(
        s3: aws_sdk_s3::Client,
        bucket: String,
        server_side_encryption: Option<S3ServerSideEncryption>,
    ) -> Self {
        Self {
            s3,
            bucket,
            server_side_encryption,
        }
    }

    fn encryption(&self) -> Option<ServerSideEncryption> {
        self.server_side_encryption
            .map(|encryption| ServerSideEncryption::from(encryption.as_str()))
    }

    /// Reads one part at a time, so unknown-length bodies need no producer task
    /// or whole-object buffer. Metadata belongs on multipart initiation.
    pub(crate) async fn put<R>(
        &self,
        path: &str,
        reader: &mut R,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<(), UploadError>
    where
        R: AsyncRead + Unpin,
    {
        let head = read_part(reader).await?;
        if (head.len() as u64) < PART_SIZE {
            self.s3
                .put_object()
                .bucket(&self.bucket)
                .key(path)
                .set_metadata(metadata.cloned())
                .set_server_side_encryption(self.encryption())
                .content_type(CONTENT_TYPE)
                .body(ByteStream::from(head))
                .send()
                .await
                .map_err(failed)?;
            return Ok(());
        }

        let s3 = self.s3.clone();
        let bucket = self.bucket.clone();
        let key = path.to_owned();
        let metadata = metadata.cloned();
        let encryption = self.encryption();
        // Match the SDK's automatic UploadPart checksum when opted in. S3's
        // default initiation algorithm can differ from the SDK's CRC32.
        let checksum_algorithm = (self.s3.config().request_checksum_calculation()
            == Some(&RequestChecksumCalculation::WhenSupported))
        .then_some(ChecksumAlgorithm::Crc32);
        // Let initiation finish even if the request disappears. The returned
        // guard then drops and aborts the upload when its join handle is gone.
        // SDK operation timeouts bound this task; it never owns the body.
        let mut upload = tokio::spawn(async move {
            let response = s3
                .create_multipart_upload()
                .bucket(&bucket)
                .key(&key)
                .set_metadata(metadata)
                .set_server_side_encryption(encryption)
                .content_type(CONTENT_TYPE)
                .set_checksum_algorithm(checksum_algorithm)
                .send()
                .await
                .map_err(failed)?;
            let id = response
                .upload_id()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| failed(io::Error::other("S3 omitted the multipart upload ID")))?
                .to_owned();
            Ok::<_, UploadError>(MultipartUpload {
                s3,
                bucket,
                key,
                id: Some(id),
            })
        })
        .await
        .map_err(failed)??;

        let result = self.put_parts(&upload, reader, head).await;
        // Clearing the ID stops the guard from aborting an upload that finished.
        if result.is_ok() {
            upload.id = None;
        }
        result
    }

    async fn put_parts<R: AsyncRead + Unpin>(
        &self,
        upload: &MultipartUpload,
        reader: &mut R,
        mut part: Vec<u8>,
    ) -> Result<(), UploadError> {
        let mut completed = Vec::new();
        let mut number = 1;
        while !part.is_empty() {
            if number > MAX_PARTS {
                return Err(failed(io::Error::other(
                    "S3 multipart upload exceeds 10000 parts",
                )));
            }
            let response = self
                .s3
                .upload_part()
                .bucket(&upload.bucket)
                .key(&upload.key)
                .set_upload_id(upload.id.clone())
                .part_number(number)
                .body(ByteStream::from(part))
                .send()
                .await
                .map_err(failed)?;
            let etag = response
                .e_tag()
                .filter(|etag| !etag.is_empty())
                .ok_or_else(|| failed(io::Error::other("S3 omitted the part ETag")))?;
            completed.push(
                CompletedPart::builder()
                    .part_number(number)
                    .e_tag(etag)
                    .set_checksum_crc32(response.checksum_crc32().map(str::to_owned))
                    .set_checksum_crc32_c(response.checksum_crc32_c().map(str::to_owned))
                    .set_checksum_crc64_nvme(response.checksum_crc64_nvme().map(str::to_owned))
                    .set_checksum_sha1(response.checksum_sha1().map(str::to_owned))
                    .set_checksum_sha256(response.checksum_sha256().map(str::to_owned))
                    .build(),
            );
            number += 1;
            part = read_part(reader).await?;
        }
        self.s3
            .complete_multipart_upload()
            .bucket(&upload.bucket)
            .key(&upload.key)
            .set_upload_id(upload.id.clone())
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(completed))
                    .build(),
            )
            .send()
            .await
            .map_err(failed)?;
        Ok(())
    }
}

async fn read_part<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, UploadError> {
    let mut part = Vec::with_capacity(PART_SIZE as usize);
    reader
        .take(PART_SIZE)
        .read_to_end(&mut part)
        .await
        .map_err(failed)?;
    Ok(part)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::{AppSettings, S3ChecksumMode};
    use wiremock::matchers::{any, method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    const TAG: &str = "v=1:sha256:abc123";

    fn uploader(endpoint: &str) -> Uploader {
        uploader_with_checksums(endpoint, S3ChecksumMode::WhenRequired)
    }

    fn uploader_with_checksums(endpoint: &str, checksum_mode: S3ChecksumMode) -> Uploader {
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
            s3_checksum_mode: checksum_mode,
            turbo_token: None,
        };

        let config = super::super::client::build_config(&settings);
        let s3 = aws_sdk_s3::Client::from_conf(config.clone().build());

        Uploader::new(s3, "turbo".to_owned(), None)
    }

    fn tag_metadata() -> HashMap<String, String> {
        HashMap::from([("x-artifact-tag".to_owned(), TAG.to_owned())])
    }

    async fn upload(server: &MockServer, size: usize) -> Result<(), UploadError> {
        let mut reader = std::io::Cursor::new(vec![7u8; size]);

        uploader(&server.uri())
            .put("team/hash", &mut reader, Some(&tag_metadata()))
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

        Mock::given(method("DELETE"))
            .and(query_param("uploadId", "upload-1"))
            .respond_with(ResponseTemplate::new(204))
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

        upload(&server, 12).await.expect("upload failed");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method.as_str(), "PUT");
        assert_eq!(
            header(&requests[0], "x-amz-meta-x-artifact-tag").as_deref(),
            Some(TAG)
        );
        assert_eq!(
            header(&requests[0], "content-type").as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(requests[0].body, vec![7u8; 12], "body must be sent raw");
    }

    /// `SdkError` prints "service error" on its own, so the S3 code must come
    /// from the source chain.
    #[tokio::test]
    async fn upload_errors_report_the_s3_error_code() {
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(403).set_body_raw(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>"#,
                "application/xml",
            ))
            .mount(&server)
            .await;

        let error = upload(&server, 12).await.expect_err("upload must fail");

        let message = error.to_string();
        assert!(message.contains("AccessDenied"), "message was: {message}");
    }

    /// 8,388,607 bytes: the size that already worked.
    #[tokio::test]
    async fn one_byte_below_the_part_size_stays_a_single_put_object() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;

        upload(&server, (PART_SIZE - 1) as usize)
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

        upload(&server, PART_SIZE as usize)
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
        assert_eq!(
            header(initiate, "content-type").as_deref(),
            Some("application/octet-stream")
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

        upload(&server, 8_715_039).await.expect("upload failed");

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

        upload(&server, 12).await.expect("upload failed");

        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn large_upload_without_content_length_succeeds() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;

        let result = upload(&server, PART_SIZE as usize).await;

        assert!(result.is_ok());
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn empty_upload_is_a_single_put() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;
        upload(&server, 0).await.unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].body.is_empty());
    }

    #[tokio::test]
    async fn parts_preserve_bytes_order_metadata_and_encryption() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("PUT"))
            .respond_with(|request: &Request| {
                let number = request
                    .url
                    .query_pairs()
                    .find(|(key, _)| key == "partNumber")
                    .unwrap()
                    .1;
                ResponseTemplate::new(200).insert_header("ETag", format!("\"part-{number}\""))
            })
            .with_priority(1)
            .mount(&server)
            .await;
        let data: Vec<_> = (0..PART_SIZE as usize + 257)
            .map(|n| (n % 251) as u8)
            .collect();
        let mut reader = io::Cursor::new(&data);
        let mut uploader = uploader(&server.uri());
        uploader.server_side_encryption = Some(S3ServerSideEncryption::Aes256);
        let mut metadata = tag_metadata();
        metadata.insert("x-artifact-duration".to_owned(), "321".to_owned());
        uploader
            .put("team/hash", &mut reader, Some(&metadata))
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            header(&requests[0], "x-amz-server-side-encryption").as_deref(),
            Some("AES256")
        );
        assert_eq!(
            header(&requests[0], "x-amz-meta-x-artifact-duration").as_deref(),
            Some("321")
        );
        let parts: Vec<_> = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT")
            .collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].body, data[..PART_SIZE as usize]);
        assert_eq!(parts[1].body, data[PART_SIZE as usize..]);
        let complete = String::from_utf8(requests.last().unwrap().body.clone()).unwrap();
        assert!(complete.find("part-1").unwrap() < complete.find("part-2").unwrap());
        assert!(complete.contains("<PartNumber>1</PartNumber>"));
        assert!(complete.contains("<PartNumber>2</PartNumber>"));
    }

    #[tokio::test]
    async fn single_put_preserves_encryption() {
        let server = MockServer::start().await;
        mount_single_part(&server).await;
        let mut uploader = uploader(&server.uri());
        uploader.server_side_encryption = Some(S3ServerSideEncryption::AwsKms);
        uploader
            .put("team/hash", &mut io::Cursor::new(b"small"), None)
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            header(&requests[0], "x-amz-server-side-encryption").as_deref(),
            Some("aws:kms")
        );
    }

    async fn assert_aborted(server: &MockServer) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if server
                    .received_requests()
                    .await
                    .unwrap()
                    .iter()
                    .any(|r| r.method.as_str() == "DELETE")
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("multipart upload was not aborted");
    }

    #[tokio::test]
    async fn failed_part_aborts_without_completing() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(403))
            .with_priority(1)
            .mount(&server)
            .await;
        assert!(upload(&server, PART_SIZE as usize).await.is_err());
        assert_aborted(&server).await;
        let requests = server.received_requests().await.unwrap();
        assert!(
            !requests
                .iter()
                .any(|r| r.method.as_str() == "POST" && r.url.query() == Some("uploadId=upload-1"))
        );
    }

    #[tokio::test]
    async fn missing_etag_aborts_without_completing() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(200))
            .with_priority(1)
            .mount(&server)
            .await;
        assert!(upload(&server, PART_SIZE as usize).await.is_err());
        assert_aborted(&server).await;
        let requests = server.received_requests().await.unwrap();
        assert!(
            !requests
                .iter()
                .any(|r| r.method.as_str() == "POST" && r.url.query() == Some("uploadId=upload-1"))
        );
    }

    /// A completion whose response is lost leaves nothing to abort, and the
    /// store answers the abort with `NoSuchUpload`.
    #[tokio::test]
    async fn abort_survives_a_missing_multipart_upload() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("POST"))
            .and(query_param("uploadId", "upload-1"))
            .respond_with(ResponseTemplate::new(403))
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchUpload</Code><Message>The upload does not exist</Message></Error>"#,
                "application/xml",
            ))
            .with_priority(1)
            .mount(&server)
            .await;

        assert!(upload(&server, PART_SIZE as usize).await.is_err());

        assert_aborted(&server).await;
    }

    #[tokio::test]
    async fn failed_completion_aborts() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("POST"))
            .and(query_param("uploadId", "upload-1"))
            .respond_with(ResponseTemplate::new(403))
            .with_priority(1)
            .mount(&server)
            .await;
        assert!(upload(&server, PART_SIZE as usize).await.is_err());
        assert_aborted(&server).await;
    }

    struct ReadFailure;

    impl AsyncRead for ReadFailure {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Ready(Err(io::Error::other("request body failed")))
        }
    }

    #[tokio::test]
    async fn reader_failure_after_first_part_aborts() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        let mut reader = io::Cursor::new(vec![7; PART_SIZE as usize]).chain(ReadFailure);
        assert!(
            uploader(&server.uri())
                .put("team/hash", &mut reader, None)
                .await
                .is_err()
        );
        assert_aborted(&server).await;
    }

    #[tokio::test]
    async fn reader_failure_before_first_part_never_starts_upload() {
        let server = MockServer::start().await;
        assert!(
            uploader(&server.uri())
                .put("team/hash", &mut ReadFailure, None)
                .await
                .is_err()
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    struct PendingReader(std::sync::Arc<tokio::sync::Notify>);

    impl AsyncRead for PendingReader {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            self.0.notify_one();
            std::task::Poll::Pending
        }
    }

    #[tokio::test]
    async fn cancellation_while_waiting_for_body_aborts() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        let pending = std::sync::Arc::new(tokio::sync::Notify::new());
        let mut reader =
            io::Cursor::new(vec![7; PART_SIZE as usize]).chain(PendingReader(pending.clone()));
        let uploader = uploader(&server.uri());
        let task = tokio::spawn(async move { uploader.put("team/hash", &mut reader, None).await });
        tokio::time::timeout(Duration::from_secs(5), pending.notified())
            .await
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_aborted(&server).await;
    }

    #[tokio::test]
    async fn cancellation_during_initiation_aborts_when_id_arrives() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let notification = started.clone();
        Mock::given(method("POST"))
            .and(query_param("uploads", ""))
            .respond_with(move |_: &Request| {
                notification.notify_one();
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_raw("<InitiateMultipartUploadResult><UploadId>upload-1</UploadId></InitiateMultipartUploadResult>", "application/xml")
            })
            .with_priority(1).mount(&server).await;
        let uploader = uploader(&server.uri());
        let task = tokio::spawn(async move {
            uploader
                .put(
                    "team/hash",
                    &mut io::Cursor::new(vec![7; PART_SIZE as usize]),
                    None,
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), started.notified())
            .await
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_aborted(&server).await;
        assert!(
            !server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|r| r.method.as_str() == "PUT")
        );
    }

    #[tokio::test]
    async fn opted_in_checksums_match_initiation_and_completion() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        Mock::given(method("PUT"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "\"part-1\"")
                    .insert_header("x-amz-checksum-crc32", "example-crc32"),
            )
            .with_priority(1)
            .mount(&server)
            .await;
        let uploader = uploader_with_checksums(&server.uri(), S3ChecksumMode::WhenSupported);
        uploader
            .put(
                "team/hash",
                &mut io::Cursor::new(vec![7; PART_SIZE as usize]),
                None,
            )
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            header(&requests[0], "x-amz-checksum-algorithm").as_deref(),
            Some("CRC32")
        );
        assert_eq!(
            header(&requests[1], "x-amz-sdk-checksum-algorithm").as_deref(),
            Some("CRC32")
        );
        let complete = String::from_utf8(requests[2].body.clone()).unwrap();
        assert!(complete.contains("<ChecksumCRC32>example-crc32</ChecksumCRC32>"));
    }

    #[tokio::test]
    async fn retried_part_replays_the_same_bytes() {
        let server = MockServer::start().await;
        mount_multipart(&server).await;
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = attempts.clone();
        Mock::given(method("PUT"))
            .respond_with(move |_: &Request| {
                if count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                    ResponseTemplate::new(503)
                } else {
                    ResponseTemplate::new(200).insert_header("ETag", "\"part-1\"")
                }
            })
            .with_priority(1)
            .mount(&server)
            .await;
        upload(&server, PART_SIZE as usize).await.unwrap();
        let requests = server.received_requests().await.unwrap();
        let parts: Vec<_> = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT")
            .collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].body, parts[1].body);
        assert_eq!(parts[0].body, vec![7; PART_SIZE as usize]);
    }
}
