use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::parser::{
    content_value_to_text, deserialize_option_content_value, deserialize_option_stringish,
};
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

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOkrProgressListRequest {
    pub user_access_token: SecretString,
    pub user_id_type: OkrUserIdType,
    pub target: FeishuOkrProgressListTarget,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub department_id_type: OkrDepartmentIdType,
}

impl FeishuOkrProgressListRequest {
    pub fn new(
        user_access_token: SecretString,
        target: FeishuOkrProgressListTarget,
    ) -> FeishuOkrProgressListRequest {
        FeishuOkrProgressListRequest {
            user_access_token,
            user_id_type: OkrUserIdType::OpenId,
            target,
            page_size: Some(100),
            page_token: None,
            department_id_type: OkrDepartmentIdType::OpenDepartmentId,
        }
    }
}

impl fmt::Debug for FeishuOkrProgressListRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOkrProgressListRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_id_type", &self.user_id_type)
            .field("target", &self.target)
            .field("page_size", &self.page_size)
            .field("page_token", &self.page_token)
            .field("department_id_type", &self.department_id_type)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuOkrProgressListTarget {
    Objective(String),
    KeyResult(String),
}

impl FeishuOkrProgressListTarget {
    pub fn id(&self) -> &str {
        match self {
            FeishuOkrProgressListTarget::Objective(id)
            | FeishuOkrProgressListTarget::KeyResult(id) => id,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkrDepartmentIdType {
    OpenDepartmentId,
    DepartmentId,
}

impl OkrDepartmentIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OkrDepartmentIdType::OpenDepartmentId => "open_department_id",
            OkrDepartmentIdType::DepartmentId => "department_id",
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
pub struct FeishuOkrProgressListResponse {
    pub code: i64,
    pub msg: Option<String>,
    pub data: Option<FeishuOkrProgressListData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrProgressListData {
    #[serde(default)]
    pub progress_list: Vec<FeishuOkrProgressRecord>,
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
