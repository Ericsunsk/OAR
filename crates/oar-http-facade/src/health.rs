use hyper::http::StatusCode;
use serde_json::json;

use crate::response::{json_facade_response, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::tenant_maintenance_daemon_status::TenantMaintenanceDaemonStatusSnapshot;

pub(crate) fn healthz_response(runtime: Option<&OarHttpFacadeRuntime>) -> FacadeResponse {
    let tenant_maintenance = tenant_maintenance_snapshot(runtime);
    let body = json!({
        "status": "ok",
        "service": "oar-http-facade",
        "tenant_maintenance": tenant_maintenance_health_value(&tenant_maintenance),
    });
    json_facade_response(StatusCode::OK, body)
}

pub(crate) fn readyz_response(runtime: Option<&OarHttpFacadeRuntime>) -> FacadeResponse {
    let tenant_maintenance = tenant_maintenance_snapshot(runtime);
    let readiness = tenant_maintenance_readiness(&tenant_maintenance);
    let body = json!({
        "status": readiness.status,
        "service": "oar-http-facade",
        "reason_code": readiness.reason_code,
        "tenant_maintenance": tenant_maintenance_health_value(&tenant_maintenance),
    });
    json_facade_response(readiness.status_code, body)
}

struct ReadinessStatus {
    status_code: StatusCode,
    status: &'static str,
    reason_code: &'static str,
}

fn tenant_maintenance_snapshot(
    runtime: Option<&OarHttpFacadeRuntime>,
) -> TenantMaintenanceDaemonStatusSnapshot {
    runtime
        .map(|runtime| runtime.tenant_maintenance_daemon_status().snapshot())
        .unwrap_or_else(TenantMaintenanceDaemonStatusSnapshot::disabled)
}

fn tenant_maintenance_health_value(
    snapshot: &TenantMaintenanceDaemonStatusSnapshot,
) -> serde_json::Value {
    json!({
        "enabled": snapshot.enabled,
        "daemon_state": snapshot.state,
        "successful_rounds": snapshot.successful_rounds,
        "failed_rounds": snapshot.failed_rounds,
        "failed_tenant_ticks": snapshot.failed_tenant_ticks,
        "daemon_failures": snapshot.daemon_failures,
        "last_round_status": snapshot.last_round_status,
        "last_round_tenant_count": snapshot.last_round_tenant_count,
        "last_round_failed_tenant_count": snapshot.last_round_failed_tenant_count,
        "last_failure_code": snapshot.last_failure_code,
        "last_daemon_failure_code": snapshot.last_daemon_failure_code,
        "stages": {
            "scheduled_sweep": {
                "successful_runs": snapshot.stages.scheduled_sweep.successful_runs,
                "degraded_runs": snapshot.stages.scheduled_sweep.degraded_runs,
                "failed_runs": snapshot.stages.scheduled_sweep.failed_runs,
                "last_status": snapshot.stages.scheduled_sweep.last_status,
                "last_outcome": snapshot.stages.scheduled_sweep.last_outcome,
                "last_candidate_count": snapshot.stages.scheduled_sweep.last_candidate_count,
                "last_attempted_count": snapshot.stages.scheduled_sweep.last_attempted_count,
                "last_has_more": snapshot.stages.scheduled_sweep.last_has_more,
                "last_failure_code": snapshot.stages.scheduled_sweep.last_failure_code,
            },
            "outbox_drain": {
                "successful_runs": snapshot.stages.outbox_drain.successful_runs,
                "degraded_runs": snapshot.stages.outbox_drain.degraded_runs,
                "failed_runs": snapshot.stages.outbox_drain.failed_runs,
                "last_status": snapshot.stages.outbox_drain.last_status,
                "last_claimed": snapshot.stages.outbox_drain.last_claimed,
                "last_sent": snapshot.stages.outbox_drain.last_sent,
                "last_retryable": snapshot.stages.outbox_drain.last_retryable,
                "last_failed": snapshot.stages.outbox_drain.last_failed,
                "last_exhausted": snapshot.stages.outbox_drain.last_exhausted,
                "last_stale": snapshot.stages.outbox_drain.last_stale,
                "last_failure_code": snapshot.stages.outbox_drain.last_failure_code,
            },
        },
    })
}

fn tenant_maintenance_readiness(
    snapshot: &TenantMaintenanceDaemonStatusSnapshot,
) -> ReadinessStatus {
    if !snapshot.enabled {
        return ready("tenant_maintenance_disabled");
    }

    match snapshot.state {
        "running" => {}
        "configured" => return not_ready("tenant_maintenance_daemon_not_started"),
        "stopped" => return not_ready("tenant_maintenance_daemon_stopped"),
        "failed" => return not_ready("tenant_maintenance_daemon_failed"),
        _ => return not_ready("tenant_maintenance_daemon_unknown"),
    }

    if snapshot.last_round_status == Some("failed") {
        return not_ready("tenant_maintenance_round_failed");
    }
    if snapshot.stages.scheduled_sweep.last_status == Some("failed") {
        return not_ready("tenant_maintenance_scheduled_sweep_failed");
    }
    if snapshot.stages.outbox_drain.last_status == Some("failed") {
        return not_ready("tenant_maintenance_outbox_drain_failed");
    }

    ready("tenant_maintenance_ready")
}

fn ready(reason_code: &'static str) -> ReadinessStatus {
    ReadinessStatus {
        status_code: StatusCode::OK,
        status: "ok",
        reason_code,
    }
}

fn not_ready(reason_code: &'static str) -> ReadinessStatus {
    ReadinessStatus {
        status_code: StatusCode::SERVICE_UNAVAILABLE,
        status: "not_ready",
        reason_code,
    }
}
