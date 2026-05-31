use std::error::Error;
use std::fmt;

use oar_core::storage::postgres::PostgresTenantRepository;
use oar_runtime::{DiscoveringTenantMaintenanceRuntime, PostgresRuntimeTenantDiscovery};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::runtime::OarHttpFacadeRuntime;
use crate::tenant_maintenance_daemon_failure::TenantMaintenanceDaemonFailureCode;

mod handle;
mod worker;

pub(crate) use handle::TenantMaintenanceDaemonHandle;

use worker::{daemon_runtime_config_error, FacadeTenantMaintenanceTickFactory};

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
        f.write_str(self.failure_code().as_str())
    }
}

impl Error for TenantMaintenanceDaemonStartError {}

impl TenantMaintenanceDaemonStartError {
    fn failure_code(&self) -> TenantMaintenanceDaemonFailureCode {
        match self {
            Self::MissingPersistence => {
                TenantMaintenanceDaemonFailureCode::DaemonMissingPersistence
            }
            Self::MissingFeishuAuth => TenantMaintenanceDaemonFailureCode::DaemonMissingFeishuAuth,
            Self::InvalidRuntimeConfig => {
                TenantMaintenanceDaemonFailureCode::DaemonRuntimeConfigInvalid
            }
            Self::InvalidWorkerConfig => {
                TenantMaintenanceDaemonFailureCode::DaemonWorkerConfigInvalid
            }
            Self::RefreshAdapterBuildFailed => {
                TenantMaintenanceDaemonFailureCode::DaemonRefreshAdapterBuildFailed
            }
            Self::WebhookSinkBuildFailed => {
                TenantMaintenanceDaemonFailureCode::DaemonWebhookSinkBuildFailed
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
    let status = runtime.tenant_maintenance_daemon_status().clone();
    let factory = FacadeTenantMaintenanceTickFactory {
        pool,
        worker_settings: settings.worker.clone(),
        audit_outbox_sink: settings.audit_outbox_sink.clone(),
        feishu_login,
        persistence,
        status: status.clone(),
    };
    let mut daemon =
        DiscoveringTenantMaintenanceRuntime::try_new(settings.runtime, discovery, factory)
            .map_err(daemon_runtime_config_error)?;
    let cancellation = CancellationToken::new();
    let daemon_cancellation = cancellation.clone();
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

#[cfg(test)]
mod tests {
    use oar_core::storage::postgres::tenant_maintenance::{
        PostgresTenantMaintenanceStage, PostgresTenantMaintenanceStageFailure,
    };
    use oar_core::storage::postgres::PostgresTenantMaintenanceReport;
    use oar_runtime::RuntimeTenantTickFactory;
    use sqlx::postgres::PgPoolOptions;

    use super::worker::report_stage_safe_error;
    use super::*;
    use crate::persistence::FacadePersistenceRuntime;
    use crate::tenant_maintenance::tenant_maintenance_runtime_settings_from_env_map;
    use crate::tenant_maintenance_daemon_status::TenantMaintenanceDaemonStatusHandle;

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
            Some("tenant_maintenance_runtime_stage_failed: scheduled_sweep")
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
            status: TenantMaintenanceDaemonStatusHandle::for_enabled(true),
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
