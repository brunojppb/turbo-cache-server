use std::collections::HashMap;

/// When Turborepo is configured with `"signature": true` (turbo.json), the CLI
/// computes an HMAC-SHA256 of each artifact and sends it as the `x-artifact-tag`
/// header on PUT. The server persists this value as S3 object metadata and returns
/// it on GET so the client can verify artifact integrity. Without it, every
/// download fails signature verification and is treated as a cache miss.
/// See: https://turborepo.dev/api/remote-cache-spec
pub const ARTIFACT_TAG_HEADER: &str = "x-artifact-tag";

/// Turborepo sends the task run time on PUT and reads it back on GET and HEAD to
/// report how much time the cache saved. Without it, every remote cache hit
/// reports zero time saved.
/// See: https://turborepo.dev/api/remote-cache-spec
pub const ARTIFACT_DURATION_HEADER: &str = "x-artifact-duration";

/// Identifies one artifact in the cache: the Turborepo hash, scoped by team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactId {
    pub team: String,
    pub hash: String,
}

impl ArtifactId {
    /// Object path as stored in the bucket.
    pub fn object_path(&self) -> String {
        format!("/{}/{}", self.team, self.hash)
    }
}

/// The artifact headers Turborepo sends on upload and expects back on download.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ArtifactMetadata {
    pub tag: Option<String>,
    pub duration: Option<u64>,
}

impl ArtifactMetadata {
    /// Reads the protocol key-value form. S3 metadata and HTTP headers share the keys.
    pub fn from_key_values(mut map: HashMap<String, String>) -> Self {
        Self {
            tag: map.remove(ARTIFACT_TAG_HEADER),
            duration: map
                .remove(ARTIFACT_DURATION_HEADER)
                .as_deref()
                .and_then(parse_duration),
        }
    }

    /// Writes the protocol key-value form.
    pub fn into_key_values(self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        if let Some(tag) = self.tag {
            map.insert(ARTIFACT_TAG_HEADER.to_owned(), tag);
        }

        if let Some(duration) = self.duration {
            map.insert(ARTIFACT_DURATION_HEADER.to_owned(), duration.to_string());
        }

        map
    }
}

/// How much of a rejected duration reaches the log.
const MAX_LOGGED_DURATION_CHARS: usize = 32;

/// Turborepo fails the whole cache read on a duration it cannot parse, so a
/// value the server cannot vouch for is dropped instead of passed on.
pub(crate) fn parse_duration(raw: &str) -> Option<u64> {
    match raw.parse::<u64>() {
        Ok(duration) => Some(duration),
        Err(_) => {
            // A client picks this value and can repeat it on every request, so
            // it stays off the warning path and never reaches the log in full.
            let value: String = raw.chars().take(MAX_LOGGED_DURATION_CHARS).collect();
            tracing::debug!(value = %value, "Ignoring malformed x-artifact-duration");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const TAG: &str = "v=1:sha256:abc123";

    #[test]
    fn reads_both_artifact_headers_from_key_values() {
        let stored = HashMap::from([
            (ARTIFACT_TAG_HEADER.to_owned(), TAG.to_owned()),
            (ARTIFACT_DURATION_HEADER.to_owned(), "1234".to_owned()),
        ]);

        assert_eq!(
            ArtifactMetadata::from_key_values(stored),
            ArtifactMetadata {
                tag: Some(TAG.to_owned()),
                duration: Some(1234),
            }
        );
    }

    /// Another writer may share the bucket, so the read path does not trust the
    /// stored value either.
    #[test]
    fn drops_a_malformed_duration_from_key_values() {
        let stored = HashMap::from([(
            ARTIFACT_DURATION_HEADER.to_owned(),
            "not-a-number".to_owned(),
        )]);

        assert_eq!(ArtifactMetadata::from_key_values(stored).duration, None);
    }

    #[test]
    fn writes_the_duration_as_a_decimal_string() {
        let metadata = ArtifactMetadata {
            tag: None,
            duration: Some(7),
        };

        assert_eq!(
            metadata.into_key_values().get(ARTIFACT_DURATION_HEADER),
            Some(&"7".to_owned())
        );
    }

    #[test]
    fn builds_the_object_path_from_team_and_hash() {
        let id = ArtifactId {
            team: "my-team".to_owned(),
            hash: "abc123".to_owned(),
        };

        assert_eq!(id.object_path(), "/my-team/abc123");
    }
}
