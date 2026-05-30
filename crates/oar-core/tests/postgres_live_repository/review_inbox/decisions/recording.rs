use super::super::*;
use oar_core::storage::postgres::{StoredReviewInboxLedgerStage, StoredReviewInboxLedgerStatus};

#[test]
fn postgres_live_review_decision_recorder_confirm_and_reject() {
    run_live_postgres_test(
        "review_decision_recorder_confirm_reject",
        |pool| async move {
            seed_user(&pool, "tenant_recorder", "user_recorder").await?;
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let recorder = PostgresReviewDecisionRecorder::new(pool.clone());

            let action = proposed_action("tenant_recorder", "user_recorder", "action_recorder", 1);
            repository
                .insert_proposed_action(&action, Some(ms(1_748_250_020_000)))
                .await?;
            repository
                .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                    id: "inbox_recorder",
                    tenant_id: "tenant_recorder",
                    user_id: "user_recorder",
                    proposed_action_id: "action_recorder",
                    proposed_action_version: 1,
                    sort_key: 500,
                    sync_cursor: 300,
                    status: ReviewInboxItemStatus::Open,
                    ledger_status: None,
                    operation_id: None,
                }))
                .await?;

            let report = recorder
                .record_decision(PostgresReviewDecisionRecorderRequest {
                    expected_sync_cursor_value: 300,
                    decision: InsertProposedActionDecisionRequest {
                        id: "decision_recorder_reject",
                        tenant_id: "tenant_recorder",
                        proposed_action_id: "action_recorder",
                        proposed_action_version: 1,
                        actor_user_id: "user_recorder",
                        decision: &ProposedActionDecision::Reject,
                        confirmed_action_id: None,
                        decided_at: ms(1_748_250_021_000),
                    },
                    confirmed_action: None,
                    confirmed_at_ms: None,
                    operation_id: None,
                    inbox_item: &inbox_item(InboxItemSpec {
                        id: "inbox_recorder",
                        tenant_id: "tenant_recorder",
                        user_id: "user_recorder",
                        proposed_action_id: "action_recorder",
                        proposed_action_version: 1,
                        sort_key: 500,
                        sync_cursor: 301,
                        status: ReviewInboxItemStatus::Rejected,
                        ledger_status: None,
                        operation_id: None,
                    }),
                    event: &AuditEvent::proposed_action_decision(
                        review_decision_audit_context(ReviewDecisionAuditSpec {
                            event_id: "evt_recorder_reject",
                            trace_id: "trace_recorder",
                            sequence: 1,
                            occurred_at_ms: 1_748_250_021_000,
                            actor_id: "user_recorder",
                            tenant_id: "tenant_recorder",
                            action_id: "action_recorder",
                            action_type: "reject",
                        }),
                        summary("reject decision"),
                    ),
                    outbox: &outbox_envelope(
                        "tenant_recorder",
                        "trace_recorder",
                        1_748_250_022_000,
                    ),
                })
                .await?;
            assert!(!report.duplicate);
            assert_eq!(report.operation, None);

            let confirm_action = proposed_action(
                "tenant_recorder",
                "user_recorder",
                "action_recorder_confirm",
                1,
            );
            repository
                .insert_proposed_action(&confirm_action, Some(ms(1_748_250_023_000)))
                .await?;
            repository
                .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                    id: "inbox_recorder_confirm",
                    tenant_id: "tenant_recorder",
                    user_id: "user_recorder",
                    proposed_action_id: "action_recorder_confirm",
                    proposed_action_version: 1,
                    sort_key: 501,
                    sync_cursor: 400,
                    status: ReviewInboxItemStatus::Open,
                    ledger_status: None,
                    operation_id: None,
                }))
                .await?;
            let mut confirm_action_for_decision = confirm_action.clone();
            let confirmed_action = confirm_action_for_decision
                .decide(ProposedActionDecision::Confirm, ms(1_748_250_024_000))
                .expect("confirm decision should be valid")
                .expect("confirm decision should create confirmed action");
            let operation_id = format!("op-{}", confirmed_action.idempotency_key);
            let report = recorder
                .record_decision(PostgresReviewDecisionRecorderRequest {
                    expected_sync_cursor_value: 400,
                    decision: InsertProposedActionDecisionRequest {
                        id: "decision_recorder_confirm",
                        tenant_id: "tenant_recorder",
                        proposed_action_id: "action_recorder_confirm",
                        proposed_action_version: 1,
                        actor_user_id: "user_recorder",
                        decision: &ProposedActionDecision::Confirm,
                        confirmed_action_id: Some(&confirmed_action.action_id),
                        decided_at: ms(1_748_250_024_000),
                    },
                    confirmed_action: Some(&confirmed_action),
                    confirmed_at_ms: Some(1_748_250_024_000),
                    operation_id: Some(&operation_id),
                    inbox_item: &inbox_item(InboxItemSpec {
                        id: "inbox_recorder_confirm",
                        tenant_id: "tenant_recorder",
                        user_id: "user_recorder",
                        proposed_action_id: "action_recorder_confirm",
                        proposed_action_version: 1,
                        sort_key: 501,
                        sync_cursor: 401,
                        status: ReviewInboxItemStatus::Confirmed,
                        ledger_status: Some("confirmed"),
                        operation_id: Some(&operation_id),
                    }),
                    event: &AuditEvent::proposed_action_decision(
                        review_decision_audit_context(ReviewDecisionAuditSpec {
                            event_id: "evt_recorder_confirm",
                            trace_id: "trace_recorder_confirm",
                            sequence: 1,
                            occurred_at_ms: 1_748_250_024_000,
                            actor_id: "user_recorder",
                            tenant_id: "tenant_recorder",
                            action_id: "action_recorder_confirm",
                            action_type: "confirm",
                        }),
                        summary("confirm decision"),
                    ),
                    outbox: &outbox_envelope(
                        "tenant_recorder",
                        "trace_recorder_confirm",
                        1_748_250_024_500,
                    ),
                })
                .await?;
            assert!(!report.duplicate);
            assert_eq!(
                report.inbox_item_id.as_deref(),
                Some("inbox_recorder_confirm")
            );
            assert_eq!(
                report.operation.as_ref().map(|operation| operation.status),
                Some(ActionStatus::Confirmed)
            );

            let projected = sqlx::query(
                r#"
                SELECT status, ledger_status, operation_id
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder")
            .bind("inbox_recorder_confirm")
            .fetch_one(&pool)
            .await?;
            assert_eq!(projected.try_get::<String, _>("status")?, "confirmed");
            assert_eq!(
                projected.try_get::<Option<String>, _>("ledger_status")?,
                Some("confirmed".to_string())
            );
            assert_eq!(
                projected.try_get::<Option<String>, _>("operation_id")?,
                Some(operation_id)
            );

            let snapshot = repository
                .load_review_inbox_snapshot("tenant_recorder", "user_recorder", 0, 10)
                .await?;
            let reject_events = ledger_stages_for(&snapshot, "action_recorder");
            let confirm_events = ledger_stages_for(&snapshot, "action_recorder_confirm");

            assert_eq!(
                reject_events,
                vec![
                    (
                        StoredReviewInboxLedgerStage::ConfirmedAction,
                        StoredReviewInboxLedgerStatus::Error
                    ),
                    (
                        StoredReviewInboxLedgerStage::AuditEvent,
                        StoredReviewInboxLedgerStatus::Ok
                    ),
                ]
            );
            assert_eq!(
                confirm_events,
                vec![
                    (
                        StoredReviewInboxLedgerStage::ConfirmedAction,
                        StoredReviewInboxLedgerStatus::Ok
                    ),
                    (
                        StoredReviewInboxLedgerStage::OperationLedger,
                        StoredReviewInboxLedgerStatus::Ok
                    ),
                    (
                        StoredReviewInboxLedgerStage::AuditEvent,
                        StoredReviewInboxLedgerStatus::Ok
                    ),
                ]
            );

            Ok(())
        },
    );
}

