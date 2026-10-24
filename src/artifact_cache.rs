use tokio::io::AsyncRead;

use crate::{
    domain::{ArtifactId, ArtifactMetadata},
    storage::{ArtifactStream, Storage, StorageError},
};

/// Coordinates the cache operations whose policy spans more than one S3 request.
pub struct ArtifactCache {
    storage: Storage,
}

impl ArtifactCache {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub async fn put<R: AsyncRead + Unpin>(
        &self,
        id: &ArtifactId,
        reader: &mut R,
        metadata: &ArtifactMetadata,
    ) -> Result<(), StorageError> {
        self.storage.put_file_stream(id, reader, metadata).await
    }

    pub async fn head(&self, id: &ArtifactId) -> Result<ArtifactMetadata, StorageError> {
        self.storage.head_file(id).await
    }

    pub async fn get(
        &self,
        id: &ArtifactId,
    ) -> Result<(ArtifactStream, ArtifactMetadata), StorageError> {
        let (body, metadata) = tokio::join!(self.storage.get_file(id), self.storage.head_file(id));
        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(error) => {
                tracing::warn!(error = %error, "HEAD request failed, omitting object metadata");
                ArtifactMetadata::default()
            }
        };
        Ok((body?, metadata))
    }
}
