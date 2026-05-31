use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::storage::postgres::PostgresTenantMaintenanceConfig;

use super::TenantMaintenanceSettingsError;

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
    pub(super) fn from_runtime_env(
        instance_id: String,
        due_lookahead_ms: u64,
    ) -> Result<Self, TenantMaintenanceSettingsError> {
        validate_tenant_maintenance_instance_id(&instance_id)?;
        let settings = Self {
            instance_id,
            due_lookahead_ms,
            audit_stream: DEFAULT_TENANT_MAINTENANCE_AUDIT_STREAM.to_string(),
            scheduled_lease_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LEASE_MS,
            scheduled_retry_delay_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_RETRY_DELAY_MS,
            scheduled_next_run_delay_ms: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_NEXT_RUN_DELAY_MS,
            scheduled_backlog_next_run_delay_ms:
                DEFAULT_TENANT_MAINTENANCE_SCHEDULED_BACKLOG_NEXT_RUN_DELAY_MS,
            scheduled_limit: DEFAULT_TENANT_MAINTENANCE_SCHEDULED_LIMIT,
            scheduled_audit_sequence_start:
                DEFAULT_TENANT_MAINTENANCE_SCHEDULED_AUDIT_SEQUENCE_START,
            outbox_batch_limit: DEFAULT_TENANT_MAINTENANCE_OUTBOX_BATCH_LIMIT,
            outbox_lease_ms: DEFAULT_TENANT_MAINTENANCE_OUTBOX_LEASE_MS,
            outbox_retry_delay_ms: DEFAULT_TENANT_MAINTENANCE_OUTBOX_RETRY_DELAY_MS,
            outbox_max_attempts: DEFAULT_TENANT_MAINTENANCE_OUTBOX_MAX_ATTEMPTS,
        };
        settings.config_for_tenant(TENANT_MAINTENANCE_CONFIG_PROBE_TENANT_ID, 0)?;
        Ok(settings)
    }

    pub(crate) fn config_for_tenant(
        &self,
        tenant_id: &str,
        now_ms: u64,
    ) -> Result<PostgresTenantMaintenanceConfig, TenantMaintenanceSettingsError> {
        let tenant_id = tenant_id.trim().to_string();
        let due_before_ms = now_ms.saturating_add(self.due_lookahead_ms);
        let config = PostgresTenantMaintenanceConfig {
            tenant_id: tenant_id.clone(),
            lease_id: maintenance_lease_id(&self.instance_id, &tenant_id),
            audit_stream: self.audit_stream.clone(),
            scheduled_lease_ms: self.scheduled_lease_ms,
            scheduled_retry_delay_ms: self.scheduled_retry_delay_ms,
            scheduled_next_run_delay_ms: self.scheduled_next_run_delay_ms,
            scheduled_backlog_next_run_delay_ms: self.scheduled_backlog_next_run_delay_ms,
            scheduled_due_before_ms: due_before_ms,
            scheduled_limit: self.scheduled_limit,
            scheduled_audit_trace_id: token_refresh_trace_id(&tenant_id),
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
            actor_id: service_actor_id(&self.instance_id),
            display_name: Some("OAR Tenant Maintenance".to_string()),
        }
    }
}

const TENANT_MAINTENANCE_ID_PREFIX: &str = "oar_tenant_maintenance";
const TENANT_MAINTENANCE_INSTANCE_ID_MAX_LEN: usize = 128;
const TENANT_MAINTENANCE_CONFIG_PROBE_TENANT_ID: &str = "tenant_maintenance_config_probe";
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

fn maintenance_lease_id(instance_id: &str, tenant_id: &str) -> String {
    format!("{TENANT_MAINTENANCE_ID_PREFIX}:{instance_id}:{tenant_id}")
}

fn token_refresh_trace_id(tenant_id: &str) -> String {
    format!("{TENANT_MAINTENANCE_ID_PREFIX}:{tenant_id}:token_refresh")
}

fn service_actor_id(instance_id: &str) -> String {
    format!("{TENANT_MAINTENANCE_ID_PREFIX}:{instance_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_settings_build_safe_core_config_for_tenant() {
        let settings =
            TenantMaintenanceWorkerSettings::from_runtime_env("oar-prod-1".to_string(), 300_000)
                .expect("settings");

        let config = settings
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
    }

    #[test]
    fn worker_settings_reject_empty_tenant_config_without_echoing_input() {
        let settings =
            TenantMaintenanceWorkerSettings::from_runtime_env("oar-prod-1".to_string(), 300_000)
                .expect("settings");

        let error = settings
            .config_for_tenant("   ", 1_700_000_000_000)
            .expect_err("empty tenant should fail");

        assert_eq!(error.to_string(), "oar_tenant_maintenance_config_invalid");
        assert!(!format!("{error:?}").contains("tenant-worker-test"));
    }

    #[test]
    fn worker_settings_validate_instance_id_boundary() {
        for instance_id in ["abc.DEF_123:-", &"a".repeat(128)] {
            TenantMaintenanceWorkerSettings::from_runtime_env(instance_id.to_string(), 300_000)
                .expect("valid instance id");
        }

        for instance_id in [
            "",
            "tenant/maintenance",
            "tenant maintenance",
            "租户",
            &"a".repeat(129),
        ] {
            assert_eq!(
                TenantMaintenanceWorkerSettings::from_runtime_env(instance_id.to_string(), 300_000),
                Err(TenantMaintenanceSettingsError::InvalidConfig)
            );
        }
    }
}
