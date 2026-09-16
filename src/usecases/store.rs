use std::collections::HashMap;

use futures::Stream;
use tokio::io::AsyncRead;

use crate::domain::CacheError;

/// The storage port. `ArtifactCache` reaches any artifact store through this
/// trait. `storage::Storage` is the S3 implementation; tests use an in-memory one.
///
/// This crate's own handlers run on the actix runtime and need no `Send`
/// futures, so callers outside this crate get no `Send` guarantee either.
#[allow(async_fn_in_trait)]
pub trait ArtifactStore: Send + Sync + 'static {
    /// The artifact body as a stream of byte chunks.
    type ByteStream: Stream<Item = Result<bytes::Bytes, Self::StreamError>> + Unpin;
    type StreamError: std::error::Error;

    /// Returns the artifact body stream.
    async fn get(&self, path: &str) -> Result<Self::ByteStream, CacheError>;

    /// Returns the key-value metadata stored with the artifact.
    async fn head(&self, path: &str) -> Result<HashMap<String, String>, CacheError>;

    /// Writes the artifact body and its key-value metadata.
    async fn put<R>(
        &self,
        path: &str,
        reader: &mut R,
        metadata: HashMap<String, String>,
    ) -> Result<(), CacheError>
    where
        R: AsyncRead + Unpin;
}
