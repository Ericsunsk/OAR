mod client;
mod error;
mod review_inbox;
mod types;

pub use client::{
    build_batch_get_okr_request, build_list_cycle_objectives_request, build_list_cycles_request,
    build_list_objective_key_results_request, build_progress_list_request, AsyncFeishuOkrRead,
    FeishuOkrReadClient,
};
pub use error::FeishuOkrReadError;
pub use review_inbox::{
    plan_okr_review_inbox_sync, OkrReviewInboxPlan, OkrReviewInboxPlanError,
    OkrReviewInboxPlanInput,
};
pub use types::{
    FeishuOkr, FeishuOkrBatchGetData, FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse,
    FeishuOkrCycle, FeishuOkrCycleListData, FeishuOkrCycleListRequest, FeishuOkrCycleListResponse,
    FeishuOkrCycleObjectivesListData, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrCycleObjectivesListResponse, FeishuOkrItem, FeishuOkrKeyResult, FeishuOkrObjective,
    FeishuOkrObjectiveKeyResultsListData, FeishuOkrObjectiveKeyResultsListRequest,
    FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrProgressListData,
    FeishuOkrProgressListRequest, FeishuOkrProgressListResponse, FeishuOkrProgressListTarget,
    FeishuOkrProgressRate, FeishuOkrProgressRecord, FeishuOkrProgressRecordRef,
    OkrDepartmentIdType, OkrReadCycle, OkrReadCyclesPage, OkrReadKeyResult, OkrReadKeyResultsPage,
    OkrReadObjective, OkrReadObjectivesPage, OkrReadOkr, OkrReadProgressPage,
    OkrReadProgressRecord, OkrReadSnapshot, OkrUserIdType,
};

#[cfg(test)]
mod tests;
