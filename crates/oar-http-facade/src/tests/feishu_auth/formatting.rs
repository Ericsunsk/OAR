use crate::feishu_auth::iso8601_utc;
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn iso8601_formatter_uses_utc_epoch_contract() {
    assert_eq!(iso8601_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    assert_eq!(
        iso8601_utc(UNIX_EPOCH + Duration::from_secs(86_400)),
        "1970-01-02T00:00:00Z"
    );
}
