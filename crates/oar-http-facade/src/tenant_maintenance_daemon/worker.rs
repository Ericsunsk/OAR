use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use oar_core::storage::postgres::{
    postgres_repository_safe_error_reason, PostgresRepositoryError,
    PostgresTenantMaintenanceConfigValidationError, PostgresTenantMaintenanceReport,
    PostgresTenantMaintenanceWorker,
};
use oar_lark_adapter::{
    build_postgres_async_feishu_auth_refresh_adapter, AuditOutboxSinkDispatcher,
    FeishuAuthRefreshAdapterBuildError, PostgresAsyncFeishuAuthRefreshAdapter,
    ReqwestAsyncHttpClient, WebhookAuditOutboxSink, WebhookAuditOutboxSinkConfigError,
};
use oar_runtime::{
    RuntimeTenantTickFactory, RuntimeTenantTickFactoryFuture, RuntimeTick, RuntimeTickFuture,
    TenantMaintenanceRuntimeConfigValidationError,
};

use super::TenantMaintenanceDaemonStartError;
use crate::tenant_maintenance::{
    TenantMaintenanceAuditOutboxSinkSettings, TenantMaintenanceSettingsError,
    TenantMaintenanceWorkerSettings,
};
use crate::tenant_maintenance_daemon_failure::TenantMaintenanceDaemonFailureCode;
use crate::tenant_maintenance_daemon_status::TenantMaintenanceDaemonStatusHandle;

type FacadeAuditOutboxDispatcher =
    AuditOutboxSinkDispatcher<WebhookAuditOutboxSink<ReqwestAsyncHttpClient>>;
type FacadeTenantMaintenanceCoreWorker = PostgresTenantMaintenanceWorker<
    PostgresAsyncFeishuAuthRefreshAdapter,
    FacadeAuditOutboxDispatcher,
    fn() -> u64,
>;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct TenantMaintenanceTickError {
    safe_error: String,
}

impl fmt::Debug for TenantMaintenanceTickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantMaintenanceTickError")
            .field("safe_error", &self.safe_error)
            .finish()
    }
}

impl fmt::Display for TenantMaintenanceTickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_error)
    }
}

impl Error for TenantMaintenanceTickError {}

pub(super) struct FacadeTenantMaintenanceTick {
    worker: FacadeTenantMaintenanceCoreWorker,
    status: TenantMaintenanceDaemonStatusHandle,
}

impl RuntimeTick for FacadeTenantMaintenanceTick {
    type Report = PostgresTenantMaintenanceReport;
    type Error = TenantMaintenanceTickError;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error> {
        Box::pin(async move {
            let report = self
                .worker
                .run_once()
                .await
                .map_err(tick_repository_error)?;
            self.status.record_tenant_report(&report);
            if let Some(safe_error) = report_stage_safe_error(&report) {
                return Err(TenantMaintenanceTickError { safe_error });
            }
            Ok(report)
        })
    }

    fn safe_error(error: &Self::Error) -> String {
        error.safe_error.clone()
    }
}

pub(super) struct FacadeTenantMaintenanceTickFactory {
    pub(super) pool: sqlx::PgPool,
    pub(super) worker_settings: TenantMaintenanceWorkerSettings,
    pub(super) audit_outbox_sink: TenantMaintenanceAuditOutboxSinkSettings,
    pub(super) feishu_login: std::sync::Arc<crate::feishu_auth::FeishuLoginRuntime>,
    pub(super) persistence: crate::persistence::FacadePersistenceRuntime,
    pub(super) status: TenantMaintenanceDaemonStatusHandle,
}

impl fmt::Debug for FacadeTenantMaintenanceTickFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FacadeTenantMaintenanceTickFactory")
            .field("pool", &"[REDACTED]")
            .field("worker_settings", &"[REDACTED]")
            .field("audit_outbox_sink", &"[REDACTED]")
            .field("feishu_login", &"[REDACTED]")
            .field("persistence", &"[REDACTED]")
            .field("status", &self.status.snapshot().state)
            .finish()
    }
}

impl RuntimeTenantTickFactory<FacadeTenantMaintenanceTick> for FacadeTenantMaintenanceTickFactory {
    type Error = TenantMaintenanceDaemonStartError;

