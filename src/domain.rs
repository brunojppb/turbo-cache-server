/// One cached artifact, scoped to a storage namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactId {
    pub team: String,
    pub hash: String,
}

impl ArtifactId {
    /// Builds an id when both values are single object-key segments.
    pub fn new(team: String, hash: String) -> Option<Self> {
        (is_key_segment(&team) && is_key_segment(&hash)).then_some(Self { team, hash })
    }
}

/// A value that the S3 request URL keeps as one path segment.
/// URL parsing resolves `.` and `..` and reads `\` as `/` for http(s), so these
/// could move an object key into another namespace or bucket.
fn is_key_segment(value: &str) -> bool {
    !matches!(value, "" | "." | "..") && !value.contains(['/', '\\'])
}

/// The two optional values Turborepo associates with artifact bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactMetadata {
    pub tag: Option<String>,
    pub duration: Option<u64>,
}

impl ArtifactMetadata {
    pub fn parse_duration(raw: &str) -> Option<u64> {
        raw.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(team: &str, hash: &str) -> Option<ArtifactId> {
        ArtifactId::new(team.to_owned(), hash.to_owned())
    }

    #[test]
    fn accepts_plain_team_and_hash() {
        assert!(id("my-team", "abc123").is_some());
        assert!(id("team_x.y", "a..b").is_some());
        assert!(id("...", "abc123").is_some());
    }

    #[test]
    fn rejects_values_that_change_the_object_path() {
        for bad in ["", ".", "..", "a/b", "../other", "a\\b", "..\\other"] {
            assert_eq!(id(bad, "abc123"), None, "team {bad:?}");
            assert_eq!(id("my-team", bad), None, "hash {bad:?}");
        }
    }
}
