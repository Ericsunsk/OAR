pub(crate) fn classify_failure_code(value: impl AsRef<str>) -> &'static str {
    let value = value.as_ref().trim();
    let public_code = match value {
        "tenant_maintenance_daemon_stopped_unexpectedly" => {
            Some("tenant_maintenance_daemon_stopped_unexpectedly")
        }
        "tenant_maintenance_daemon_task_failed" => Some("tenant_maintenance_daemon_task_failed"),
        "tenant_maintenance_daemon_missing_persistence" => {
            Some("tenant_maintenance_daemon_missing_persistence")
        }
        "tenant_maintenance_daemon_missing_feishu_auth" => {
            Some("tenant_maintenance_daemon_missing_feishu_auth")
        }
        "tenant_maintenance_daemon_runtime_config_invalid" => {
            Some("tenant_maintenance_daemon_runtime_config_invalid")
        }
        "tenant_maintenance_daemon_worker_config_invalid" => {
            Some("tenant_maintenance_daemon_worker_config_invalid")
        }
        "tenant_maintenance_daemon_refresh_adapter_build_failed" => {
            Some("tenant_maintenance_daemon_refresh_adapter_build_failed")
        }
        "tenant_maintenance_daemon_webhook_sink_build_failed" => {
            Some("tenant_maintenance_daemon_webhook_sink_build_failed")
        }
        "tenant_maintenance_discovery_invalid: empty_registry" => {
            Some("tenant_maintenance_discovery_invalid_empty_registry")
        }
        "tenant_maintenance_discovery_invalid: empty_tenant_id" => {
            Some("tenant_maintenance_discovery_invalid_empty_tenant_id")
        }
        "tenant_maintenance_discovery_invalid: duplicate_tenant_id" => {
            Some("tenant_maintenance_discovery_invalid_duplicate_tenant_id")
        }
        "tenant_maintenance_registry_invalid: empty_registry" => {
            Some("tenant_maintenance_registry_invalid_empty_registry")
        }
        "tenant_maintenance_registry_invalid: empty_tenant_id" => {
            Some("tenant_maintenance_registry_invalid_empty_tenant_id")
        }
        "tenant_maintenance_registry_invalid: duplicate_tenant_id" => {
            Some("tenant_maintenance_registry_invalid_duplicate_tenant_id")
        }
        "tenant_maintenance_scheduled_sweep_busy" => {
            Some("tenant_maintenance_scheduled_sweep_busy")
        }
        "tenant_maintenance_scheduled_sweep_failed" => {
            Some("tenant_maintenance_scheduled_sweep_failed")
        }
        "tenant_maintenance_scheduled_sweep_lease_lost" => {
            Some("tenant_maintenance_scheduled_sweep_lease_lost")
        }
        "tenant_maintenance_outbox_retryable" => Some("tenant_maintenance_outbox_retryable"),
        "tenant_maintenance_outbox_stale" => Some("tenant_maintenance_outbox_stale"),
        "tenant_maintenance_outbox_failed" => Some("tenant_maintenance_outbox_failed"),
        "tenant_maintenance_outbox_exhausted" => Some("tenant_maintenance_outbox_exhausted"),
        _ => None,
    };
    if let Some(code) = public_code {
        return code;
    }

    if value.starts_with("tenant_maintenance_runtime_tick_failed:") {
        return "tenant_maintenance_runtime_tick_failed";
    }
    if value.starts_with("tenant_maintenance_runtime_stage_failed:") {
        return "tenant_maintenance_runtime_stage_failed";
    }
    if value.starts_with("tenant_maintenance_stage_failed:") {
        return "tenant_maintenance_stage_failed";
    }
    if value.starts_with("tenant_maintenance_registry_build_failed:") {
        return "tenant_maintenance_registry_build_failed";
    }
    "tenant_maintenance_failure"
}
