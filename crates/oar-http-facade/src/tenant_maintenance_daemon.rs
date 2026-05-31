use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oar_core::storage::postgres::{
    postgres_repository_safe_error_reason, PostgresRepositoryError,
    PostgresTenantMaintenanceConfigValidationError, PostgresTenantMaintenanceReport,
    PostgresTenantMaintenanceWorker, PostgresTenantRepository,
};
use oar_lark_adapter::{
    build_postgres_async_feishu_auth_refresh_adapter, AuditOutboxSinkDispatcher,
    FeishuAuthRefreshAdapterBuildError, PostgresAsyncFeishuAuthRefreshAdapter,
    ReqwestAsyncHttpClient, WebhookAuditOutboxSink, WebhookAuditOutboxSinkConfigError,
};
use oar_runtime::{
    DiscoveringTenantMaintenanceRuntime, PostgresRuntimeTenantDiscovery, RuntimeTenantTickFactory,
    RuntimeTenantTickFactoryFuture, RuntimeTick, RuntimeTickFuture,
    TenantMaintenanceRuntimeConfigValidationError,
};
use tokio::task::{JoinError, JoinHandle};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::runtime::OarHttpFacadeRuntime;
use crate::tenant_maintenance::{
    TenantMaintenanceAuditOutboxSinkSettings, TenantMaintenanceSettingsError,
    TenantMaintenanceWorkerSettings,
};

const DAEMON_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

type FacadeAuditOutboxDispatcher =
    AuditOutboxSinkDispatcher<WebhookAuditOutboxSink<ReqwestAsyncHttpClient>>;
type FacadeTenantMaintenanceCoreWorker = PostgresTenantMaintenanceWorker<
    PostgresAsyncFeishuAuthRefreshAdapter,
    FacadeAuditOutboxDispatcher,
    fn() -> u64,
>;

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum TenantMaintenanceDaemonStartError {
    MissingPersistence,
    MissingFeishuAuth,
    InvalidRuntimeConfig,
    InvalidWorkerConfig,
    RefreshAdapterBuildFailed,
    WebhookSinkBuildFailed,
}

impl fmt::Debug for TenantMaintenanceDaemonStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantMaintenanceDaemonStartError")
            .field("safe_error", &self.to_string())
            .finish()
    }
}

impl fmt::Display for TenantMaintenanceDaemonStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPersistence => write!(f, "tenant_maintenance_daemon_missing_persistence"),
            Self::MissingFeishuAuth => write!(f, "tenant_maintenance_daemon_missing_feishu_auth"),
            Self::InvalidRuntimeConfig => {
                write!(f, "tenant_maintenance_daemon_runtime_config_invalid")
            }
            Self::InvalidWorkerConfig => {
                write!(f, "tenant_maintenance_daemon_worker_config_invalid")
            }
            Self::RefreshAdapterBuildFailed => {
                write!(f, "tenant_maintenance_daemon_refresh_adapter_build_failed")
            }
            Self::WebhookSinkBuildFailed => {
                write!(f, "tenant_maintenance_daemon_webhook_sink_build_failed")
            }
        }
    }
}

impl Error for TenantMaintenanceDaemonStartError {}

#[derive(Clone, PartialEq, Eq)]
struct TenantMaintenanceTickError {
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

struct FacadeTenantMaintenanceTick {
    worker: FacadeTenantMaintenanceCoreWorker,
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

pub(crate) struct TenantMaintenanceDaemonHandle {
    cancellation: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl fmt::Debug for TenantMaintenanceDaemonHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantMaintenanceDaemonHandle")
            .field("cancellation", &"[REDACTED]")
            .field("task", &"[REDACTED]")
            .finish()
    }
}

impl TenantMaintenanceDaemonHandle {
    pub(crate) async fn shutdown(mut self) {
        self.cancellation.cancel();
        let Some(mut task) = self.task.take() else {
            return;
        };
        tokio::select! {
            result = &mut task => {
                if let Err(error) = result {
                    warn!(
                        panic = error.is_panic(),
                        cancelled = error.is_cancelled(),
                        "tenant maintenance daemon task finished with join error"
                    );
                }
            }
            _ = time::sleep(DAEMON_SHUTDOWN_TIMEOUT) => {
                warn!("tenant maintenance daemon shutdown timed out; aborting task");
                task.abort();
                let _ = task.await;
            }
        }
    }

