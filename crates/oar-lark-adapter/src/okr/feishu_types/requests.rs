use std::fmt;

use crate::redaction::SecretString;

use super::{FeishuOkrProgressListTarget, OkrDepartmentIdType, OkrUserIdType};

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
