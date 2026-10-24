use decay::domain::ArtifactMetadata;

#[test]
fn duration_is_canonicalized_and_bad_values_are_omitted() {
    assert_eq!(ArtifactMetadata::parse_duration("007"), Some(7));
    assert_eq!(ArtifactMetadata::parse_duration("invalid"), None);
}
