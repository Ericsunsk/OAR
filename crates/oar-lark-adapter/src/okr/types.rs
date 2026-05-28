use std::collections::BTreeMap;
use std::fmt;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOkrBatchGetRequest {
    pub user_access_token: SecretString,
    pub user_id_type: OkrUserIdType,
    pub okr_ids: Vec<String>,
    pub lang: Option<String>,
}

impl fmt::Debug for FeishuOkrBatchGetRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOkrBatchGetRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_id_type", &self.user_id_type)
            .field("okr_ids", &self.okr_ids)
            .field("lang", &self.lang)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkrUserIdType {
    OpenId,
    UserId,
    UnionId,
}

impl OkrUserIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OkrUserIdType::OpenId => "open_id",
            OkrUserIdType::UserId => "user_id",
            OkrUserIdType::UnionId => "union_id",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrBatchGetResponse {
    pub code: i64,
    pub msg: Option<String>,
    pub data: Option<FeishuOkrBatchGetData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrBatchGetData {
    #[serde(default, rename = "okr_list")]
    pub okr_list: Vec<FeishuOkr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkr {
    #[serde(alias = "okr_id")]
    pub id: Option<String>,
    pub period_id: Option<String>,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub permission: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub confirm_status: Option<String>,
    #[serde(default)]
    pub objective_list: Vec<FeishuOkrObjective>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrObjective {
    #[serde(alias = "objective_id")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub permission: Option<String>,
    pub content: Option<String>,
    pub progress_report: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub score: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub weight: Option<String>,
    pub progress_rate: Option<FeishuOkrProgressRate>,
    #[serde(default)]
    pub kr_list: Vec<FeishuOkrKeyResult>,
    #[serde(default)]
    pub progress_record_list: Vec<FeishuOkrProgressRecordRef>,
    pub last_updated_time: Option<String>,
    pub progress_rate_percent_last_updated_time: Option<String>,
    pub progress_rate_status_last_updated_time: Option<String>,
    pub progress_record_last_updated_time: Option<String>,
    pub progress_report_last_updated_time: Option<String>,
    pub score_last_updated_time: Option<String>,
    pub deadline: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrKeyResult {
    #[serde(alias = "kr_id")]
    pub id: Option<String>,
    pub content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub score: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub kr_weight: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub weight: Option<String>,
    pub progress_rate: Option<FeishuOkrProgressRate>,
    #[serde(default)]
    pub progress_record_list: Vec<FeishuOkrProgressRecordRef>,
    pub last_updated_time: Option<String>,
    pub progress_rate_percent_last_updated_time: Option<String>,
    pub progress_rate_status_last_updated_time: Option<String>,
    pub progress_record_last_updated_time: Option<String>,
    pub progress_report_last_updated_time: Option<String>,
    pub score_last_updated_time: Option<String>,
    pub deadline: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrProgressRate {
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub percent: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrProgressRecordRef {
    pub id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadSnapshot {
    #[serde(default)]
    pub okrs: Vec<OkrReadOkr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadOkr {
    pub okr_id: Option<String>,
    pub period_id: Option<String>,
    pub okr_name: Option<String>,
    #[serde(default)]
    pub objectives: Vec<OkrReadObjective>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadObjective {
    pub objective_id: Option<String>,
    pub content: Option<String>,
    pub progress: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub progress_record_ids: Vec<String>,
    pub deadline: Option<String>,
    pub last_updated_time: Option<String>,
    #[serde(default)]
    pub krs: Vec<OkrReadKeyResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadKeyResult {
    pub kr_id: Option<String>,
    pub content: Option<String>,
    pub progress: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub progress_record_ids: Vec<String>,
    pub deadline: Option<String>,
    pub last_updated_time: Option<String>,
}

impl OkrReadSnapshot {
    pub fn from_batch_get_data(data: &FeishuOkrBatchGetData) -> Self {
        let okrs = data.okr_list.iter().map(OkrReadOkr::from).collect();
        Self { okrs }
    }
}

impl From<&FeishuOkr> for OkrReadOkr {
    fn from(value: &FeishuOkr) -> Self {
        Self {
            okr_id: value.id.clone(),
            period_id: value.period_id.clone(),
            okr_name: value.name.clone(),
            objectives: value
                .objective_list
                .iter()
                .map(OkrReadObjective::from)
                .collect(),
        }
    }
}

impl From<&FeishuOkrObjective> for OkrReadObjective {
    fn from(value: &FeishuOkrObjective) -> Self {
        Self {
            objective_id: value.id.clone(),
            content: value.content.clone(),
            progress: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.percent.clone()),
            status: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.status.clone()),
            progress_record_ids: collect_progress_record_ids(&value.progress_record_list),
            deadline: value.deadline.clone(),
            last_updated_time: latest_updated_time(&[
                value.last_updated_time.as_deref(),
                value.progress_rate_percent_last_updated_time.as_deref(),
                value.progress_rate_status_last_updated_time.as_deref(),
                value.progress_record_last_updated_time.as_deref(),
                value.progress_report_last_updated_time.as_deref(),
                value.score_last_updated_time.as_deref(),
            ]),
            krs: value.kr_list.iter().map(OkrReadKeyResult::from).collect(),
        }
    }
}

impl From<&FeishuOkrKeyResult> for OkrReadKeyResult {
    fn from(value: &FeishuOkrKeyResult) -> Self {
        Self {
            kr_id: value.id.clone(),
            content: value.content.clone(),
            progress: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.percent.clone()),
            status: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.status.clone()),
            progress_record_ids: collect_progress_record_ids(&value.progress_record_list),
            deadline: value.deadline.clone(),
            last_updated_time: latest_updated_time(&[
                value.last_updated_time.as_deref(),
                value.progress_rate_percent_last_updated_time.as_deref(),
                value.progress_rate_status_last_updated_time.as_deref(),
                value.progress_record_last_updated_time.as_deref(),
                value.progress_report_last_updated_time.as_deref(),
                value.score_last_updated_time.as_deref(),
            ]),
        }
    }
}

fn collect_progress_record_ids(records: &[FeishuOkrProgressRecordRef]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| record.id.clone())
        .collect()
}

pub type FeishuOkrItem = FeishuOkr;

fn deserialize_option_stringish<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(stringify_json_scalar))
}

fn stringify_json_scalar(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => Some(v),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn latest_updated_time(values: &[Option<&str>]) -> Option<String> {
    let numeric_latest = values
        .iter()
        .flatten()
        .filter_map(|value| {
            let trimmed = value.trim();
            trimmed.parse::<u64>().ok().map(|parsed| (parsed, trimmed))
        })
        .max_by_key(|(parsed, _)| *parsed)
        .map(|(_, raw)| raw.to_string());

    numeric_latest.or_else(|| {
        values
            .iter()
            .flatten()
            .map(|value| value.trim())
            .find(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}
