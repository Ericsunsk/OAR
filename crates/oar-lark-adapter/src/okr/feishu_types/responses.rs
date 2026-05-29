use serde::{Deserialize, Serialize};

use super::{
    FeishuOkr, FeishuOkrCycle, FeishuOkrKeyResult, FeishuOkrObjective, FeishuOkrProgressRecord,
};
use crate::okr::parser::deserialize_option_stringish;

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
