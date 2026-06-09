use super::support::*;
use super::*;
use std::time::Duration;

#[test]
fn postgres_live_operational_recovery_requeue_failed_audit_outbox_is_confirmed_and_audited() {
    run_live_postgres_test(
        "operational_recovery_requeue_failed_audit_outbox",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_ops_recovery_outbox",
                "operator_ops_recovery_outbox",
            )
            .await?;

            let audit = PostgresAuditEventRepository::new(pool.clone());
            let source_outbox_id = audit
                .enqueue_outbox(
                    "tenant_ops_recovery_outbox",
                    "audit-events",
                    "trace_requeue_source",
                    &json!({
                        "trace_id": "trace_requeue_source",
                        "kind": "audit_delivery",
                        "sequence": 9
                    }),
                    1_748_263_000_000,
                )
                .await?;
            assert!(
                audit
                    .mark_outbox_failed("tenant_ops_recovery_outbox", source_outbox_id)
                    .await?
            );

            let requeue_next_attempt_at_ms = 1_748_263_100_000;
            let confirmed_at_ms = 1_748_263_200_000;
            let action = ConfirmedAction::proposed(
                "recovery-action-outbox-requeue",
                "tenant_ops_recovery_outbox",
                "operator_ops_recovery_outbox",
                "idem-recovery-outbox-requeue",
            )
            .confirm(UNIX_EPOCH + Duration::from_millis(confirmed_at_ms));
            let report = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action: action.clone(),
                    confirmed_at_ms,
                    operation_id: "op-recovery-outbox-requeue".to_string(),
                    occurred_at_ms: 1_748_263_300_000,
                    outbox_next_attempt_at_ms: 1_748_263_900_000,
                    audit_trace_id: "trace_recovery_outbox_requeue".to_string(),
                    kind: OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox {
                        outbox_id: source_outbox_id,
                        expected_attempt_count: 0,
                        requeue_next_attempt_at_ms,
                    },
                })
                .await?;

            assert_eq!(report.operation.status, ActionStatus::Succeeded);
            assert!(!report.duplicate);
            assert_eq!(
                report.recovered_target,
                Some(OperationalRecoveryExecutionTarget::AuditOutboxRequeue {
                    outbox_id: source_outbox_id
                })
            );
            assert_eq!(report.events.len(), 3);

            let state =
                audit_outbox_row_state(&pool, "tenant_ops_recovery_outbox", source_outbox_id)
                    .await?;
            assert_eq!(state.status, "pending");
            assert_eq!(state.attempt_count, 0);
            assert_eq!(
                state.next_attempt_at_ms,
                Some(requeue_next_attempt_at_ms as i64)
            );
            assert_eq!(state.sent_at_ms, None);

            assert_eq!(
                audit_event_operation_count(
                    &pool,
                    "tenant_ops_recovery_outbox",
                    "trace_recovery_outbox_requeue",
                    "op-recovery-outbox-requeue",
                )
                .await?,
                3
            );
            assert_eq!(
                audit_outbox_count_for_trace(
                    &pool,
                    "tenant_ops_recovery_outbox",
                    "trace_recovery_outbox_requeue",
                )
                .await?,
                3
            );

            let duplicate = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action,
                    confirmed_at_ms,
                    operation_id: "op-recovery-outbox-requeue".to_string(),
                    occurred_at_ms: 1_748_263_400_000,
                    outbox_next_attempt_at_ms: 1_748_263_900_000,
                    audit_trace_id: "trace_recovery_outbox_requeue".to_string(),
                    kind: OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox {
                        outbox_id: source_outbox_id,
                        expected_attempt_count: 0,
                        requeue_next_attempt_at_ms,
                    },
                })
                .await?;
            assert!(duplicate.duplicate);
            assert_eq!(duplicate.recovered_target, None);
            assert!(duplicate.events.is_empty());
            assert_eq!(
                audit_outbox_count_for_trace(
                    &pool,
                    "tenant_ops_recovery_outbox",
                    "trace_recovery_outbox_requeue",
                )
                .await?,
                3
            );

            let report_after_requeue = PostgresOperationalRecoveryRepository::new(pool.clone())
                .load_tenant_recovery_report("tenant_ops_recovery_outbox", 10)
                .await?;
            assert!(!report_after_requeue
                .failed_audit_outbox
                .iter()
                .any(|item| item.id == source_outbox_id));

            let claimed = audit
                .claim_outbox(
                    "tenant_ops_recovery_outbox",
                    "audit-events",
                    requeue_next_attempt_at_ms,
                    1,
                    requeue_next_attempt_at_ms + 30_000,
                )
                .await?;
            assert_eq!(claimed.len(), 1);
            assert_eq!(claimed[0].id, source_outbox_id);
            assert_eq!(claimed[0].attempt_count, 1);

            let debug = format!("{report:?}{duplicate:?}");
            assert_no_auth_refresh_sensitive_payload(&debug);
            assert!(!debug.contains("authorization"));
            assert!(!debug.contains("fingerprint"));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_operational_recovery_requeue_failed_audit_outbox_stale_guard_fails_closed() {
    run_live_postgres_test(
        "operational_recovery_requeue_failed_audit_outbox_stale",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_ops_recovery_outbox_stale",
                "operator_ops_recovery_outbox_stale",
            )
            .await?;

            let audit = PostgresAuditEventRepository::new(pool.clone());
            let source_outbox_id = audit
                .enqueue_outbox(
                    "tenant_ops_recovery_outbox_stale",
                    "audit-events",
                    "trace_requeue_stale_source",
                    &json!({
                        "trace_id": "trace_requeue_stale_source",
                        "kind": "audit_delivery",
                        "sequence": 11
                    }),
                    1_748_264_000_000,
                )
                .await?;
            assert!(
                audit
                    .mark_outbox_failed("tenant_ops_recovery_outbox_stale", source_outbox_id)
                    .await?
            );

            let confirmed_at_ms = 1_748_264_100_000;
            let action = ConfirmedAction::proposed(
                "recovery-action-outbox-stale",
                "tenant_ops_recovery_outbox_stale",
                "operator_ops_recovery_outbox_stale",
                "idem-recovery-outbox-stale",
            )
            .confirm(UNIX_EPOCH + Duration::from_millis(confirmed_at_ms));
            let report = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action,
                    confirmed_at_ms,
                    operation_id: "op-recovery-outbox-stale".to_string(),
                    occurred_at_ms: 1_748_264_200_000,
                    outbox_next_attempt_at_ms: 1_748_264_900_000,
                    audit_trace_id: "trace_recovery_outbox_stale".to_string(),
                    kind: OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox {
                        outbox_id: source_outbox_id,
                        expected_attempt_count: 99,
                        requeue_next_attempt_at_ms: 1_748_264_300_000,
                    },
                })
                .await?;

            assert_eq!(report.operation.status, ActionStatus::Failed);
            assert_eq!(report.recovered_target, None);
            assert_eq!(report.events.len(), 3);
            let state =
                audit_outbox_row_state(&pool, "tenant_ops_recovery_outbox_stale", source_outbox_id)
                    .await?;
            assert_eq!(state.status, "failed");
            assert_eq!(state.attempt_count, 0);
            assert_eq!(state.next_attempt_at_ms, None);
            assert_eq!(
                audit_event_operation_count(
                    &pool,
                    "tenant_ops_recovery_outbox_stale",
                    "trace_recovery_outbox_stale",
                    "op-recovery-outbox-stale",
                )
                .await?,
                3
            );

            Ok(())
        },
    );
}

