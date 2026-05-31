use std::error::Error;
use std::fmt;
use std::time::Duration;

use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::storage::postgres::PostgresTenantMaintenanceConfig;
use oar_runtime::TenantMaintenanceRuntimeConfig;
use reqwest::Url;

use crate::util::non_empty_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceRuntimeSettings {
    pub(crate) runtime: TenantMaintenanceRuntimeConfig,
    pub(crate) worker: TenantMaintenanceWorkerSettings,
    pub(crate) audit_outbox_sink: TenantMaintenanceAuditOutboxSinkSettings,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum TenantMaintenanceAuditOutboxSinkSettings {
    Webhook { endpoint: String },
}

impl fmt::Debug for TenantMaintenanceAuditOutboxSinkSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Webhook { .. } => f
                .debug_struct("Webhook")
                .field("endpoint", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceWorkerSettings {
    pub(crate) instance_id: String,
    pub(crate) due_lookahead_ms: u64,
    audit_stream: String,
    scheduled_lease_ms: u64,
    scheduled_retry_delay_ms: u64,
    scheduled_next_run_delay_ms: u64,
    scheduled_backlog_next_run_delay_ms: u64,
    scheduled_limit: u32,
    scheduled_audit_sequence_start: u64,
    outbox_batch_limit: i64,
    outbox_lease_ms: u64,
    outbox_retry_delay_ms: u64,
    outbox_max_attempts: u32,
}

impl TenantMaintenanceWorkerSettings {
    pub(crate) fn config_for_tenant(
        &self,
        tenant_id: &str,
        now_ms: u64,
    ) -> Result<PostgresTenantMaintenanceConfig, TenantMaintenanceSettingsError> {
        let tenant_id = tenant_id.trim().to_string();
        let due_before_ms = now_ms.saturating_add(self.due_lookahead_ms);
        let config = PostgresTenantMaintenanceConfig {
            tenant_id: tenant_id.clone(),
            lease_id: format!("oar_tenant_maintenance:{}:{tenant_id}", self.instance_id),
            audit_stream: self.audit_stream.clone(),
            scheduled_lease_ms: self.scheduled_lease_ms,
            scheduled_retry_delay_ms: self.scheduled_retry_delay_ms,
            scheduled_next_run_delay_ms: self.scheduled_next_run_delay_ms,
            scheduled_backlog_next_run_delay_ms: self.scheduled_backlog_next_run_delay_ms,
            scheduled_due_before_ms: due_before_ms,
            scheduled_limit: self.scheduled_limit,
            scheduled_audit_trace_id: format!("oar_tenant_maintenance:{tenant_id}:token_refresh"),
            scheduled_audit_sequence_start: self.scheduled_audit_sequence_start,
            scheduled_actor: self.audit_actor(),
            scheduled_workspace_id: None,
            outbox_batch_limit: self.outbox_batch_limit,
            outbox_lease_ms: self.outbox_lease_ms,
            outbox_retry_delay_ms: self.outbox_retry_delay_ms,
            outbox_max_attempts: self.outbox_max_attempts,
        };
        config
            .validate()
            .map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;
        Ok(config)
    }

    fn audit_actor(&self) -> AuditActor {
        AuditActor {
            kind: AuditActorKind::Service,
            actor_id: format!("oar_tenant_maintenance:{}", self.instance_id),
            display_name: Some("OAR Tenant Maintenance".to_string()),
        }
    }
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
    validate_tenant_maintenance_instance_id(&instance_id)?;
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

    let worker = TenantMaintenanceWorkerSettings {
        instance_id,
        due_lookahead_ms,
        audit_stream: DEFAULT_TENANT_MAINTENANCE_AUDIT_STREAM.to_string(),
        scheduled_lease_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LEASE_MS,
        scheduled_retry_delay_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_RETRY_DELAY_MS,
        scheduled_next_run_delay_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_NEXT_RUN_DELAY_MS,
        scheduled_backlog_next_run_delay_ms:
            DEFAULT_TENANT_MAINTENANCE_SCHEDULED_BACKLOG_NEXT_RUN_DELAY_MS,
        scheduled_limit: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LIMIT,
        scheduled_audit_sequence_start: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_AUDIT_SEQUENCE_START,
        outbox_batch_limit: DEFAULT_TENANT_MAINTENANCE_OUTBOX_BATCH_LIMIT,
        outbox_lease_ms: DEFAULT_TENANT_MAINTENANCE_OUTBOX_LEASE_MS,
        outbox_retry_delay_ms: DEFAULT_TENANT_MAINTENANCE_OUTBOX_RETRY_DELAY_MS,
        outbox_max_attempts: DEFAULT_TENANT_MAINTENANCE_OUTBOX_MAX_ATTEMPTS,
    };
    worker.config_for_tenant(TENANT_MAINTENANCE_CONFIG_PROBE_TENANT_ID, 0)?;

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
const TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK_ENV: &str = "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK";
const TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL_ENV: &str =
    "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL";
const ALLOW_EPHEMERAL_GRANT_KEY_ENV: &str = "OAR_ALLOW_EPHEMERAL_GRANT_KEY";
const DEFAULT_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 300_000;
const MIN_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 5_000;
const MAX_TENANT_MAINTENANCE_INTERVAL_MS: u64 = 3_600_000;
const MIN_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 1_000;
const MAX_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS: u64 = 86_400_000;
const TENANT_MAINTENANCE_INSTANCE_ID_MAX_LEN: usize = 128;
const DEFAULT_TENANT_MAINTENANCE_AUDIT_STREAM: &str = "audit-events";
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LEASE_MS: u64 = 30_000;
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_RETRY_DELAY_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_NEXT_RUN_DELAY_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_BACKLOG_NEXT_RUN_DELAY_MS: u64 = 5_000;
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LIMIT: u32 = 50;
const DEFAULT_TENANT_MAINTENANCE_SCHEDULED_AUDIT_SEQUENCE_START: u64 = 1;
const DEFAULT_TENANT_MAINTENANCE_OUTBOX_BATCH_LIMIT: i64 = 100;
const DEFAULT_TENANT_MAINTENANCE_OUTBOX_LEASE_MS: u64 = 30_000;
const DEFAULT_TENANT_MAINTENANCE_OUTBOX_RETRY_DELAY_MS: u64 = 60_000;
const DEFAULT_TENANT_MAINTENANCE_OUTBOX_MAX_ATTEMPTS: u32 = 5;
const TENANT_MAINTENANCE_CONFIG_PROBE_TENANT_ID: &str = "tenant_maintenance_config_probe";

fn tenant_maintenance_audit_outbox_sink_from_env(
    env: &impl Fn(&str) -> Option<String>,
) -> Result<TenantMaintenanceAuditOutboxSinkSettings, TenantMaintenanceSettingsError> {
    let Some(kind) = non_empty_env(env, TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK_ENV) else {
        return Err(TenantMaintenanceSettingsError::RequiresAuditOutboxSink);
    };
    match kind.as_str() {
        "webhook" => {
            let endpoint = non_empty_env(env, TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL_ENV)
                .ok_or(TenantMaintenanceSettingsError::RequiresAuditOutboxSink)?;
            validate_webhook_endpoint(&endpoint)?;
            Ok(TenantMaintenanceAuditOutboxSinkSettings::Webhook { endpoint })
        }
        "noop" | "local-noop" => Err(TenantMaintenanceSettingsError::InvalidConfig),
        _ => Err(TenantMaintenanceSettingsError::InvalidConfig),
    }
}

fn validate_webhook_endpoint(value: &str) -> Result<(), TenantMaintenanceSettingsError> {
    let endpoint = Url::parse(value).map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;
    if endpoint.scheme() == "https" && endpoint.host().is_some() {
        Ok(())
    } else {
        Err(TenantMaintenanceSettingsError::InvalidConfig)
    }
}

fn validate_tenant_maintenance_instance_id(
    value: &str,
) -> Result<(), TenantMaintenanceSettingsError> {
    let valid = !value.is_empty()
        && value.len() <= TENANT_MAINTENANCE_INSTANCE_ID_MAX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    if !valid {
        return Err(TenantMaintenanceSettingsError::InvalidConfig);
    }
    Ok(())
}

fn enabled_env_flag(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
) -> Result<bool, TenantMaintenanceSettingsError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(false);
    };
    match value.as_str() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Ok(false),
        _ => Err(TenantMaintenanceSettingsError::InvalidConfig),
    }
}

