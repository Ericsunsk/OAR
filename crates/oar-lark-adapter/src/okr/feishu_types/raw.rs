use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::okr::parser::{
    content_value_to_text, deserialize_option_content_value, deserialize_option_stringish,
};

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

impl FeishuOkrObjective {
    pub fn content_text(&self) -> Option<String> {
        self.content.as_ref().and_then(content_value_to_text)
    }

    pub fn notes_text(&self) -> Option<String> {
        self.notes.as_ref().and_then(content_value_to_text)
    }
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

impl FeishuOkrKeyResult {
    pub fn content_text(&self) -> Option<String> {
        self.content.as_ref().and_then(content_value_to_text)
    }

    pub fn notes_text(&self) -> Option<String> {
        self.notes.as_ref().and_then(content_value_to_text)
    }
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
pub struct FeishuOkrProgressRecord {
    #[serde(
        default,
        alias = "id",
        deserialize_with = "deserialize_option_stringish"
    )]
    pub progress_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_stringish")]
    pub modify_time: Option<String>,
    pub progress_rate: Option<FeishuOkrProgressRate>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrProgressRecordRef {
    pub id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type FeishuOkrItem = FeishuOkr;