    pub(crate) async fn wait_finished(&mut self) -> Result<(), JoinError> {
        match self.task.as_mut() {
            Some(task) => task.await,
            None => Ok(()),
        }
    }
}

impl Drop for TenantMaintenanceDaemonHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.as_ref() {
            if !task.is_finished() {
                task.abort();
            }
        }
    }
}

pub(crate) fn spawn_tenant_maintenance_daemon(
    runtime: &OarHttpFacadeRuntime,
) -> Result<Option<TenantMaintenanceDaemonHandle>, TenantMaintenanceDaemonStartError> {
    let Some(settings) = runtime.tenant_maintenance.clone() else {
        return Ok(None);
    };
    let persistence = runtime
        .persistence
        .clone()
        .ok_or(TenantMaintenanceDaemonStartError::MissingPersistence)?;
    let feishu_login = runtime
        .feishu_login
        .clone()
        .ok_or(TenantMaintenanceDaemonStartError::MissingFeishuAuth)?;

    let pool = persistence.pool();
    let discovery =
        PostgresRuntimeTenantDiscovery::new(PostgresTenantRepository::new(pool.clone()));
    let factory = FacadeTenantMaintenanceTickFactory {
        pool,
        worker_settings: settings.worker.clone(),
        audit_outbox_sink: settings.audit_outbox_sink.clone(),
        feishu_login,
        persistence,
    };
    let mut daemon =
        DiscoveringTenantMaintenanceRuntime::try_new(settings.runtime, discovery, factory)
            .map_err(daemon_runtime_config_error)?;
    let cancellation = CancellationToken::new();
    let daemon_cancellation = cancellation.clone();
    let status = runtime.tenant_maintenance_daemon_status().clone();
    status.mark_running();
    let task_status = status.clone();
    let task = tokio::spawn(async move {
        let report = daemon
            .run_until_cancelled_with_observer(&daemon_cancellation, |round| {
                task_status.record_round(round);
            })
            .await;
        task_status.mark_stopped(&report);
        info!(
            successful_rounds = report.successful_rounds,
            failed_rounds = report.failed_rounds,
            cancelled = report.cancelled,
            "tenant maintenance daemon stopped"
        );
    });
    Ok(Some(TenantMaintenanceDaemonHandle {
        cancellation,
        task: Some(task),
    }))
}

