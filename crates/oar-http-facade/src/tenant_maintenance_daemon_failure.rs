#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TenantMaintenanceDaemonFailureCode {
    DaemonStoppedUnexpectedly,
    DaemonTaskFailed,
    DaemonMissingPersistence,
    DaemonMissingFeishuAuth,
    DaemonRuntimeConfigInvalid,
    DaemonWorkerConfigInvalid,
    DaemonRefreshAdapterBuildFailed,
    DaemonWebhookSinkBuildFailed,
    DiscoveryInvalidEmptyRegistry,
    DiscoveryInvalidEmptyTenantId,
    DiscoveryInvalidDuplicateTenantId,
    DiscoveryFailed,
    RegistryInvalidEmptyRegistry,
    RegistryInvalidEmptyTenantId,
    RegistryInvalidDuplicateTenantId,
    ScheduledSweepBusy,
    ScheduledSweepFailed,
    ScheduledSweepLeaseLost,
    OutboxRetryable,
    OutboxStale,
    OutboxFailed,
    OutboxExhausted,
    RuntimeTickFailed,
    RuntimeStageFailed,
    StageFailed,
    RegistryBuildFailed,
    UnknownFailure,
}

impl TenantMaintenanceDaemonFailureCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DaemonStoppedUnexpectedly => "tenant_maintenance_daemon_stopped_unexpectedly",
            Self::DaemonTaskFailed => "tenant_maintenance_daemon_task_failed",
            Self::DaemonMissingPersistence => "tenant_maintenance_daemon_missing_persistence",
            Self::DaemonMissingFeishuAuth => "tenant_maintenance_daemon_missing_feishu_auth",
            Self::DaemonRuntimeConfigInvalid => "tenant_maintenance_daemon_runtime_config_invalid",
            Self::DaemonWorkerConfigInvalid => "tenant_maintenance_daemon_worker_config_invalid",
            Self::DaemonRefreshAdapterBuildFailed => {
                "tenant_maintenance_daemon_refresh_adapter_build_failed"
            }
            Self::DaemonWebhookSinkBuildFailed => {
                "tenant_maintenance_daemon_webhook_sink_build_failed"
            }
            Self::DiscoveryInvalidEmptyRegistry => {
                "tenant_maintenance_discovery_invalid_empty_registry"
            }
            Self::DiscoveryInvalidEmptyTenantId => {
                "tenant_maintenance_discovery_invalid_empty_tenant_id"
            }
            Self::DiscoveryInvalidDuplicateTenantId => {
                "tenant_maintenance_discovery_invalid_duplicate_tenant_id"
            }
            Self::DiscoveryFailed => "tenant_maintenance_discovery_failed",
            Self::RegistryInvalidEmptyRegistry => {
                "tenant_maintenance_registry_invalid_empty_registry"
            }
            Self::RegistryInvalidEmptyTenantId => {
                "tenant_maintenance_registry_invalid_empty_tenant_id"
            }
            Self::RegistryInvalidDuplicateTenantId => {
                "tenant_maintenance_registry_invalid_duplicate_tenant_id"
            }
            Self::ScheduledSweepBusy => "tenant_maintenance_scheduled_sweep_busy",
            Self::ScheduledSweepFailed => "tenant_maintenance_scheduled_sweep_failed",
            Self::ScheduledSweepLeaseLost => "tenant_maintenance_scheduled_sweep_lease_lost",
            Self::OutboxRetryable => "tenant_maintenance_outbox_retryable",
            Self::OutboxStale => "tenant_maintenance_outbox_stale",
            Self::OutboxFailed => "tenant_maintenance_outbox_failed",
            Self::OutboxExhausted => "tenant_maintenance_outbox_exhausted",
            Self::RuntimeTickFailed => "tenant_maintenance_runtime_tick_failed",
            Self::RuntimeStageFailed => "tenant_maintenance_runtime_stage_failed",
            Self::StageFailed => "tenant_maintenance_stage_failed",
            Self::RegistryBuildFailed => "tenant_maintenance_registry_build_failed",
            Self::UnknownFailure => "tenant_maintenance_failure",
        }
    }

    pub(crate) fn runtime_tick_safe_error(reason: &'static str) -> String {
        format!("{}: {}", Self::RuntimeTickFailed.as_str(), reason)
    }

    pub(crate) fn runtime_stage_safe_error(stage: &'static str) -> String {
        format!("{}: {}", Self::RuntimeStageFailed.as_str(), stage)
    }
}

