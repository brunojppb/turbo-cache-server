use tokio::io::AsyncRead;

use crate::domain::{ArtifactId, ArtifactMetadata, CacheError};

use super::ArtifactStore;

/// The artifact cache use-cases: store, fetch, and check artifacts.
pub struct ArtifactCache<S: ArtifactStore> {
    store: S,
}

impl<S: ArtifactStore> ArtifactCache<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Stores the artifact body with its metadata.
    pub async fn store<R>(
        &self,
        id: &ArtifactId,
        metadata: ArtifactMetadata,
        reader: &mut R,
    ) -> Result<(), CacheError>
    where
        R: AsyncRead + Unpin,
    {
        self.store
            .put(&id.object_path(), reader, metadata.into_key_values())
            .await
    }

    /// Returns the artifact body stream and its metadata.
    pub async fn fetch(
        &self,
        id: &ArtifactId,
    ) -> Result<(S::ByteStream, ArtifactMetadata), CacheError> {
        let path = id.object_path();

        let (body, metadata) = tokio::join!(self.store.get(&path), self.store.head(&path));

        let metadata = match metadata {
            Ok(map) => ArtifactMetadata::from_key_values(map),
            // A failed metadata lookup must not fail an otherwise good download.
            Err(error) => {
                tracing::warn!(error = %error, path, "Metadata lookup failed, omitting artifact metadata");
                ArtifactMetadata::default()
            }
        };

        Ok((body?, metadata))
    }

    /// Returns the artifact metadata, or `NotFound`.
    pub async fn check(&self, id: &ArtifactId) -> Result<ArtifactMetadata, CacheError> {
        let map = self.store.head(&id.object_path()).await?;

        Ok(ArtifactMetadata::from_key_values(map))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::sync::Mutex;

    use bytes::Bytes;
    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncReadExt;

    use super::*;

    /// In-memory `ArtifactStore` with switches to force each read path to fail.
    #[derive(Default)]
    #[allow(clippy::type_complexity)]
    struct InMemoryStore {
        objects: Mutex<HashMap<String, (Vec<u8>, HashMap<String, String>)>>,
        fail_get: bool,
        fail_head: bool,
    }

    impl ArtifactStore for InMemoryStore {
        type ByteStream = futures::stream::Iter<std::vec::IntoIter<Result<Bytes, Infallible>>>;
        type StreamError = Infallible;

        async fn get(&self, path: &str) -> Result<Self::ByteStream, CacheError> {
            if self.fail_get {
                return Err(CacheError::StoreUnavailable("get switched off".into()));
            }

            let objects = self.objects.lock().unwrap();
            let (bytes, _) = objects.get(path).ok_or(CacheError::NotFound)?;

            Ok(futures::stream::iter(vec![Ok(Bytes::from(bytes.clone()))]))
        }

        async fn head(&self, path: &str) -> Result<HashMap<String, String>, CacheError> {
            if self.fail_head {
                return Err(CacheError::StoreUnavailable("head switched off".into()));
            }

            let objects = self.objects.lock().unwrap();
            let (_, metadata) = objects.get(path).ok_or(CacheError::NotFound)?;

            Ok(metadata.clone())
        }

        async fn put<R>(
            &self,
            path: &str,
            reader: &mut R,
            metadata: HashMap<String, String>,
        ) -> Result<(), CacheError>
        where
            R: AsyncRead + Unpin,
        {
            let mut buffer = Vec::new();
            reader
                .read_to_end(&mut buffer)
                .await
                .expect("in-memory read cannot fail");

            self.objects
                .lock()
                .unwrap()
                .insert(path.to_owned(), (buffer, metadata));

            Ok(())
        }
    }

    fn artifact_id() -> ArtifactId {
        ArtifactId {
            team: "my-team".to_owned(),
            hash: "abc123".to_owned(),
        }
    }

    async fn read_body(
        stream: futures::stream::Iter<std::vec::IntoIter<Result<Bytes, Infallible>>>,
    ) -> Vec<u8> {
        let chunks: Vec<Result<Bytes, Infallible>> = stream.collect().await;
        chunks
            .into_iter()
            .flat_map(|chunk| chunk.unwrap())
            .collect()
    }

    #[tokio::test]
    async fn check_reports_not_found_for_a_missing_artifact() {
        let cache = ArtifactCache::new(InMemoryStore::default());

        let result = cache.check(&artifact_id()).await;

        assert!(matches!(result, Err(CacheError::NotFound)));
    }

    #[tokio::test]
    async fn fetch_reports_not_found_for_a_missing_artifact() {
        let cache = ArtifactCache::new(InMemoryStore::default());

        let result = cache.fetch(&artifact_id()).await;

        assert!(matches!(result, Err(CacheError::NotFound)));
    }

    #[tokio::test]
    async fn body_and_metadata_round_trip_through_store_and_fetch() {
        let cache = ArtifactCache::new(InMemoryStore::default());
        let metadata = ArtifactMetadata {
            tag: Some("v=1:sha256:abc123".to_owned()),
            duration: Some(42),
        };

        let mut reader = &b"artifact-bytes"[..];
        cache
            .store(&artifact_id(), metadata, &mut reader)
            .await
            .unwrap();

        let (body, fetched) = cache.fetch(&artifact_id()).await.unwrap();

        assert_eq!(
            fetched,
            ArtifactMetadata {
                tag: Some("v=1:sha256:abc123".to_owned()),
                duration: Some(42),
            }
        );
        assert_eq!(read_body(body).await, b"artifact-bytes");
    }

    /// A failed metadata lookup must not fail an otherwise good download.
    #[tokio::test]
    async fn fetch_returns_empty_metadata_when_the_metadata_lookup_fails() {
        let store = InMemoryStore {
            fail_head: true,
            ..Default::default()
        };
        store.objects.lock().unwrap().insert(
            artifact_id().object_path(),
            (b"artifact-bytes".to_vec(), HashMap::new()),
        );
        let cache = ArtifactCache::new(store);

        let (body, metadata) = cache.fetch(&artifact_id()).await.unwrap();

        assert_eq!(metadata, ArtifactMetadata::default());
        assert_eq!(read_body(body).await, b"artifact-bytes");
    }

    #[tokio::test]
    async fn store_writes_under_the_artifact_object_path() {
        let cache = ArtifactCache::new(InMemoryStore::default());

        let mut reader = &b"artifact-bytes"[..];
        cache
            .store(&artifact_id(), ArtifactMetadata::default(), &mut reader)
            .await
            .unwrap();

        let objects = cache.store.objects.lock().unwrap();
        assert!(objects.contains_key("/my-team/abc123"));
    }
}
