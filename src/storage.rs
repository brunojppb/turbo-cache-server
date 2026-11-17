use std::collections::HashMap;
use std::time::Duration;
use std::{fmt, io};

use futures::{Stream, StreamExt, stream};
use s3::{Bucket, Region, creds::Credentials, error::S3Error, request::ResponseDataStream};
use secrecy::ExposeSecret;
use tokio::io::AsyncRead;

use crate::app_settings::{AppSettings, S3ServerSideEncryption};
use crate::domain::{ArtifactId, ArtifactMetadata};

const SSE_HEADER: http::HeaderName = http::HeaderName::from_static("x-amz-server-side-encryption");
const ARTIFACT_TAG: &str = "x-artifact-tag";
const ARTIFACT_DURATION: &str = "x-artifact-duration";

pub type ArtifactStream = ResponseDataStream;

/// Time limits for requests to the bucket.
#[derive(Clone, Copy, Debug)]
struct S3Timeouts {
    /// Limit for a whole request with a short body: HEAD, and each PUT part.
    request: Duration,
    /// Limit for each wait during a download: for the response, then for each chunk.
    stream_idle: Duration,
}

impl Default for S3Timeouts {
    fn default() -> Self {
        Self {
            request: Duration::from_secs(60),
            stream_idle: Duration::from_secs(60),
        }
    }
}

fn timed_out() -> S3Error {
    S3Error::Io(io::Error::new(
        io::ErrorKind::TimedOut,
        "S3 sent no data within the time limit",
    ))
}

/// Ends the stream with an error when the next chunk takes longer than `idle`.
fn with_idle_timeout<S, T>(inner: S, idle: Duration) -> impl Stream<Item = Result<T, S3Error>>
where
    S: Stream<Item = Result<T, S3Error>> + Unpin,
{
    stream::unfold(Some(inner), move |state| async move {
        let mut inner = state?;
        match tokio::time::timeout(idle, inner.next()).await {
            Ok(Some(item)) => Some((item, Some(inner))),
            Ok(None) => None,
            Err(_) => Some((Err(timed_out()), None)),
        }
    })
}

fn object_path(id: &ArtifactId) -> String {
    format!("/{}/{}", id.team, id.hash)
}

pub(crate) fn metadata_from_storage(mut values: HashMap<String, String>) -> ArtifactMetadata {
    ArtifactMetadata {
        tag: values.remove(ARTIFACT_TAG),
        duration: values
            .remove(ARTIFACT_DURATION)
            .as_deref()
            .and_then(ArtifactMetadata::parse_duration),
    }
}

pub(crate) fn metadata_to_storage(metadata: &ArtifactMetadata) -> HashMap<String, String> {
    let mut values = HashMap::new();
    if let Some(tag) = &metadata.tag {
        values.insert(ARTIFACT_TAG.to_owned(), tag.clone());
    }
    if let Some(duration) = metadata.duration {
        values.insert(ARTIFACT_DURATION.to_owned(), duration.to_string());
    }
    values
}

#[derive(Debug)]
pub enum StorageError {
    /// The bucket answered, but holds no object under that path.
    NotFound,
    /// The bucket could not be reached, or rejected the request.
    Unreachable(S3Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "no such object in the bucket"),
            Self::Unreachable(error) => write!(f, "S3 request failed: {error}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotFound => None,
            Self::Unreachable(error) => Some(error),
        }
    }
}

impl From<S3Error> for StorageError {
    fn from(error: S3Error) -> Self {
        match error {
            S3Error::HttpFailWithBody(404, _) => Self::NotFound,
            other => Self::Unreachable(other),
        }
    }
}

pub struct Storage {
    bucket: Box<Bucket>,
    /// The same bucket without a total time limit, because a large download
    /// can take longer than any fixed limit. `timeouts.stream_idle` guards it.
    stream_bucket: Box<Bucket>,
    timeouts: S3Timeouts,
    server_side_encryption: Option<S3ServerSideEncryption>,
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("bucket_name", &self.bucket.name)
            .field("region", &self.bucket.region)
            .field("timeouts", &self.timeouts)
            .field("server_side_encryption", &self.server_side_encryption)
            .finish_non_exhaustive()
    }
}

impl Storage {
    pub fn new(settings: &AppSettings) -> Self {
        Self::with_timeouts(settings, S3Timeouts::default())
    }

    fn with_timeouts(settings: &AppSettings, timeouts: S3Timeouts) -> Self {
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
            (Some(access_key), Some(secret_key)) => Credentials::new(
                Some(access_key.expose_secret()),
                Some(secret_key.expose_secret()),
                None,
                None,
                None,
            )
            .unwrap(),
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
        bucket.set_request_timeout(Some(timeouts.request));

        let mut stream_bucket = bucket.clone();
        stream_bucket.set_request_timeout(None);

        Self {
            bucket,
            stream_bucket,
            timeouts,
            server_side_encryption: settings.s3_server_side_encryption,
        }
    }