#[test]
fn postgres_live_operational_recovery_requeue_failed_audit_outbox_unsafe_payload_fails_closed() {
    run_live_postgres_test(
        "operational_recovery_requeue_failed_audit_outbox_unsafe",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_ops_recovery_outbox_unsafe",
                "operator_ops_recovery_outbox_unsafe",
            )
            .await?;

            let source_outbox_id = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO audit_outbox (
                    tenant_id,
                    stream,
                    aggregate_id,
                    payload,
                    status,
                    attempt_count,
                    next_attempt_at
                )
                VALUES ($1, 'audit-events', $2, $3, 'failed', 3, NULL)
                RETURNING id
                "#,
            )
            .bind("tenant_ops_recovery_outbox_unsafe")
            .bind("trace_requeue_unsafe_source")
            .bind(json!({ "trace_id": "refresh_token should stay hidden" }))
            .fetch_one(&pool)
            .await?;

            let confirmed_at_ms = 1_748_265_100_000;
            let action = ConfirmedAction::proposed(
                "recovery-action-outbox-unsafe",
                "tenant_ops_recovery_outbox_unsafe",
                "operator_ops_recovery_outbox_unsafe",
                "idem-recovery-outbox-unsafe",
            )
            .confirm(UNIX_EPOCH + Duration::from_millis(confirmed_at_ms));
            let report = PostgresOperationalRecoveryRepository::new(pool.clone())
                .execute_confirmed_recovery(PostgresOperationalRecoveryExecutionRequest {
                    action,
                    confirmed_at_ms,
                    operation_id: "op-recovery-outbox-unsafe".to_string(),
                    occurred_at_ms: 1_748_265_200_000,
                    outbox_next_attempt_at_ms: 1_748_265_900_000,
                    audit_trace_id: "trace_recovery_outbox_unsafe".to_string(),
                    kind: OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox {
                        outbox_id: source_outbox_id,
                        expected_attempt_count: 3,
                        requeue_next_attempt_at_ms: 1_748_265_300_000,
                    },
                })
                .await?;

            assert_eq!(report.operation.status, ActionStatus::Failed);
            assert_eq!(report.recovered_target, None);
            assert_eq!(report.events.len(), 3);
            let state = audit_outbox_row_state(
                &pool,
                "tenant_ops_recovery_outbox_unsafe",
                source_outbox_id,
            )
            .await?;
            assert_eq!(state.status, "failed");
            assert_eq!(state.attempt_count, 3);
            assert_eq!(state.next_attempt_at_ms, None);

            let debug = format!("{report:?}");
            assert!(!debug.contains("refresh_token should stay hidden"));
            assert_no_auth_refresh_sensitive_payload(&debug);

            Ok(())
        },
    );
}
