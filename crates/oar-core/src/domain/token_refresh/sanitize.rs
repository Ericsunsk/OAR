const REDACTED_REFRESH_ERROR: &str = "<redacted refresh error>";

pub(crate) fn sanitize_refresh_error_for_report(reason: &str) -> String {
    match reason.trim() {
        "invalid_grant" => "invalid_grant".to_string(),
        "temporarily unavailable" => "temporarily unavailable".to_string(),
        _ => REDACTED_REFRESH_ERROR.to_string(),
    }
}
