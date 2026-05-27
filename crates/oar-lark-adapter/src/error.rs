#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeishuRefreshFailureClass {
    ReauthRequired,
    ConfigRequired,
    Transient,
}

pub fn classify_feishu_refresh_failure(code: i64, http_status: u16) -> FeishuRefreshFailureClass {
    match code {
        // OAuth refresh tokens are single-use and may require user re-authorization
        // when invalid/expired/revoked or mismatched with client context.
        20024 | 20026 | 20037 | 20064 | 20073 => FeishuRefreshFailureClass::ReauthRequired,
        // These represent app/client/scope/request issues and should stop retries.
        20002 | 20008 | 20009 | 20010 | 20036 | 20048 | 20063 | 20066 | 20067 | 20068 | 20069
        | 20070 | 20074 => FeishuRefreshFailureClass::ConfigRequired,
        20050 | 20072 => FeishuRefreshFailureClass::Transient,
        _ => classify_by_http_status(http_status),
    }
}

pub fn safe_error_for_failure_class(class: FeishuRefreshFailureClass) -> &'static str {
    match class {
        FeishuRefreshFailureClass::ReauthRequired => "invalid_grant",
        FeishuRefreshFailureClass::ConfigRequired => "refresh_config_required",
        FeishuRefreshFailureClass::Transient => "temporarily unavailable",
    }
}

fn classify_by_http_status(http_status: u16) -> FeishuRefreshFailureClass {
    match http_status {
        500..=599 | 429 => FeishuRefreshFailureClass::Transient,
        400..=499 => FeishuRefreshFailureClass::ConfigRequired,
        _ => FeishuRefreshFailureClass::Transient,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_feishu_refresh_failure, FeishuRefreshFailureClass};

    #[test]
    fn refresh_token_reauth_codes_map_to_reauth_required() {
        for code in [20024, 20026, 20037, 20064, 20073] {
            assert_eq!(
                classify_feishu_refresh_failure(code, 400),
                FeishuRefreshFailureClass::ReauthRequired
            );
        }
    }

    #[test]
    fn config_related_codes_map_to_config_required() {
        for code in [
            20002, 20008, 20009, 20010, 20036, 20048, 20063, 20066, 20067, 20068, 20069, 20070,
            20074,
        ] {
            assert_eq!(
                classify_feishu_refresh_failure(code, 400),
                FeishuRefreshFailureClass::ConfigRequired
            );
        }
    }

    #[test]
    fn transient_codes_and_http_status_fallback_are_retryable() {
        for code in [20050, 20072] {
            assert_eq!(
                classify_feishu_refresh_failure(code, 500),
                FeishuRefreshFailureClass::Transient
            );
        }
        assert_eq!(
            classify_feishu_refresh_failure(29999, 503),
            FeishuRefreshFailureClass::Transient
        );
        assert_eq!(
            classify_feishu_refresh_failure(29999, 429),
            FeishuRefreshFailureClass::Transient
        );
    }

    #[test]
    fn unknown_4xx_failures_stop_retry_loop() {
        for status in [400, 401, 403] {
            assert_eq!(
                classify_feishu_refresh_failure(29999, status),
                FeishuRefreshFailureClass::ConfigRequired
            );
        }
    }
}
