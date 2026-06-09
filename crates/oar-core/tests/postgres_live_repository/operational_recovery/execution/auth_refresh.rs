use super::support::*;
use super::*;
use std::time::Duration;

#[test]
fn postgres_live_operational_recovery_resume_paused_auth_refresh_is_confirmed_and_audited() {
    run_live_postgres_test(
        "operational_recovery_resume_paused_auth_refresh",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_ops_recovery_resume",
                "operator_ops_recovery_resume",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_ops_recovery_resume",
                "identity_ops_recovery_resume",
            )
            .await?;

            let grants = PostgresTokenGrantRepository::new(pool.clone());
            grants
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_ops_recovery_resume",
                    "grant_ops_resume_config",
                    "identity_ops_recovery_resume",
                    TokenGrantState::NeedsRefresh,
                    "fp-resume-config",
                ))
                .await?;
            grants
                .mark_refresh_failed(
                    "tenant_ops_recovery_resume",
                    "grant_ops_resume_config",
                    "fp-resume-config",
                    1_748_261_000_000,
                    "refresh_config_required",
                )
                .await?
                .expect("config-required grant should update");
            let expected_updated_at_ms = token_grant_updated_at_ms(
                &pool,
                "tenant_ops_recovery_resume",
                "grant_ops_resume_config",
            )
            .await?;

            let confirmed_at_ms = 1_748_261_100_000;
            let action = ConfirmedAction::proposed(
                "recovery-action-resume",
                "tenant_ops_recovery_resume",
                "operator_ops_recovery_resume",
                "idem-recovery-resume",
            )
            .confirm(UNIX_EPOCH + Duration::from_millis(confirmed_at_ms));
            let report = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action: action.clone(),
                    confirmed_at_ms,
                    operation_id: "op-recovery-resume".to_string(),
                    occurred_at_ms: 1_748_261_200_000,
                    outbox_next_attempt_at_ms: 1_748_261_200_000,
                    audit_trace_id: "trace_recovery_resume".to_string(),
                    kind: OperationalRecoveryExecutionKind::ResumePausedAuthRefresh {
                        grant_id: "grant_ops_resume_config".to_string(),
                        expected_updated_at_ms,
                    },
                })
                .await?;

            assert_eq!(report.operation.status, ActionStatus::Succeeded);
            assert!(!report.duplicate);
            assert_eq!(
                report.recovered_target,
                Some(OperationalRecoveryExecutionTarget::TokenGrantRefresh {
                    grant_id: "grant_ops_resume_config".to_string()
                })
            );
            assert_eq!(report.events.len(), 3);

            let resumed = grants
                .get_by_id("tenant_ops_recovery_resume", "grant_ops_resume_config")
                .await?
                .expect("resumed grant should still exist");
            assert_eq!(resumed.last_refresh_error, None);
            assert_eq!(resumed.state, TokenGrantState::NeedsRefresh);
            assert_eq!(resumed.oauth_grant_fingerprint, "fp-resume-config");

            let candidates = grants
                .list_refresh_candidate_snapshots(
                    "tenant_ops_recovery_resume",
                    UNIX_EPOCH + Duration::from_millis(1_748_261_300_000),
                    10,
                )
                .await?;
            assert!(candidates
                .iter()
                .any(|candidate| candidate.grant_id.0 == "grant_ops_resume_config"));

            let audit = PostgresAuditEventRepository::new(pool.clone())
                .find_by_tenant_and_trace_id("tenant_ops_recovery_resume", "trace_recovery_resume")
                .await?;
            assert_eq!(audit.len(), 3);
            let audited_operation_count = audit_event_operation_count(
                &pool,
                "tenant_ops_recovery_resume",
                "trace_recovery_resume",
                "op-recovery-resume",
            )
            .await?;
            assert_eq!(audited_operation_count, 3);
            let outbox_count = audit_outbox_count_for_trace(
                &pool,
                "tenant_ops_recovery_resume",
                "trace_recovery_resume",
            )
            .await?;
            assert_eq!(outbox_count, 3);

            let duplicate = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action,
                    confirmed_at_ms,
                    operation_id: "op-recovery-resume".to_string(),
                    occurred_at_ms: 1_748_261_400_000,
                    outbox_next_attempt_at_ms: 1_748_261_400_000,
                    audit_trace_id: "trace_recovery_resume".to_string(),
                    kind: OperationalRecoveryExecutionKind::ResumePausedAuthRefresh {
                        grant_id: "grant_ops_resume_config".to_string(),
                        expected_updated_at_ms,
                    },
                })
                .await?;
            assert!(duplicate.duplicate);
            assert_eq!(duplicate.recovered_target, None);
            assert!(duplicate.events.is_empty());
            assert_eq!(
                audit_outbox_count_for_trace(
                    &pool,
                    "tenant_ops_recovery_resume",
                    "trace_recovery_resume",
                )
                .await?,
                3
            );

            let debug = format!("{report:?}{duplicate:?}");
            assert_no_auth_refresh_sensitive_payload(&debug);
            assert!(!debug.contains("fingerprint"));
            assert!(!debug.contains("encrypted"));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_operational_recovery_resume_paused_auth_refresh_stale_guard_fails_closed() {
    run_live_postgres_test(
        "operational_recovery_resume_paused_auth_refresh_stale",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_ops_recovery_stale",
                "operator_ops_recovery_stale",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_ops_recovery_stale",
                "identity_ops_recovery_stale",
            )
            .await?;

            let grants = PostgresTokenGrantRepository::new(pool.clone());
            grants
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_ops_recovery_stale",
                    "grant_ops_resume_stale",
                    "identity_ops_recovery_stale",
                    TokenGrantState::NeedsRefresh,
                    "fp-resume-stale",
                ))
                .await?;
            grants
                .mark_refresh_failed(
                    "tenant_ops_recovery_stale",
                    "grant_ops_resume_stale",
                    "fp-resume-stale",
                    1_748_262_000_000,
                    "refresh_config_required",
                )
                .await?
                .expect("config-required grant should update");

            let confirmed_at_ms = 1_748_262_100_000;
            let action = ConfirmedAction::proposed(
                "recovery-action-stale",
                "tenant_ops_recovery_stale",
                "operator_ops_recovery_stale",
                "idem-recovery-stale",
            )
            .confirm(UNIX_EPOCH + Duration::from_millis(confirmed_at_ms));
            let report = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action,
                    confirmed_at_ms,
                    operation_id: "op-recovery-stale".to_string(),
                    occurred_at_ms: 1_748_262_200_000,
                    outbox_next_attempt_at_ms: 1_748_262_200_000,
                    audit_trace_id: "trace_recovery_stale".to_string(),
                    kind: OperationalRecoveryExecutionKind::ResumePausedAuthRefresh {
                        grant_id: "grant_ops_resume_stale".to_string(),
                        expected_updated_at_ms: 1,
                    },
                })
                .await?;

            assert_eq!(report.operation.status, ActionStatus::Failed);
            assert_eq!(report.recovered_target, None);
            assert_eq!(report.events.len(), 3);
            let unchanged = grants
                .get_by_id("tenant_ops_recovery_stale", "grant_ops_resume_stale")
                .await?
                .expect("stale guarded grant should still exist");
            assert_eq!(
                unchanged.last_refresh_error.as_deref(),
                Some("refresh_config_required")
            );
            assert_eq!(
                audit_event_operation_count(
                    &pool,
                    "tenant_ops_recovery_stale",
                    "trace_recovery_stale",
                    "op-recovery-stale",
                )
                .await?,
                3
            );

            Ok(())
        },
    );
}
