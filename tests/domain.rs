use decay::domain::{ArtifactId, ArtifactMetadata};

#[test]
fn artifact_identity_keeps_namespace_and_hash_distinct() {
    let id = ArtifactId::new("team-a".to_owned(), "hash-b".to_owned());

    assert_eq!(id.team(), "team-a");
    assert_eq!(id.hash(), "hash-b");
}

#[test]
fn duration_is_canonicalized_and_bad_values_are_omitted() {
    assert_eq!(ArtifactMetadata::parse_duration("007"), Some(7));
    assert_eq!(ArtifactMetadata::parse_duration("invalid"), None);
}
