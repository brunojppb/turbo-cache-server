/// One cached artifact, scoped to a storage namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactId {
    pub team: String,
    pub hash: String,
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
