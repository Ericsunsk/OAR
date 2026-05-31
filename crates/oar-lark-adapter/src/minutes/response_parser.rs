use super::error::FeishuMinutesReadError;
use super::feishu_types::{FeishuMinuteGetResponse, FeishuMinuteSearchResponse};
use super::types::{non_empty, MinuteReadSummary, MinuteSearchPage};

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

pub(super) fn map_status_or_parse_minute_search(
    status: u16,
    body: &str,
) -> Result<MinuteSearchPage, FeishuMinutesReadError> {
    map_status_or_parse_minutes_response(status, body, |body| {
        let parsed: FeishuMinuteSearchResponse =
            serde_json::from_str(body).map_err(|_| FeishuMinutesReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let data = parsed.data.ok_or(FeishuMinutesReadError::InvalidJson)?;
        Ok(MinuteSearchPage {
            minutes: data
                .items
                .into_iter()
                .map(MinuteReadSummary::from_feishu_minute)
                .collect(),
            total: data.total,
            has_more: data.has_more,
            page_token: non_empty(data.page_token),
        })
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
        401 | 2094011 | 2094012 | 99991663 | 99991664 => FeishuMinutesReadError::Unauthorized,
        403 | 2091005 => FeishuMinutesReadError::Forbidden,
        404 | 2091002 | 2091004 => FeishuMinutesReadError::NotFound,
        2091001 | 2094001..=2094007 | 2094101 | 2094102 => FeishuMinutesReadError::UpstreamClient,
        2091003 | 2095001 | 2095002 | 2095101 | 99991400 => {
            FeishuMinutesReadError::UpstreamTransient
        }
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

    #[test]
    fn parses_minute_search_page_without_sensitive_fields() {
        let page = map_status_or_parse_minute_search(
            200,
            r#"{"code":0,"data":{"items":[{"title":" Search Hit ","duration_ms":"12000","create_time_ms":"1669098360477","token":"obcnsecret","url":"https://sample.feishu.cn/minutes/obcnsecret","owner_id":"ou_secret"}],"total":1,"has_more":true,"page_token":"next_page_token"}}"#,
        )
        .expect("page");

        assert_eq!(page.total, Some(1));
        assert!(page.has_more);
        assert_eq!(page.page_token.as_deref(), Some("next_page_token"));
        assert_eq!(page.minutes[0].title.as_deref(), Some("Search Hit"));
        let debug = format!("{page:?}");
        assert!(!debug.contains("ou_secret"));
        assert!(!debug.contains("obcnsecret"));
    }

    #[test]
    fn minute_search_ignores_official_snippets_and_link_metadata() {
        let page = map_status_or_parse_minute_search(
            200,
            r#"{"code":0,"data":{"items":[{"token":"obbcwkkdw885tetaf82pu184","display_info":"2026 Product <h>Weekly</h> Notes","meta_data":{"app_link":"https://example.feishu.cn/minutes/xxxxxx","avatar":"https://p3-lark-file.byteimg.com/img/xxxx.jpg","description":"Product weekly notes"}}],"has_more":false}}"#,
        )
        .expect("page");

        assert_eq!(page.minutes.len(), 1);
        assert_eq!(page.minutes[0].title, None);
        assert_eq!(page.minutes[0].create_time_ms, None);
        assert_eq!(page.minutes[0].duration_ms, None);
        let debug = format!("{page:?}");
        assert!(!debug.contains("obbcwkkdw885tetaf82pu184"));
        assert!(!debug.contains("feishu.cn/minutes"));
        assert!(!debug.contains("p3-lark-file"));
        assert!(!debug.contains("Product weekly notes"));
    }

    #[test]
    fn maps_minutes_search_error_codes() {
        assert_eq!(
            map_status_or_parse_minute_search(200, r#"{"code":2094002,"msg":"too long"}"#)
                .expect_err("query too long"),
            FeishuMinutesReadError::UpstreamClient
        );
        assert_eq!(
            map_status_or_parse_minute_search(200, r#"{"code":2094011,"msg":"identity"}"#)
                .expect_err("identity"),
            FeishuMinutesReadError::Unauthorized
        );
        assert_eq!(
            map_status_or_parse_minute_search(200, r#"{"code":2095001,"msg":"search"}"#)
                .expect_err("search unavailable"),
            FeishuMinutesReadError::UpstreamTransient
        );
    }
}
