const GENERIC_SENSITIVE_SEGMENTS: &[&str] = &[
    "token",
    "secret",
    "password",
    "cookie",
    "oauth",
    "credential",
];

const DIRECT_SENSITIVE_MARKERS: &[&str] = &[
    "access token",
    "access_token",
    "accesstoken",
    "refresh token",
    "refresh_token",
    "refreshtoken",
    "authorization:",
    "authorization code",
    "authorization_code",
    "authorizationcode",
    "auth code",
    "auth_code",
    "bearer ",
    "client_secret",
    "oauth_grant",
    "stdout",
    "stderr",
];

pub fn contains_sensitive_marker(input: &str) -> bool {
    let lowered = input.to_ascii_lowercase();
    if DIRECT_SENSITIVE_MARKERS
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        return true;
    }

    lowered
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .any(|segment| GENERIC_SENSITIVE_SEGMENTS.contains(&segment))
}
