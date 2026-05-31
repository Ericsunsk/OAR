use std::fmt;

use reqwest::Url;

use super::error::FeishuMinutesReadError;

#[derive(Clone, PartialEq, Eq)]
pub struct MinutesSourceRef {
    pub minute_token: String,
}

impl fmt::Debug for MinutesSourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MinutesSourceRef")
            .field("minute_token", &"[REDACTED]")
            .finish()
    }
}

impl MinutesSourceRef {
    pub fn source_ref(&self) -> String {
        format!("minutes://{}", self.minute_token)
    }
}

pub fn parse_minutes_source_ref(
    source_ref: &str,
) -> Result<MinutesSourceRef, FeishuMinutesReadError> {
    let trimmed = source_ref.trim();
    if let Some(minute_token) = trimmed.strip_prefix("minutes://") {
        return minutes_ref(minute_token);
    }
    if let Some(minute_token) = trimmed.strip_prefix("feishu://minutes/") {
        return minutes_ref(minute_token);
    }
    parse_minutes_url(trimmed).ok_or(FeishuMinutesReadError::InvalidSourceRef)
}

fn parse_minutes_url(value: &str) -> Option<MinutesSourceRef> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    if !is_supported_minutes_host(url.host_str()?) {
        return None;
    }
    let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
    if segments.next()? != "minutes" {
        return None;
    }
    let minute_token = segments.next()?;
    if segments.next().is_some() {
        return None;
    }
    minutes_ref(minute_token).ok()
}

fn is_supported_minutes_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "feishu.cn"
        || host.ends_with(".feishu.cn")
        || host == "larksuite.com"
        || host.ends_with(".larksuite.com")
}

fn minutes_ref(minute_token: &str) -> Result<MinutesSourceRef, FeishuMinutesReadError> {
    let minute_token = minute_token.trim();
    if !valid_minute_token(minute_token) {
        return Err(FeishuMinutesReadError::InvalidSourceRef);
    }
    Ok(MinutesSourceRef {
        minute_token: minute_token.to_string(),
    })
}

pub(super) fn valid_minute_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes_refs_and_urls() {
        let direct =
            parse_minutes_source_ref(" minutes://obcnq3b9jl72l83w4f14xxxx ").expect("minutes ref");
        assert_eq!(direct.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");

        let feishu = parse_minutes_source_ref("feishu://minutes/obcnq3b9jl72l83w4f14xxxx")
            .expect("feishu ref");
        assert_eq!(feishu.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");

        let url = parse_minutes_source_ref(
            "https://sample.feishu.cn/minutes/obcnq3b9jl72l83w4f14xxxx?from=copy",
        )
        .expect("minutes url");
        assert_eq!(url.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");

        let lark_url = parse_minutes_source_ref(
            "https://sample.larksuite.com/minutes/obcnq3b9jl72l83w4f14xxxx",
        )
        .expect("lark minutes url");
        assert_eq!(lark_url.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");
    }

    #[test]
    fn rejects_unsafe_or_unsupported_refs() {
        assert_eq!(
            parse_minutes_source_ref("minutes://enterprise-weekly-sync"),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref("minutes://Obcnq3b9jl72l83w4f14xxxx"),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref("minutes://obcnq3b9jl72l83w4f14xxxx/child"),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref(
                "https://sample.feishu.cn/foo/minutes/obcnq3b9jl72l83w4f14xxxx"
            ),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref(
                "https://sample.feishu.cn/minutes/obcnq3b9jl72l83w4f14xxxx/child"
            ),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref("https://example.com/minutes/obcnq3b9jl72l83w4f14xxxx"),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_minutes_source_ref("http://sample.feishu.cn/minutes/obcnq3b9jl72l83w4f14xxxx"),
            Err(FeishuMinutesReadError::InvalidSourceRef)
        );
    }

    #[test]
    fn debug_redacts_minute_token() {
        let source_ref =
            parse_minutes_source_ref("minutes://obcnq3b9jl72l83w4f14xxxx").expect("minutes");

        let debug = format!("{source_ref:?}");

        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("obcnq3b9"));
    }
}
