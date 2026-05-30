use super::*;
use oar_core::storage::postgres::StoredProposedActionDecisionKind;

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
fn postgres_live_review_inbox_snapshot_loads_related_safe_rows() {
    run_live_postgres_test("review_inbox_snapshot_related_rows", |pool| async move {
        seed_user(&pool, "tenant_snapshot", "user_snapshot").await?;
        let repository = PostgresReviewInboxRepository::new(pool.clone());

        repository
            .insert_evidence_item(
                "tenant_snapshot",
                &evidence_item(
                    "evidence_action_snapshot",
                    "Snapshot evidence summary",
                    "kr_snapshot",
                ),
            )
            .await?;
        let action = proposed_action("tenant_snapshot", "user_snapshot", "action_snapshot", 1);
        repository
            .insert_proposed_action(&action, Some(ms(1_748_250_020_000)))
            .await?;
        repository
            .insert_proposed_action_evidence_ref(
                "tenant_snapshot",
                "action_snapshot",
                1,
                "evidence_action_snapshot",
            )
            .await?;
        repository
            .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
                id: "decision_snapshot",
                tenant_id: "tenant_snapshot",
                proposed_action_id: "action_snapshot",
                proposed_action_version: 1,
                actor_user_id: "user_snapshot",
                decision: &ProposedActionDecision::Reject,
                confirmed_action_id: None,
                decided_at: ms(1_748_250_021_000),
            })
            .await?;
        repository
            .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                id: "inbox_snapshot",
                tenant_id: "tenant_snapshot",
                user_id: "user_snapshot",
                proposed_action_id: "action_snapshot",
                proposed_action_version: 1,
                sort_key: 300,
                sync_cursor: 303,
                status: ReviewInboxItemStatus::Open,
                ledger_status: None,
                operation_id: None,
            }))
            .await?;

        let snapshot = repository
            .load_review_inbox_snapshot("tenant_snapshot", "user_snapshot", 0, 10)
            .await?;

        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.actions.len(), 1);
        assert_eq!(snapshot.evidence.len(), 1);
        assert_eq!(snapshot.items[0].id, "inbox_snapshot");
        assert_eq!(snapshot.actions[0].review_item_id, "inbox_snapshot");
        assert_eq!(snapshot.actions[0].id, "action_snapshot");
        assert_eq!(
            snapshot.actions[0].evidence_ids,
            vec!["evidence_action_snapshot".to_string()]
        );
        assert_eq!(
            snapshot.actions[0]
                .decision
                .as_ref()
                .map(|decision| decision.decision),
            Some(StoredProposedActionDecisionKind::Reject)
        );
        assert_eq!(snapshot.evidence[0].review_item_id, "inbox_snapshot");
        assert_eq!(
            snapshot.evidence[0].item.summary,
            "Snapshot evidence summary"
        );
        Ok(())
    });
}