    /// Streams the file from the S3 bucket
    #[tracing::instrument(name = "get S3 file", skip(id))]
    pub async fn get_file(&self, id: &ArtifactId) -> Result<ArtifactStream, StorageError> {
        let idle = self.timeouts.stream_idle;
        let mut file =
            tokio::time::timeout(idle, self.stream_bucket.get_object_stream(object_path(id)))
                .await
                .map_err(|_| StorageError::Unreachable(timed_out()))??;
        file.bytes = Box::pin(with_idle_timeout(file.bytes, idle));
        Ok(file)
    }

    /// Returns the user metadata stored on the S3 object, or reports that the
    /// object is missing.
    #[tracing::instrument(name = "head S3 file", skip(id))]
    pub async fn head_file(&self, id: &ArtifactId) -> Result<ArtifactMetadata, StorageError> {
        let (head_result, _status) = self.bucket.head_object(object_path(id)).await?;
        Ok(metadata_from_storage(
            head_result.metadata.unwrap_or_default(),
        ))
    }

    /// Streams the given data to the S3 bucket under the given path.
    /// Each metadata key-value pair is persisted as S3 user metadata
    /// (x-amz-meta-*) so it can be retrieved on subsequent HEADs.
    #[tracing::instrument(name = "put S3 file stream", skip(id, reader, metadata))]
    pub async fn put_file_stream<R>(
        &self,
        id: &ArtifactId,
        reader: &mut R,
        metadata: &ArtifactMetadata,
    ) -> Result<(), StorageError>
    where
        R: AsyncRead + Unpin,
    {
        let path = object_path(id);
        let mut builder = self.bucket.put_object_stream_builder(path);

        if let Some(encryption) = self.server_side_encryption {
            builder = builder
                .with_header(SSE_HEADER, encryption.as_str())
                .expect("Invalid server-side encryption header value");
        }

        for (key, value) in metadata_to_storage(metadata) {
            builder = builder
                .with_metadata(&key, &value)
                .expect("Invalid metadata value");
        }

        builder.execute_stream(reader).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_path_keeps_team_and_hash_in_the_existing_layout() {
        let id = ArtifactId {
            team: "my-team".to_owned(),
            hash: "abc123".to_owned(),
        };
        assert_eq!(object_path(&id), "/my-team/abc123");
    }

    /// Every id that `ArtifactId::new` accepts maps to one key under the bucket,
    /// and no two ids share a key. The sweep covers every pair of values built
    /// from up to three tokens of `TOKENS`. Run: `cargo test --locked --lib artifact_key_namespace`.
    #[test]
    fn artifact_key_namespace() {
        const TOKENS: [&str; 8] = ["a", ".", "%2e", "%", "/", "\\", "?", "#"];
        const MAX_TOKENS: usize = 3;
        let bucket_url = "https://s3.example.test/bucket";

        let mut values = vec![String::new()];
        let mut layer = vec![String::new()];
        for _ in 0..MAX_TOKENS {
            layer = layer
                .iter()
                .flat_map(|prefix| TOKENS.iter().map(move |t| format!("{prefix}{t}")))
                .collect();
            values.extend(layer.iter().cloned());
        }

        let mut seen: HashMap<String, (String, String)> = HashMap::new();
        for team in &values {
            for hash in &values {
                let Some(id) = ArtifactId::new(team.clone(), hash.clone()) else {
                    continue;
                };
                let path = object_path(&id);
                // Same URL build as rust-s3's `Request::url`.
                let url = url::Url::parse(&format!(
                    "{bucket_url}/{}",
                    s3::signing::uri_encode(&path[1..], false)
                ))
                .unwrap_or_else(|e| panic!("({team:?}, {hash:?}) gives an invalid URL: {e}"));
                let segments: Vec<&str> = url.path_segments().expect("base URL").collect();
                assert_eq!(
                    segments.len(),
                    3,
                    "({team:?}, {hash:?}) resolves to {}",
                    url.path()
                );
                assert_eq!(
                    segments[0], "bucket",
                    "({team:?}, {hash:?}) leaves the bucket"
                );
                assert_eq!(
                    url.host_str(),
                    Some("s3.example.test"),
                    "({team:?}, {hash:?})"
                );
                assert_eq!(url.query(), None, "({team:?}, {hash:?}) adds a query");
                assert_eq!(url.fragment(), None, "({team:?}, {hash:?}) adds a fragment");
                if let Some(other) = seen.insert(path.clone(), (team.clone(), hash.clone())) {
                    panic!("({team:?}, {hash:?}) and {other:?} share the key {path}");
                }
            }
        }
        assert!(seen.len() > 1000, "the sweep accepted too few ids");
    }

    /// The bucket holds the parsed number, not the bytes the client sent.
    #[test]
    fn writes_the_parsed_duration_to_storage() {
        let metadata = ArtifactMetadata {
            duration: ArtifactMetadata::parse_duration("007"),
            ..ArtifactMetadata::default()
        };

        let stored = metadata_to_storage(&metadata);

        assert_eq!(stored.get(ARTIFACT_DURATION).map(String::as_str), Some("7"));
    }

    #[test]
    fn reads_both_artifact_headers_from_storage() {
        let tag = "v=1:sha256:abc123";
        let stored = HashMap::from([
            (ARTIFACT_TAG.to_owned(), tag.to_owned()),
            (ARTIFACT_DURATION.to_owned(), "1234".to_owned()),
        ]);

        assert_eq!(
            metadata_from_storage(stored),
            ArtifactMetadata {
                tag: Some(tag.to_owned()),
                duration: Some(1234),
            }
        );
    }

    /// Another writer may share the bucket, so the read path does not trust the
    /// stored value either.
    #[test]
    fn drops_a_malformed_duration_from_storage() {
        let stored = HashMap::from([(ARTIFACT_DURATION.to_owned(), "not-a-number".to_owned())]);

        assert_eq!(metadata_from_storage(stored).duration, None);
    }

    fn settings_with_credentials() -> AppSettings {
        AppSettings {
            host: "127.0.0.1".to_owned(),
            port: 8000,
            s3_access_key: Some("super-secret-access-key".into()),
            s3_secret_key: Some("super-secret-secret-key".into()),
            s3_endpoint: Some("http://localhost:9000".to_owned()),
            s3_use_path_style: true,
            s3_region: "eu-central-1".to_owned(),
            s3_bucket_name: "turbo".to_owned(),
            s3_server_side_encryption: None,
            turbo_token: None,
            turbo_tokens_dir: None,
            turbo_tokens_reload_interval: crate::auth::team_tokens::DEFAULT_RELOAD_INTERVAL,
        }
    }

    /// What the fake S3 does after it reads a request.
    #[derive(Clone, Copy)]
    enum FakeS3 {
        /// Sends `chunks` bytes of the body, one each `gap`.
        Slow { chunks: usize, gap: Duration },
        /// Sends the headers and one byte, then waits forever.
        StallBody,
        /// Never answers.
        StallHeaders,
    }

    /// Starts a fake S3 on loopback and returns its endpoint.
    fn start_fake_s3(behavior: FakeS3) -> String {
        use std::io::{BufRead, BufReader, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap_or(0) > 2 {
                        line.clear();
                    }
                    match behavior {
                        FakeS3::Slow { chunks, gap } => {
                            let head =
                                format!("HTTP/1.1 200 OK\r\nContent-Length: {chunks}\r\n\r\n");
                            let _ = stream.write_all(head.as_bytes());
                            for _ in 0..chunks {
                                let _ = stream.write_all(b"x");
                                let _ = stream.flush();
                                std::thread::sleep(gap);
                            }
                        }
                        FakeS3::StallBody => {
                            let _ =
                                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nx");
                            let _ = stream.flush();
                            std::thread::sleep(Duration::from_secs(30));
                        }
                        FakeS3::StallHeaders => std::thread::sleep(Duration::from_secs(30)),
                    }
                });
            }
        });
        endpoint
    }

    fn storage_for(endpoint: String, timeouts: S3Timeouts) -> Storage {
        let settings = AppSettings {
            s3_endpoint: Some(endpoint),
            ..settings_with_credentials()
        };
        Storage::with_timeouts(&settings, timeouts)
    }

    const SHORT: S3Timeouts = S3Timeouts {
        request: Duration::from_millis(300),
        stream_idle: Duration::from_millis(300),
    };

    fn artifact() -> ArtifactId {
        ArtifactId::new("team".to_owned(), "abc123".to_owned()).unwrap()
    }

    async fn read_all(storage: &Storage) -> Result<usize, StorageError> {
        let mut file = storage.get_file(&artifact()).await?;
        let mut read = 0;
        while let Some(chunk) = file.bytes.next().await {
            read += chunk.map_err(StorageError::Unreachable)?.len();
        }
        Ok(read)
    }

    /// A download that keeps sending data finishes, even when it takes longer
    /// than the limit for a whole request.
    #[tokio::test]
    async fn streams_a_download_that_outlasts_the_request_limit() {
        let gap = Duration::from_millis(100);
        let storage = storage_for(start_fake_s3(FakeS3::Slow { chunks: 8, gap }), SHORT);

        assert_eq!(read_all(&storage).await.unwrap(), 8);
    }

    #[tokio::test]
    async fn stops_a_download_when_the_body_stalls() {
        let storage = storage_for(start_fake_s3(FakeS3::StallBody), SHORT);

        let error = read_all(&storage).await.unwrap_err();

        assert!(error.to_string().contains("time limit"), "{error}");
    }

    #[tokio::test]
    async fn stops_a_download_when_s3_never_answers() {
        let storage = storage_for(start_fake_s3(FakeS3::StallHeaders), SHORT);

        let error = read_all(&storage).await.unwrap_err();

        assert!(error.to_string().contains("time limit"), "{error}");
    }

    /// Storage ends up in tracing spans, which record it through `Debug`.
    #[test]
    fn debug_output_hides_s3_credentials() {
        let storage = Storage::new(&settings_with_credentials());

        let debug_output = format!("{storage:?}");

        assert!(!debug_output.contains("super-secret-access-key"));
        assert!(!debug_output.contains("super-secret-secret-key"));
    }
}