fn bounded_u64_env(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
    default_value: u64,
    min_value: u64,
    max_value: u64,
) -> Result<u64, TenantMaintenanceSettingsError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(default_value);
    };
    let value = value
        .parse::<u64>()
        .map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;
    if !(min_value..=max_value).contains(&value) {
        return Err(TenantMaintenanceSettingsError::InvalidConfig);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_settings_build_safe_core_config_for_tenant() {
        let settings = tenant_maintenance_runtime_settings_from_env_map(
            &configured_tenant_maintenance_env,
            true,
            true,
        )
        .expect("settings")
        .expect("enabled settings");

        let config = settings
            .worker
            .config_for_tenant(" tenant-worker-test ", 1_700_000_000_000)
            .expect("worker config");

        assert_eq!(config.tenant_id, "tenant-worker-test");
        assert_eq!(
            config.lease_id,
            "oar_tenant_maintenance:oar-prod-1:tenant-worker-test"
        );
        assert_eq!(
            config.scheduled_audit_trace_id,
            "oar_tenant_maintenance:tenant-worker-test:token_refresh"
        );
        assert_eq!(config.scheduled_due_before_ms, 1_700_000_300_000);
        assert_eq!(config.scheduled_actor.kind, AuditActorKind::Service);
        assert_eq!(
            config.scheduled_actor.actor_id,
            "oar_tenant_maintenance:oar-prod-1"
        );
        assert_eq!(config.audit_stream, "audit-events");
        assert_eq!(config.scheduled_limit, 50);
        assert_eq!(config.outbox_batch_limit, 100);
        assert!(!format!("{settings:?}").contains("webhook-secret"));
    }

    #[test]
    fn worker_settings_reject_empty_tenant_config_without_echoing_input() {
        let settings = tenant_maintenance_runtime_settings_from_env_map(
            &configured_tenant_maintenance_env,
            true,
            true,
        )
        .expect("settings")
        .expect("enabled settings");

        let error = settings
            .worker
            .config_for_tenant("   ", 1_700_000_000_000)
            .expect_err("empty tenant should fail");

        assert_eq!(error.to_string(), "oar_tenant_maintenance_config_invalid");
        assert!(!format!("{error:?}").contains("tenant-worker-test"));
    }

    fn configured_tenant_maintenance_env(key: &str) -> Option<String> {
        match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
            "OAR_TENANT_MAINTENANCE_INSTANCE_ID" => Some("oar-prod-1".to_string()),
            "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK" => Some("webhook".to_string()),
            "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL" => {
                Some("https://audit.example.test/webhook?token=webhook-secret".to_string())
            }
            _ => None,
        }
    }
}
