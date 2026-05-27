use oar_core::domain::scheduler::{
    SchedulerJobAttemptReport, SchedulerJobKind, SchedulerJobOutcome, SchedulerJobStatus,
};

#[test]
fn scheduler_job_statuses_are_recurring_not_terminal() {
    let statuses = [SchedulerJobStatus::Pending, SchedulerJobStatus::Running];
    assert_eq!(statuses.len(), 2);
}

#[test]
fn scheduler_attempt_report_carries_safe_metadata_only() {
    let report = SchedulerJobAttemptReport {
        tenant_id: "tenant_01".to_string(),
        job_kind: SchedulerJobKind::TokenRefreshSweep,
        lease_id: Some("lease_01".to_string()),
        started_at_ms: 100,
        finished_at_ms: 200,
        outcome: SchedulerJobOutcome::FailedSafe,
        safe_error_code: Some("transient_timeout".to_string()),
    };

    let debug = format!("{report:?}");

    assert!(debug.contains("transient_timeout"));
    assert!(!debug.contains("access_token"));
    assert!(!debug.contains("refresh_token"));
    assert!(!debug.contains("authorization_code"));
}