struct FacadeTenantMaintenanceTickFactory {
    pool: sqlx::PgPool,
    worker_settings: TenantMaintenanceWorkerSettings,
    audit_outbox_sink: TenantMaintenanceAuditOutboxSinkSettings,
    feishu_login: std::sync::Arc<crate::feishu_auth::FeishuLoginRuntime>,
    persistence: crate::persistence::FacadePersistenceRuntime,
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
        Box::pin(async move {
            build_tenant_maintenance_worker(
                pool,
                worker_settings,
                audit_outbox_sink,
                feishu_login,
                persistence,
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
    Ok(FacadeTenantMaintenanceTick { worker })
}

fn system_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn daemon_runtime_config_error(
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
        safe_error: format!(
            "tenant_maintenance_runtime_tick_failed: {}",
            postgres_repository_safe_error_reason(&error)
        ),
    }
}

fn report_stage_safe_error(report: &PostgresTenantMaintenanceReport) -> Option<String> {
    report
        .scheduled_sweep
        .failed()
        .map(|failure| {
            format!(
                "tenant_maintenance_runtime_stage_failed: {}",
                failure.safe_error
            )
        })
        .or_else(|| {
            report.outbox_drain.failed().map(|failure| {
                format!(
                    "tenant_maintenance_runtime_stage_failed: {}",
                    failure.safe_error
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use oar_core::storage::postgres::tenant_maintenance::{
        PostgresTenantMaintenanceStage, PostgresTenantMaintenanceStageFailure,
    };
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::persistence::FacadePersistenceRuntime;
    use crate::tenant_maintenance::tenant_maintenance_runtime_settings_from_env_map;

    #[test]
    fn disabled_runtime_does_not_start_daemon() {
        let runtime = OarHttpFacadeRuntime::disabled();
        let handle = spawn_tenant_maintenance_daemon(&runtime).expect("disabled daemon start");

        assert!(handle.is_none());
    }

    #[test]
    fn daemon_start_error_does_not_leak_runtime_secrets() {
        let settings = tenant_maintenance_runtime_settings_from_env_map(
            &configured_tenant_maintenance_env,
            true,
            true,
        )
        .expect("settings")
        .expect("enabled");
        let runtime = OarHttpFacadeRuntime {
            tenant_maintenance: Some(settings),
            ..OarHttpFacadeRuntime::disabled()
        };
        let error =
            spawn_tenant_maintenance_daemon(&runtime).expect_err("missing persistence should fail");
        let rendered = format!("{error:?} {error}");

        assert!(!rendered.contains("webhook-secret"));
        assert!(!rendered.contains("feishu-sensitive-secret"));
        assert!(!rendered.contains("key-test-v1"));
    }

    #[test]
    fn stage_failure_report_is_promoted_to_runtime_safe_error() {
        let report = PostgresTenantMaintenanceReport {
            scheduled_sweep: PostgresTenantMaintenanceStage::Failed(
                PostgresTenantMaintenanceStageFailure {
                    safe_error: "scheduled_safe_error".to_string(),
                },
            ),
            outbox_drain: PostgresTenantMaintenanceStage::Failed(
                PostgresTenantMaintenanceStageFailure {
                    safe_error: "outbox_safe_error".to_string(),
                },
            ),
        };

        assert_eq!(
            report_stage_safe_error(&report).as_deref(),
            Some("tenant_maintenance_runtime_stage_failed: scheduled_safe_error")
        );
    }

    #[tokio::test]
    async fn tick_factory_builds_worker_without_eager_database_access_or_secret_debug() {
        let settings = tenant_maintenance_runtime_settings_from_env_map(
            &configured_tenant_maintenance_env,
            true,
            true,
        )
        .expect("settings")
        .expect("enabled");
        let persistence = test_persistence();
        let feishu_login = crate::feishu_auth::FeishuLoginRuntime::from_env_map(
            &configured_tenant_maintenance_env,
        )
        .expect("feishu runtime")
        .expect("enabled feishu runtime");
        let mut factory = FacadeTenantMaintenanceTickFactory {
            pool: persistence.pool(),
            worker_settings: settings.worker,
            audit_outbox_sink: settings.audit_outbox_sink,
            feishu_login: std::sync::Arc::new(feishu_login),
            persistence,
        };
        let worker = factory.build_tick("tenant_factory_test").await;

        assert!(worker.is_ok());
        let debug = format!("{factory:?}");
        assert!(!debug.contains("webhook-secret"));
        assert!(!debug.contains("feishu-sensitive-secret"));
    }

    #[tokio::test]
    async fn daemon_handle_drop_cancels_background_task() {
        let cancellation = CancellationToken::new();
        let observed_cancellation = cancellation.clone();
        let task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let handle = TenantMaintenanceDaemonHandle {
            cancellation,
            task: Some(task),
        };

        drop(handle);

        assert!(observed_cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn daemon_handle_wait_finished_observes_task_completion() {
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(async {});
        let mut handle = TenantMaintenanceDaemonHandle {
            cancellation,
            task: Some(task),
        };

        assert!(handle.wait_finished().await.is_ok());
    }

    impl fmt::Debug for FacadeTenantMaintenanceTickFactory {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("FacadeTenantMaintenanceTickFactory")
                .field("pool", &"[REDACTED]")
                .field("worker_settings", &"[REDACTED]")
                .field("audit_outbox_sink", &"[REDACTED]")
                .field("feishu_login", &"[REDACTED]")
                .field("persistence", &"[REDACTED]")
                .finish()
        }
    }

    fn configured_tenant_maintenance_env(key: &str) -> Option<String> {
        match key {
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("feishu-sensitive-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
            "OAR_TENANT_MAINTENANCE_INSTANCE_ID" => Some("tenant-maintenance-test".to_string()),
            "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK" => Some("webhook".to_string()),
            "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL" => {
                Some("https://audit.example.test/webhook?token=webhook-secret".to_string())
            }
            _ => None,
        }
    }

    fn test_persistence() -> FacadePersistenceRuntime {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/oar_unreachable")
            .expect("lazy pool");
        FacadePersistenceRuntime::new_for_test(pool, "key-test-v1".to_string(), [7; 32])
    }
}
