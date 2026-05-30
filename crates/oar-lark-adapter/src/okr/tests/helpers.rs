use crate::okr::{
    FeishuOkrBatchGetRequest, FeishuOkrCycleListRequest, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrProgressListRequest,
    FeishuOkrProgressListTarget, OkrDepartmentIdType, OkrUserIdType,
};
use crate::redaction::SecretString;
pub(super) use crate::test_support::http::{AsyncFakeHttpClient, FakeHttpClient};

pub(super) fn sample_request() -> FeishuOkrBatchGetRequest {
    FeishuOkrBatchGetRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        okr_ids: vec!["okr_1".to_string(), "okr_2".to_string()],
        lang: Some("zh_cn".to_string()),
    }
}

pub(super) fn sample_cycle_list_request() -> FeishuOkrCycleListRequest {
    FeishuOkrCycleListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        user_id: "ou_user_1".to_string(),
        page_size: Some(100),
        page_token: Some("next token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

pub(super) fn sample_cycle_objectives_request() -> FeishuOkrCycleObjectivesListRequest {
    FeishuOkrCycleObjectivesListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        cycle_id: "cycle 2026/05".to_string(),
        page_size: Some(100),
        page_token: Some("objective token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

pub(super) fn sample_objective_key_results_request() -> FeishuOkrObjectiveKeyResultsListRequest {
    FeishuOkrObjectiveKeyResultsListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        objective_id: "obj/1?x".to_string(),
        page_size: Some(100),
        page_token: Some("kr token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

pub(super) fn sample_progress_list_request(
    target: FeishuOkrProgressListTarget,
) -> FeishuOkrProgressListRequest {
    FeishuOkrProgressListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        target,
        page_size: None,
        page_token: Some("progress token/1".to_string()),
        department_id_type: OkrDepartmentIdType::OpenDepartmentId,
    }
}
