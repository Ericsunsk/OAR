mod client;
mod error;
mod review_inbox;
mod types;

pub use client::{
    build_batch_get_okr_request, build_progress_list_request, AsyncFeishuOkrRead,
    FeishuOkrReadClient, OkrProgressListRequest,
};
pub use error::FeishuOkrReadError;
pub use review_inbox::{
    plan_okr_review_inbox_sync, OkrReviewInboxPlan, OkrReviewInboxPlanError,
    OkrReviewInboxPlanInput,
};
pub use types::{
    FeishuOkr, FeishuOkrBatchGetData, FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse,
    FeishuOkrItem, FeishuOkrKeyResult, FeishuOkrObjective, FeishuOkrProgressRate,
    FeishuOkrProgressRecordRef, OkrReadKeyResult, OkrReadObjective, OkrReadOkr, OkrReadSnapshot,
    OkrUserIdType,
};

#[cfg(test)]
mod tests;