fn ledger_stages_for(
    snapshot: &oar_core::storage::postgres::StoredReviewInboxSnapshot,
    action_id: &str,
) -> Vec<(StoredReviewInboxLedgerStage, StoredReviewInboxLedgerStatus)> {
    snapshot
        .ledger_events
        .iter()
        .filter(|event| event.action_id == action_id)
        .map(|event| (event.stage, event.stage_status))
        .collect()
}

struct ReviewDecisionAuditSpec<'a> {
    event_id: &'a str,
    trace_id: &'a str,
    sequence: u64,
    occurred_at_ms: u64,
    actor_id: &'a str,
    tenant_id: &'a str,
    action_id: &'a str,
    action_type: &'a str,
}

fn review_decision_audit_context(spec: ReviewDecisionAuditSpec<'_>) -> AuditEventContext {
    AuditEventContext {
        event_id: spec.event_id.to_string(),
        trace_id: spec.trace_id.to_string(),
        sequence: spec.sequence,
        occurred_at_ms: spec.occurred_at_ms,
        subject: AuditSubject {
            actor: actor(spec.actor_id),
            scope: scope(spec.tenant_id),
            target: AuditTarget {
                resource_type: "proposed_action".to_string(),
                resource_id: spec.action_id.to_string(),
                action_type: spec.action_type.to_string(),
            },
        },
    }
}
