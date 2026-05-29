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

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOkrCycleListRequest {
    pub user_access_token: SecretString,
    pub user_id_type: OkrUserIdType,
    pub user_id: String,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub lang: Option<String>,
}

impl fmt::Debug for FeishuOkrCycleListRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOkrCycleListRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_id_type", &self.user_id_type)
            .field("user_id", &self.user_id)
            .field("page_size", &self.page_size)
            .field("page_token", &self.page_token)
            .field("lang", &self.lang)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOkrCycleObjectivesListRequest {
    pub user_access_token: SecretString,
    pub user_id_type: OkrUserIdType,
    pub cycle_id: String,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub lang: Option<String>,
}

impl fmt::Debug for FeishuOkrCycleObjectivesListRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOkrCycleObjectivesListRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_id_type", &self.user_id_type)
            .field("cycle_id", &self.cycle_id)
            .field("page_size", &self.page_size)
            .field("page_token", &self.page_token)
            .field("lang", &self.lang)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOkrObjectiveKeyResultsListRequest {
    pub user_access_token: SecretString,
    pub user_id_type: OkrUserIdType,
    pub objective_id: String,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub lang: Option<String>,
}

impl fmt::Debug for FeishuOkrObjectiveKeyResultsListRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOkrObjectiveKeyResultsListRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_id_type", &self.user_id_type)
            .field("objective_id", &self.objective_id)
            .field("page_size", &self.page_size)
            .field("page_token", &self.page_token)
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
pub struct FeishuOkrCycleListResponse {
    pub code: i64,
    pub msg: Option<String>,
    pub data: Option<FeishuOkrCycleListData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrCycleListData {
    #[serde(default, alias = "cycles", alias = "cycle_list")]
    pub items: Vec<FeishuOkrCycle>,
    #[serde(
        default,
        alias = "next_page_token",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub page_token: Option<String>,
    #[serde(default)]
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrCycleObjectivesListResponse {
    pub code: i64,
    pub msg: Option<String>,
    pub data: Option<FeishuOkrCycleObjectivesListData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrCycleObjectivesListData {
    #[serde(default, alias = "objectives", alias = "objective_list")]
    pub items: Vec<FeishuOkrObjective>,
    #[serde(
        default,
        alias = "next_page_token",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub page_token: Option<String>,
    #[serde(default)]
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrObjectiveKeyResultsListResponse {
    pub code: i64,
    pub msg: Option<String>,
    pub data: Option<FeishuOkrObjectiveKeyResultsListData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrObjectiveKeyResultsListData {
    #[serde(
        default,
        alias = "key_results",
        alias = "key_result_list",
        alias = "kr_list"
    )]
    pub items: Vec<FeishuOkrKeyResult>,
    #[serde(
        default,
        alias = "next_page_token",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub page_token: Option<String>,
    #[serde(default)]
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrCycle {
    #[serde(
        default,
        alias = "cycle_id",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub id: Option<String>,
    #[serde(alias = "title")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub start_time: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub end_time: Option<String>,
    #[serde(
        default,
        alias = "cycle_status",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    #[serde(default, deserialize_with = "deserialize_option_content_value")]
    pub content: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_option_content_value")]
    pub notes: Option<Value>,
    pub progress_report: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub score: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub weight: Option<String>,
    pub progress_rate: Option<FeishuOkrProgressRate>,
    #[serde(default)]
    #[serde(alias = "key_results")]
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
    #[serde(default, deserialize_with = "deserialize_option_content_value")]
    pub content: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_option_content_value")]
    pub notes: Option<Value>,
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
    #[serde(
        default,
        alias = "cycle_status",
        deserialize_with = "deserialize_option_stringish"
    )]
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
pub struct OkrReadCyclesPage {
    #[serde(default)]
    pub cycles: Vec<OkrReadCycle>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadCycle {
    pub cycle_id: Option<String>,
    pub name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadObjectivesPage {
    pub cycle_id: String,
    #[serde(default)]
    pub objectives: Vec<OkrReadObjective>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadKeyResultsPage {
    pub objective_id: String,
    #[serde(default)]
    pub krs: Vec<OkrReadKeyResult>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
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

impl OkrReadCyclesPage {
    pub fn from_cycle_list_data(data: &FeishuOkrCycleListData) -> Self {
        Self {
            cycles: data.items.iter().map(OkrReadCycle::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl OkrReadObjectivesPage {
    pub fn from_cycle_objectives_list_data(
        cycle_id: impl Into<String>,
        data: &FeishuOkrCycleObjectivesListData,
    ) -> Self {
        Self {
            cycle_id: cycle_id.into(),
            objectives: data.items.iter().map(OkrReadObjective::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl OkrReadKeyResultsPage {
    pub fn from_objective_key_results_list_data(
        objective_id: impl Into<String>,
        data: &FeishuOkrObjectiveKeyResultsListData,
    ) -> Self {
        Self {
            objective_id: objective_id.into(),
            krs: data.items.iter().map(OkrReadKeyResult::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl From<&FeishuOkrCycle> for OkrReadCycle {
    fn from(value: &FeishuOkrCycle) -> Self {
        Self {
            cycle_id: value.id.clone(),
            name: value.name.clone().and_then(non_empty),
            start_time: value.start_time.clone(),
            end_time: value.end_time.clone(),
            status: value.status.clone(),
        }
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
            content: value.content.as_ref().and_then(content_value_to_text),
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
            content: value.content.as_ref().and_then(content_value_to_text),
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

impl FeishuOkrObjective {
    pub fn content_text(&self) -> Option<String> {
        self.content.as_ref().and_then(content_value_to_text)
    }

    pub fn notes_text(&self) -> Option<String> {
        self.notes.as_ref().and_then(content_value_to_text)
    }
}

impl FeishuOkrKeyResult {
    pub fn content_text(&self) -> Option<String> {
        self.content.as_ref().and_then(content_value_to_text)
    }

    pub fn notes_text(&self) -> Option<String> {
        self.notes.as_ref().and_then(content_value_to_text)
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

fn deserialize_option_content_value<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(normalize_content_value))
}

fn normalize_content_value(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                Some(parsed)
            } else {
                Some(Value::String(trimmed.to_string()))
            }
        }
        other => Some(other),
    }
}

fn stringify_json_scalar(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => non_empty(v),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn content_value_to_text(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    collect_content_text(value, 0, &mut parts);
    if parts.is_empty() {
        return stringify_json_scalar(value.clone());
    }
    non_empty(parts.join("\n")).map(limit_text)
}

fn collect_content_text(value: &Value, depth: usize, parts: &mut Vec<String>) {
    const MAX_CONTENT_DEPTH: usize = 8;
    const MAX_CONTENT_PARTS: usize = 64;
    if depth > MAX_CONTENT_DEPTH || parts.len() >= MAX_CONTENT_PARTS {
        return;
    }

    match value {
        Value::Null => {}
        Value::String(value) => {
            if let Some(value) = non_empty(value.clone()) {
                parts.push(value);
            }
        }
        Value::Number(value) => parts.push(value.to_string()),
        Value::Bool(value) => parts.push(value.to_string()),
        Value::Array(values) => {
            for value in values
                .iter()
                .take(MAX_CONTENT_PARTS.saturating_sub(parts.len()))
            {
                collect_content_text(value, depth + 1, parts);
                if parts.len() >= MAX_CONTENT_PARTS {
                    break;
                }
            }
        }
        Value::Object(map) => {
            for field in [
                "text",
                "plain_text",
                "content",
                "title",
                "name",
                "value",
                "text_run",
                "paragraph",
                "elements",
                "children",
                "blocks",
            ] {
                if let Some(value) = map.get(field) {
                    collect_content_text(value, depth + 1, parts);
                    if parts.len() >= MAX_CONTENT_PARTS {
                        break;
                    }
                }
            }
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn limit_text(value: String) -> String {
    const MAX_CONTENT_TEXT_CHARS: usize = 4096;
    if value.chars().count() <= MAX_CONTENT_TEXT_CHARS {
        return value;
    }
    let mut truncated = value
        .chars()
        .take(MAX_CONTENT_TEXT_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
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
