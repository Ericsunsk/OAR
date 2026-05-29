#![forbid(unsafe_code)]

#[cfg(feature = "postgres")]
pub mod audit_outbox;
pub mod calendar;
pub mod config;
pub mod credentials;
pub mod crypto;
pub mod error;
pub mod factory;
pub mod material;
pub mod oauth;
pub mod okr;
#[cfg(feature = "postgres")]
pub mod postgres;
pub mod redaction;
pub mod task;

#[cfg(feature = "postgres")]
pub use audit_outbox::{
    sink_unavailable_error, AuditOutboxDeliveryEnvelope, AuditOutboxSafePayload, AuditOutboxSink,
    AuditOutboxSinkDelivery, AuditOutboxSinkDispatcher, AuditOutboxSinkError, NoopAuditOutboxSink,
};
pub use calendar::{
    build_free_busy_batch_request, AsyncFeishuCalendarRead, CalendarFreeBusyBatchRequest,
    CalendarFreeBusyItem, CalendarFreeBusyList, CalendarFreeBusyPage, CalendarUserIdType,
    FeishuCalendarReadClient, FeishuCalendarReadError,
};
pub use config::FeishuOpenApiConfig;
pub use credentials::{
    AsyncFeishuAppCredentialProvider, FeishuAppCredential, FeishuAppCredentialProvider,
    StaticFeishuAppCredentialProvider,
};
pub use crypto::{
    AesGcmGrantEncryptor, AesGcmGrantEncryptorError, GrantTimeSource, SystemGrantClock,
};
pub use error::{
    classify_feishu_refresh_failure, safe_error_for_failure_class, FeishuRefreshFailureClass,
};
pub use factory::{
    build_async_reqwest_feishu_auth_refresh_adapter, build_feishu_auth_refresh_adapter,
    build_reqwest_feishu_auth_refresh_adapter, FeishuAuthRefreshAdapter,
    FeishuAuthRefreshAdapterBuildError, PostgresFeishuAuthRefreshEnvConfig,
    PostgresFeishuAuthRefreshEnvConfigError, StaticAesGcmKeyResolver, StaticAesGcmKeyResolverError,
};
#[cfg(feature = "postgres")]
pub use factory::{
    build_postgres_async_feishu_auth_refresh_adapter,
    build_postgres_feishu_auth_refresh_adapter_with_http, PostgresAsyncFeishuAuthRefreshAdapter,
    PostgresFeishuAuthRefreshAdapter, PostgresFeishuAuthRefreshMaterialProvider,
};
pub use material::{
    AesGcmKeyResolver, AesGcmRefreshMaterialProvider, AesGcmRefreshMaterialProviderError,
    AsyncAesGcmKeyResolver, AsyncFeishuGrantMaterialStore, DecryptedFeishuGrantMaterial,
    FeishuGrantMaterialStore, FeishuStoredRefreshMaterialProvider,
    FeishuStoredRefreshMaterialProviderError, StoredFeishuGrantMaterial,
};
pub use oauth::{
    AsyncFeishuOAuthLogin, AsyncFeishuRefreshMaterialProvider, AsyncHttpClient,
    FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuGrantEnvelope, FeishuOAuthLogin,
    FeishuOAuthLoginClient, FeishuOAuthLoginConfig, FeishuOAuthLoginConfigError,
    FeishuOAuthLoginError, FeishuOAuthLoginToken, FeishuOAuthLoginUser, FeishuOAuthTransport,
    FeishuOAuthTransportError, FeishuRefreshMaterial, FeishuRefreshMaterialProvider, HttpClient,
    HttpResponse, ReqwestAsyncHttpClient, ReqwestBlockingHttpClient,
};
pub use okr::{
    build_batch_get_okr_request, build_list_cycle_objectives_request, build_list_cycles_request,
    build_list_objective_key_results_request, build_progress_list_request,
    plan_okr_review_inbox_sync, AsyncFeishuOkrRead, FeishuOkr, FeishuOkrBatchGetData,
    FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse, FeishuOkrCycle, FeishuOkrCycleListData,
    FeishuOkrCycleListRequest, FeishuOkrCycleListResponse, FeishuOkrCycleObjectivesListData,
    FeishuOkrCycleObjectivesListRequest, FeishuOkrCycleObjectivesListResponse, FeishuOkrItem,
    FeishuOkrKeyResult, FeishuOkrObjective, FeishuOkrObjectiveKeyResultsListData,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrObjectiveKeyResultsListResponse,
    FeishuOkrProgressRate, FeishuOkrProgressRecordRef, FeishuOkrReadClient, FeishuOkrReadError,
    OkrProgressListRequest, OkrReadCycle, OkrReadCyclesPage, OkrReadKeyResult,
    OkrReadKeyResultsPage, OkrReadObjective, OkrReadObjectivesPage, OkrReadOkr, OkrReadSnapshot,
    OkrReviewInboxPlan, OkrReviewInboxPlanError, OkrReviewInboxPlanInput, OkrUserIdType,
};
#[cfg(feature = "postgres")]
pub use postgres::{PostgresFeishuGrantMaterialStore, PostgresFeishuGrantMaterialStoreError};
pub use redaction::SecretString;
pub use task::{
    build_get_task_request, build_list_tasks_request, parse_task_source_ref, AsyncFeishuTaskRead,
    FeishuTaskGetRequest, FeishuTaskListRequest, FeishuTaskReadClient, FeishuTaskReadError,
    TaskListType, TaskReadDue, TaskReadOwner, TaskReadPage, TaskReadSummary, TaskSourceRef,
    TaskUserIdType,
};