    fn build_tick(
        &mut self,
        tenant_id: &str,
    ) -> RuntimeTenantTickFactoryFuture<'_, FacadeTenantMaintenanceTick, Self::Error> {
        let tenant_id = tenant_id.to_string();
        let pool = self.pool.clone();
        let worker_settings = self.worker_settings.clone();
        let audit_outbox_sink = self.audit_outbox_sink.clone();
        let feishu_login = self.feishu_login.clone();
        let persistence = self.persistence.clone();
        let status = self.status.clone();
        Box::pin(async move {
            build_tenant_maintenance_worker(
                pool,
                worker_settings,
                audit_outbox_sink,
                feishu_login,
                persistence,
                status,
                &tenant_id,
            )
        })
    }

    fn safe_error(error: &Self::Error) -> String {
        error.to_string()
    }
}

fn build_tenant_maintenance_worker(
    pool: sqlx::PgPool,
    worker_settings: TenantMaintenanceWorkerSettings,
    audit_outbox_sink: TenantMaintenanceAuditOutboxSinkSettings,
    feishu_login: std::sync::Arc<crate::feishu_auth::FeishuLoginRuntime>,
    persistence: crate::persistence::FacadePersistenceRuntime,
    status: TenantMaintenanceDaemonStatusHandle,
    tenant_id: &str,
) -> Result<FacadeTenantMaintenanceTick, TenantMaintenanceDaemonStartError> {
    let open_api_config = feishu_login.open_api_config();
    let refresh_adapter = build_postgres_async_feishu_auth_refresh_adapter(
        pool.clone(),
        open_api_config.clone(),
        feishu_login.client_id().to_string(),
        feishu_login.client_secret(),
        persistence.grant_key_id().to_string(),
        persistence.grant_key_material(),
    )
    .map_err(refresh_adapter_build_error)?;

    let TenantMaintenanceAuditOutboxSinkSettings::Webhook { endpoint } = audit_outbox_sink;
    let webhook_http_client = ReqwestAsyncHttpClient::with_config(&open_api_config)
        .map_err(|_| TenantMaintenanceDaemonStartError::WebhookSinkBuildFailed)?;
    let webhook_sink = WebhookAuditOutboxSink::new(endpoint, webhook_http_client)
        .map_err(webhook_sink_build_error)?;
    let outbox_dispatcher = AuditOutboxSinkDispatcher::new(webhook_sink);
    let worker_config = worker_settings
        .config_for_tenant(tenant_id, system_time_ms())
        .map_err(worker_settings_error)?;
    let worker = PostgresTenantMaintenanceWorker::try_new(
        pool,
        refresh_adapter,
        outbox_dispatcher,
        system_time_ms as fn() -> u64,
        worker_config,
    )
    .map_err(worker_config_error)?;
    Ok(FacadeTenantMaintenanceTick { worker, status })
}

fn system_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(super) fn daemon_runtime_config_error(
    _error: TenantMaintenanceRuntimeConfigValidationError,
) -> TenantMaintenanceDaemonStartError {
    TenantMaintenanceDaemonStartError::InvalidRuntimeConfig
}

fn refresh_adapter_build_error(
    _error: FeishuAuthRefreshAdapterBuildError,
) -> TenantMaintenanceDaemonStartError {
    TenantMaintenanceDaemonStartError::RefreshAdapterBuildFailed
}

fn webhook_sink_build_error(
    _error: WebhookAuditOutboxSinkConfigError,
) -> TenantMaintenanceDaemonStartError {
    TenantMaintenanceDaemonStartError::WebhookSinkBuildFailed
}

fn worker_config_error(
    _error: PostgresTenantMaintenanceConfigValidationError,
) -> TenantMaintenanceDaemonStartError {
    TenantMaintenanceDaemonStartError::InvalidWorkerConfig
}

fn worker_settings_error(
    _error: TenantMaintenanceSettingsError,
) -> TenantMaintenanceDaemonStartError {
    TenantMaintenanceDaemonStartError::InvalidWorkerConfig
}

fn tick_repository_error(error: PostgresRepositoryError) -> TenantMaintenanceTickError {
    TenantMaintenanceTickError {
        safe_error: TenantMaintenanceDaemonFailureCode::runtime_tick_safe_error(
            postgres_repository_safe_error_reason(&error),
        ),
    }
}

pub(super) fn report_stage_safe_error(report: &PostgresTenantMaintenanceReport) -> Option<String> {
    report
        .scheduled_sweep
        .failed()
        .map(|_| TenantMaintenanceDaemonFailureCode::runtime_stage_safe_error("scheduled_sweep"))
        .or_else(|| {
            report.outbox_drain.failed().map(|_| {
                TenantMaintenanceDaemonFailureCode::runtime_stage_safe_error("outbox_drain")
            })
        })
}