pub(crate) fn classify_failure_code(value: impl AsRef<str>) -> TenantMaintenanceDaemonFailureCode {
    let value = value.as_ref().trim();
    let public_code = match value {
        "tenant_maintenance_daemon_stopped_unexpectedly" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonStoppedUnexpectedly)
        }
        "tenant_maintenance_daemon_task_failed" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonTaskFailed)
        }
        "tenant_maintenance_daemon_missing_persistence" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonMissingPersistence)
        }
        "tenant_maintenance_daemon_missing_feishu_auth" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonMissingFeishuAuth)
        }
        "tenant_maintenance_daemon_runtime_config_invalid" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonRuntimeConfigInvalid)
        }
        "tenant_maintenance_daemon_worker_config_invalid" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonWorkerConfigInvalid)
        }
        "tenant_maintenance_daemon_refresh_adapter_build_failed" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonRefreshAdapterBuildFailed)
        }
        "tenant_maintenance_daemon_webhook_sink_build_failed" => {
            Some(TenantMaintenanceDaemonFailureCode::DaemonWebhookSinkBuildFailed)
        }
        "tenant_maintenance_discovery_invalid: empty_registry" => {
            Some(TenantMaintenanceDaemonFailureCode::DiscoveryInvalidEmptyRegistry)
        }
        "tenant_maintenance_discovery_invalid: empty_tenant_id" => {
            Some(TenantMaintenanceDaemonFailureCode::DiscoveryInvalidEmptyTenantId)
        }
        "tenant_maintenance_discovery_invalid: duplicate_tenant_id" => {
            Some(TenantMaintenanceDaemonFailureCode::DiscoveryInvalidDuplicateTenantId)
        }
        "tenant_maintenance_registry_invalid: empty_registry" => {
            Some(TenantMaintenanceDaemonFailureCode::RegistryInvalidEmptyRegistry)
        }
        "tenant_maintenance_registry_invalid: empty_tenant_id" => {
            Some(TenantMaintenanceDaemonFailureCode::RegistryInvalidEmptyTenantId)
        }
        "tenant_maintenance_registry_invalid: duplicate_tenant_id" => {
            Some(TenantMaintenanceDaemonFailureCode::RegistryInvalidDuplicateTenantId)
        }
        "tenant_maintenance_scheduled_sweep_busy" => {
            Some(TenantMaintenanceDaemonFailureCode::ScheduledSweepBusy)
        }
        "tenant_maintenance_scheduled_sweep_failed" => {
            Some(TenantMaintenanceDaemonFailureCode::ScheduledSweepFailed)
        }
        "tenant_maintenance_scheduled_sweep_lease_lost" => {
            Some(TenantMaintenanceDaemonFailureCode::ScheduledSweepLeaseLost)
        }
        "tenant_maintenance_outbox_retryable" => {
            Some(TenantMaintenanceDaemonFailureCode::OutboxRetryable)
        }
        "tenant_maintenance_outbox_stale" => Some(TenantMaintenanceDaemonFailureCode::OutboxStale),
        "tenant_maintenance_outbox_failed" => {
            Some(TenantMaintenanceDaemonFailureCode::OutboxFailed)
        }
        "tenant_maintenance_outbox_exhausted" => {
            Some(TenantMaintenanceDaemonFailureCode::OutboxExhausted)
        }
        _ => None,
    };
    if let Some(code) = public_code {
        return code;
    }

    if value.starts_with("tenant_maintenance_runtime_tick_failed:") {
        return TenantMaintenanceDaemonFailureCode::RuntimeTickFailed;
    }
    if value.starts_with("tenant_maintenance_runtime_stage_failed:") {
        return TenantMaintenanceDaemonFailureCode::RuntimeStageFailed;
    }
    if value == "tenant_discovery_failed" || value.starts_with("tenant_discovery_failed:") {
        return TenantMaintenanceDaemonFailureCode::DiscoveryFailed;
    }
    if value.starts_with("tenant_maintenance_stage_failed:") {
        return TenantMaintenanceDaemonFailureCode::StageFailed;
    }
    if value.starts_with("tenant_maintenance_registry_build_failed:") {
        return TenantMaintenanceDaemonFailureCode::RegistryBuildFailed;
    }
    TenantMaintenanceDaemonFailureCode::UnknownFailure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_maps_known_daemon_lifecycle_code() {
        let code = classify_failure_code("tenant_maintenance_daemon_task_failed");

        assert_eq!(code, TenantMaintenanceDaemonFailureCode::DaemonTaskFailed);
        assert_eq!(code.as_str(), "tenant_maintenance_daemon_task_failed");
    }

    #[test]
    fn classifier_redacts_unknown_failure_code() {
        let code = classify_failure_code("refresh_token webhook-secret tenant_secret_id");

        assert_eq!(code, TenantMaintenanceDaemonFailureCode::UnknownFailure);
        assert_eq!(code.as_str(), "tenant_maintenance_failure");
    }

    #[test]
    fn classifier_maps_discovery_failure_without_raw_reason() {
        let code = classify_failure_code("tenant_discovery_failed: unknown_tenant_status");

        assert_eq!(code, TenantMaintenanceDaemonFailureCode::DiscoveryFailed);
        assert_eq!(code.as_str(), "tenant_maintenance_discovery_failed");
        assert!(!code.as_str().contains("unknown_tenant_status"));
    }
}
