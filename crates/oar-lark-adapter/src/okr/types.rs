use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

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
    pub okr_list: Vec<FeishuOkrItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeishuOkrItem {
    #[serde(flatten)]
    pub fields: BTreeMap<String, serde_json::Value>,
}
