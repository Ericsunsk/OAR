mod audit_sink;
mod env;
mod worker;

use std::error::Error;
use std::fmt;
use std::time::Duration;

use oar_runtime::TenantMaintenanceRuntimeConfig;

use self::audit_sink::tenant_maintenance_audit_outbox_sink_from_env;
pub(crate) use self::audit_sink::TenantMaintenanceAuditOutboxSinkSettings;
use self::env::{bounded_u64_env, enabled_env_flag};
pub(crate) use self::worker::TenantMaintenanceWorkerSettings;
use crate::util::non_empty_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceRuntimeSettings {
    pub(crate) runtime: TenantMaintenanceRuntimeConfig,
    pub(crate) worker: TenantMaintenanceWorkerSettings,
    pub(crate) audit_outbox_sink: TenantMaintenanceAuditOutboxSinkSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TenantMaintenanceSettingsError {
    RequiresDatabase,
    RequiresFeishuAuth,
    RequiresAuditOutboxSink,
    MissingInstanceId,
    InvalidConfig,
}

impl fmt::Display for TenantMaintenanceSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresDatabase => write!(f, "oar_tenant_maintenance_database_required"),
            Self::RequiresFeishuAuth => write!(f, "oar_tenant_maintenance_feishu_auth_required"),
            Self::RequiresAuditOutboxSink => {
                write!(f, "oar_tenant_maintenance_audit_outbox_sink_required")
            }
            Self::MissingInstanceId => write!(f, "oar_tenant_maintenance_instance_id_required"),
            Self::InvalidConfig => write!(f, "oar_tenant_maintenance_config_invalid"),
        }
    }
}

impl Error for TenantMaintenanceSettingsError {}

pub(crate) fn tenant_maintenance_runtime_settings_from_env_map(
    env: &impl Fn(&str) -> Option<String>,
    has_persistence: bool,
    has_feishu_login: bool,
) -> Result<Option<TenantMaintenanceRuntimeSettings>, TenantMaintenanceSettingsError> {
    if !enabled_env_flag(env, TENANT_MAINTENANCE_ENABLED_ENV)? {
        return Ok(None);
    }
    if enabled_env_flag(env, ALLOW_EPHEMERAL_GRANT_KEY_ENV)? {
        return Err(TenantMaintenanceSettingsError::InvalidConfig);
    }
    if !has_persistence {
        return Err(TenantMaintenanceSettingsError::RequiresDatabase);
    }
    if !has_feishu_login {
        return Err(TenantMaintenanceSettingsError::RequiresFeishuAuth);
    }
    let audit_outbox_sink = tenant_maintenance_audit_outbox_sink_from_env(env)?;

    let instance_id = non_empty_env(env, TENANT_MAINTENANCE_INSTANCE_ID_ENV)
        .ok_or(TenantMaintenanceSettingsError::MissingInstanceId)?;
    let interval_ms = bounded_u64_env(
        env,
        TENANT_MAINTENANCE_INTERVAL_MS_ENV,
        DEFAULT_TENANT_MAINTENANCE_INTERVAL_MS,
        MIN_TENANT_MAINTENANCE_INTERVAL_MS,
        MAX_TENANT_MAINTENANCE_INTERVAL_MS,
    )?;
    let due_lookahead_ms = bounded_u64_env(
        env,
        TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS_ENV,
        DEFAULT_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
        MIN_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
        MAX_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS,
    )?;
    let runtime = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_millis(interval_ms),
    };
    runtime
        .validate()
        .map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;

    let worker = TenantMaintenanceWorkerSettings::from_runtime_env(instance_id, due_lookahead_ms)?;

    Ok(Some(TenantMaintenanceRuntimeSettings {
        runtime,
        worker,
        audit_outbox_sink,
    }))
}

const TENANT_MAINTENANCE_ENABLED_ENV: &str = "OAR_TENANT_MAINTENANCE_ENABLED";
const TENANT_MAINTENANCE_INSTANCE_ID_ENV: &str = "OAR_TENANT_MAINTENANCE_INSTANCE_ID";
const TENANT_MAINTENANCE_INTERVAL_MS_ENV: &str = "OAR_TENANT_MAINTENANCE_INTERVAL_MS";
const TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS_ENV: &str = "OAR_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS";
const ALLOW_EPHEMERAL_GRANT_KEY_ENV: &str = "OAR_ALLOW_EPHEMERAL_GRANT_KEY";
const DEFAULT_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 300_000;
const MIN_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 5_000;
const MAX_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 3_600_000;
const MIN_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 1_000;
const MAX_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 86_400_000;
