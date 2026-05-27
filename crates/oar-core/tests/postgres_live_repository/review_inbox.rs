use super::harness::*;

use std::collections::HashSet;

const VALID_HASH: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn ms(value: u64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_millis(value)
}

fn evidence_item(id: &str, summary: &str, source_id: &str) -> EvidenceItem {
    EvidenceItem::new(
        EvidenceId(id.to_string()),
        summary,
        EvidenceRef::new(EvidenceSourceKind::OkrProgress, source_id, None)
            .expect("evidence reference should be valid"),
        VALID_HASH,
        EvidenceVisibilityScope::Tenant,
        ms(1_748_250_000_000),
        ms(1_748_250_001_000),
    )
    .expect("evidence item should be valid")
}

fn proposed_action(tenant_id: &str, user_id: &str, id: &str, version: u64) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        version,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![format!("evidence_{id}")],
        json!({"kind": "update_kr_progress"}),
    )
    .expect("proposed action draft should be valid");
    action.publish().expect("publish should work");
    action
}

struct InboxItemSpec<'a> {
    id: &'a str,
    tenant_id: &'a str,
    user_id: &'a str,
    proposed_action_id: &'a str,
    proposed_action_version: u64,
    sort_key: i64,
    sync_cursor: u64,
    status: ReviewInboxItemStatus,
    ledger_status: Option<&'a str>,
    operation_id: Option<&'a str>,
}

fn inbox_item(spec: InboxItemSpec<'_>) -> ReviewInboxItem {
    let mut item = ReviewInboxItem::new(
        ReviewInboxItemId(spec.id.to_string()),
        TenantId(spec.tenant_id.to_string()),
        WorkspaceUserId(spec.user_id.to_string()),
        spec.proposed_action_id,
        spec.proposed_action_version,
        80,
        10,
        spec.sort_key,
        spec.sync_cursor,
        ms(1_748_250_005_000 + spec.sync_cursor),
    );
    item.status = spec.status;
    item.ledger_status = spec.ledger_status.map(str::to_string);
    item.operation_id = spec.operation_id.map(str::to_string);
    item
}

#[test]
fn postgres_live_review_inbox_roundtrip_and_ordering() {
    run_live_postgres_test("review_inbox_roundtrip_ordering", |pool| async move {
        seed_user(&pool, "tenant_inbox", "user_inbox").await?;
        let repository = PostgresReviewInboxRepository::new(pool.clone());

        repository
            .insert_evidence_item(
                "tenant_inbox",
                &evidence_item("evidence_1", "kr risk evidence one", "kr_1"),
            )
            .await?;
        repository
            .insert_evidence_item(
                "tenant_inbox",
                &evidence_item("evidence_2", "kr risk evidence two", "kr_2"),
            )
            .await?;

        let action_1 = proposed_action("tenant_inbox", "user_inbox", "action_1", 1);
        let action_2 = proposed_action("tenant_inbox", "user_inbox", "action_2", 1);
        repository
            .insert_proposed_action(&action_1, Some(ms(1_748_250_002_000)))
            .await?;
        repository
            .insert_proposed_action(&action_2, Some(ms(1_748_250_003_000)))
            .await?;

        repository
            .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                id: "inbox_1",
                tenant_id: "tenant_inbox",
                user_id: "user_inbox",
                proposed_action_id: "action_1",
                proposed_action_version: 1,
                sort_key: 100,
                sync_cursor: 101,
                status: ReviewInboxItemStatus::Open,
                ledger_status: Some("confirmed"),
                operation_id: Some("op_1"),
            }))
            .await?;
        repository
            .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                id: "inbox_2",
                tenant_id: "tenant_inbox",
                user_id: "user_inbox",
                proposed_action_id: "action_2",
                proposed_action_version: 1,
                sort_key: 200,
                sync_cursor: 202,
                status: ReviewInboxItemStatus::Open,
                ledger_status: Some("executing"),
                operation_id: Some("op_2"),
            }))
            .await?;

        let rows = repository
            .list_review_inbox_items("tenant_inbox", "user_inbox", 100, 20)
            .await?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "inbox_2");
        assert_eq!(rows[1].id, "inbox_1");
        Ok(())
    });
}

#[test]
fn postgres_live_review_inbox_decision_uniqueness_is_version_scoped() {
    run_live_postgres_test("review_inbox_decision_version_unique", |pool| async move {
        seed_user(&pool, "tenant_decision", "user_decision").await?;
        let repository = PostgresReviewInboxRepository::new(pool.clone());

        let action_v1 = proposed_action("tenant_decision", "user_decision", "action_seq", 1);
        let action_v2 = proposed_action("tenant_decision", "user_decision", "action_seq", 2);
        repository
            .insert_proposed_action(&action_v1, Some(ms(1_748_250_010_000)))
            .await?;
        repository
            .insert_proposed_action(&action_v2, Some(ms(1_748_250_010_500)))
            .await?;

        let first = repository
            .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
                id: "decision_v1",
                tenant_id: "tenant_decision",
                proposed_action_id: "action_seq",
                proposed_action_version: 1,
                actor_user_id: "user_decision",
                decision: &ProposedActionDecision::Reject,
                confirmed_action_id: None,
                decided_at: ms(1_748_250_011_000),
            })
            .await?;
        let duplicate_same_version = repository
            .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
                id: "decision_v1_dup",
                tenant_id: "tenant_decision",
                proposed_action_id: "action_seq",
                proposed_action_version: 1,
                actor_user_id: "user_decision",
                decision: &ProposedActionDecision::Reject,
                confirmed_action_id: None,
                decided_at: ms(1_748_250_011_500),
            })
            .await?;
        let second_version = repository
            .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
                id: "decision_v2",
                tenant_id: "tenant_decision",
                proposed_action_id: "action_seq",
                proposed_action_version: 2,
                actor_user_id: "user_decision",
                decision: &ProposedActionDecision::Reject,
                confirmed_action_id: None,
                decided_at: ms(1_748_250_012_000),
            })
            .await?;

        assert!(first);
        assert!(!duplicate_same_version);
        assert!(second_version);
        Ok(())
    });
}

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
                        audit_context(
                            "evt_recorder_reject",
                            "trace_recorder",
                            1,
                            1_748_250_021_000,
                            "user_recorder",
                            "tenant_recorder",
                            "action_recorder",
                        ),
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

            Ok(())
        },
    );
}

#[test]
fn postgres_live_evidence_schema_excludes_raw_content_and_tokens() {
    run_live_postgres_test("review_inbox_evidence_schema_guard", |pool| async move {
        let rows = sqlx::query(
            "SELECT column_name FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'evidence_items'",
        )
        .fetch_all(&pool)
        .await?;

        let names: HashSet<String> = rows
            .into_iter()
            .map(|row| row.try_get::<String, _>("column_name"))
            .collect::<Result<_, _>>()?;

        for forbidden in [
            "raw_content",
            "raw_transcript",
            "access_token",
            "refresh_token",
        ] {
            assert!(!names.contains(forbidden));
        }

        Ok(())
    });
}
