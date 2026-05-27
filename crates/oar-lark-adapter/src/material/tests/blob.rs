use crate::material::blob::{
    compose_encrypted_grant_blob, parse_encrypted_grant_blob, BlobParseError,
};

#[test]
fn parse_rejects_invalid_shapes() {
    assert!(matches!(
        parse_encrypted_grant_blob(&[]),
        Err(BlobParseError::Invalid)
    ));
    assert!(matches!(
        parse_encrypted_grant_blob(&[0, 0, 0, 1, 7, 0, 0, 0]),
        Err(BlobParseError::Invalid)
    ));
}

#[test]
fn compose_roundtrip_extracts_segments() {
    let primary = vec![1, 2, 3, 4];
    let renewal = vec![8, 9];
    let blob = compose_encrypted_grant_blob(primary.clone(), renewal.clone());
    let (parsed_primary, parsed_renewal) =
        parse_encrypted_grant_blob(&blob).expect("blob should parse");
    assert_eq!(parsed_primary, primary.as_slice());
    assert_eq!(parsed_renewal, renewal.as_slice());
}
