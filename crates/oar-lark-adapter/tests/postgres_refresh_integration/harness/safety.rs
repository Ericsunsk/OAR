use super::constants::{
    CLIENT_SECRET, NEW_ACCESS_TOKEN, NEW_REFRESH_TOKEN, OLD_FP, SEED_ACCESS_TOKEN,
    SEED_REFRESH_TOKEN,
};

pub(crate) fn assert_no_sensitive_text(text: &str) {
    for needle in [
        SEED_ACCESS_TOKEN,
        SEED_REFRESH_TOKEN,
        NEW_ACCESS_TOKEN,
        NEW_REFRESH_TOKEN,
        CLIENT_SECRET,
        "access_token",
        "refresh_token",
        "authorization_code",
        "Authorization",
        "Bearer",
        "encrypted_primary",
        "encrypted_renewal",
        OLD_FP,
        "fp-current",
    ] {
        assert!(
            !text.contains(needle),
            "sensitive marker leaked into text: {needle}"
        );
    }
}

pub(crate) fn assert_no_byte_secret(bytes: &[u8]) {
    for needle in [
        SEED_ACCESS_TOKEN,
        SEED_REFRESH_TOKEN,
        NEW_ACCESS_TOKEN,
        NEW_REFRESH_TOKEN,
    ] {
        assert!(
            !contains_subslice(bytes, needle.as_bytes()),
            "sensitive marker leaked into encrypted blob: {needle}"
        );
    }
}

pub(crate) fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
