use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::storage::postgres::tenant_maintenance::{
    PostgresTenantMaintenanceConfig, PostgresTenantMaintenanceConfigValidationError,
};

fn valid_tenant_maintenance_config() -> PostgresTenantMaintenanceConfig {
    PostgresTenantMaintenanceConfig {
        tenant_id: "tenant".to_string(),
        lease_id: "lease".to_string(),
        audit_stream: "audit".to_string(),
        scheduled_lease_ms: 1,
        scheduled_retry_delay_ms: 1,
        scheduled_next_run_delay_ms: 1,
        scheduled_backlog_next_run_delay_ms: 1,
        scheduled_due_before_ms: 0,
        scheduled_limit: 1,
        scheduled_audit_trace_id: "trace".to_string(),
        scheduled_audit_sequence_start: 1,
        scheduled_actor: AuditActor {
            kind: AuditActorKind::System,
            actor_id: "maintenance".to_string(),
            display_name: None,
        },
        scheduled_workspace_id: None,
        outbox_batch_limit: 1,
        outbox_lease_ms: 1,
        outbox_retry_delay_ms: 1,
        outbox_max_attempts: 1,
    }
}

#[test]
fn tenant_maintenance_config_validate_rejects_fail_closed_inputs() {
    let mut config = valid_tenant_maintenance_config();
    config.tenant_id.clear();
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
            "tenant_id"
        ))
    );

    let mut config = valid_tenant_maintenance_config();
    config.lease_id.clear();
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
            "lease_id"
        ))
    );

    let mut config = valid_tenant_maintenance_config();
    config.audit_stream.clear();
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
            "audit_stream"
        ))
    );

    let mut config = valid_tenant_maintenance_config();
    config.scheduled_audit_trace_id.clear();
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
            "scheduled_audit_trace_id"
        ))
    );

    let mut config = valid_tenant_maintenance_config();
    config.scheduled_limit = 0;
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::NonPositiveField("scheduled_limit"))
    );

    let mut config = valid_tenant_maintenance_config();
    config.outbox_batch_limit = 0;
    assert_eq!(
        config.validate(),
        Err(PostgresTenantMaintenanceConfigValidationError::NonPositiveField("outbox_batch_limit"))
    );
}
