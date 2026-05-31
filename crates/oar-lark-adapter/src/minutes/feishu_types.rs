use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteGetResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuMinuteGetData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteSearchResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuMinuteSearchData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteGetData {
    pub(super) minute: Option<FeishuMinute>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteSearchData {
    #[serde(default, alias = "minutes")]
    pub(super) items: Vec<FeishuMinute>,
    #[serde(default)]
    pub(super) has_more: bool,
    pub(super) page_token: Option<String>,
    pub(super) total: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinute {
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(
        default,
        alias = "create_time_ms",
        alias = "start_time",
        alias = "start_time_ms"
    )]
    pub(super) create_time: Option<String>,
    #[serde(default, alias = "duration_ms")]
    pub(super) duration: Option<String>,
}
