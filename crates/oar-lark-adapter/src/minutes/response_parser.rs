use super::error::FeishuMinutesReadError;
use super::feishu_types::FeishuMinuteGetResponse;
use super::types::MinuteReadSummary;

pub(super) fn map_status_or_parse_minute(
    status: u16,
    body: &str,
) -> Result<MinuteReadSummary, FeishuMinutesReadError> {
    map_status_or_parse_minutes_response(status, body, |body| {
        let parsed: FeishuMinuteGetResponse =
            serde_json::from_str(body).map_err(|_| FeishuMinutesReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let minute = parsed
            .data
            .and_then(|data| data.minute)
            .ok_or(FeishuMinutesReadError::InvalidJson)?;
        Ok(MinuteReadSummary::from_feishu_minute(minute))
    })
}

fn map_status_or_parse_minutes_response<T>(
    status: u16,
    body: &str,
    parse_success: impl FnOnce(&str) -> Result<T, FeishuMinutesReadError>,
) -> Result<T, FeishuMinutesReadError> {
    match status {
        200..=299 => parse_success(body),
        401 => Err(FeishuMinutesReadError::Unauthorized),
        403 => Err(FeishuMinutesReadError::Forbidden),
        404 => Err(FeishuMinutesReadError::NotFound),
        429 => Err(FeishuMinutesReadError::UpstreamTransient),
        400..=499 => Err(FeishuMinutesReadError::UpstreamClient),
        _ => Err(FeishuMinutesReadError::UpstreamTransient),
    }
}

fn map_api_code(code: i64) -> FeishuMinutesReadError {
    match code {
        401 | 99991663 | 99991664 => FeishuMinutesReadError::Unauthorized,
        403 | 2091005 => FeishuMinutesReadError::Forbidden,
        404 | 2091002 | 2091004 => FeishuMinutesReadError::NotFound,
        2091001 => FeishuMinutesReadError::UpstreamClient,
        2091003 | 99991400 => FeishuMinutesReadError::UpstreamTransient,
        _ => FeishuMinutesReadError::ApiFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_permission_not_found_and_pending_codes() {
        assert_eq!(
            map_status_or_parse_minute(200, r#"{"code":2091005,"msg":"forbidden"}"#)
                .expect_err("forbidden"),
            FeishuMinutesReadError::Forbidden
        );
        assert_eq!(
            map_status_or_parse_minute(200, r#"{"code":2091002,"msg":"not found"}"#)
                .expect_err("not found"),
            FeishuMinutesReadError::NotFound
        );
        assert_eq!(
            map_status_or_parse_minute(200, r#"{"code":2091003,"msg":"not ready"}"#)
                .expect_err("not ready"),
            FeishuMinutesReadError::UpstreamTransient
        );
    }

    #[test]
    fn parses_minute_summary_without_sensitive_fields() {
        let summary = map_status_or_parse_minute(
            200,
            r#"{"code":0,"data":{"minute":{"title":" Weekly Sync ","duration":"314000","create_time":"1669098360477","owner_id":"ou_secret","url":"https://sample.feishu.cn/minutes/obcnsecret","cover":"https://cover"}}}"#,
        )
        .expect("summary");

        assert_eq!(summary.title.as_deref(), Some("Weekly Sync"));
        assert_eq!(summary.duration_ms.as_deref(), Some("314000"));
        assert_eq!(summary.create_time_ms.as_deref(), Some("1669098360477"));
        let debug = format!("{summary:?}");
        assert!(!debug.contains("ou_secret"));
        assert!(!debug.contains("obcnsecret"));
        assert!(!debug.contains("https://cover"));
    }

    #[test]
    fn invalid_json_and_missing_minute_fail_closed() {
        assert_eq!(
            map_status_or_parse_minute(200, r#"{"code":0,"data":{}}"#).expect_err("missing"),
            FeishuMinutesReadError::InvalidJson
        );
        assert_eq!(
            map_status_or_parse_minute(200, "not json").expect_err("json"),
            FeishuMinutesReadError::InvalidJson
        );
    }
}
