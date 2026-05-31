use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteGetResponse {
    pub(super) code: i64,
    pub(super) data: Option<FeishuMinuteGetData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinuteGetData {
    pub(super) minute: Option<FeishuMinute>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuMinute {
    pub(super) title: Option<String>,
    pub(super) create_time: Option<String>,
    pub(super) duration: Option<String>,
}
