use super::harness::*;

fn execution_projection_proposed_action(
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        1,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![format!("evidence_{id}")],
        json!({"kind": "update_kr_progress", "delta": "weekly"}),
    )
    .expect("proposed action should be valid");
    action.publish().expect("publish should work");
    action
}

struct ProjectionInboxSpec<'a> {
    id: &'a str,
    tenant_id: &'a str,
    user_id: &'a str,
    proposed_action_id: &'a str,
    status: ReviewInboxItemStatus,
    ledger_status: Option<&'a str>,
    operation_id: Option<&'a str>,
    sync_cursor: u64,
}

fn execution_projection_inbox_item(spec: ProjectionInboxSpec<'_>) -> ReviewInboxItem {
    let mut item = ReviewInboxItem::new(
        ReviewInboxItemId(spec.id.to_string()),
        TenantId(spec.tenant_id.to_string()),
        WorkspaceUserId(spec.user_id.to_string()),
        spec.proposed_action_id,
        1,
        80,
        3,
        900,
        spec.sync_cursor,
        SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(spec.sync_cursor),
    );
    item.status = spec.status;
    item.ledger_status = spec.ledger_status.map(str::to_string);
    item.operation_id = spec.operation_id.map(str::to_string);
    item
}

async fn seed_confirmed_inbox_projection(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    proposed_action_id: &str,
    inbox_id: &str,
    operation_id: &str,
    sync_cursor: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = PostgresReviewInboxRepository::new(pool.clone());
    let proposed_action =
        execution_projection_proposed_action(tenant_id, user_id, proposed_action_id);
    repository
        .insert_proposed_action(
            &proposed_action,
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(sync_cursor)),
        )
        .await?;
    repository
        .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
            id: inbox_id,
            tenant_id,
            user_id,
            proposed_action_id,
            status: ReviewInboxItemStatus::Confirmed,
            ledger_status: Some("confirmed"),
            operation_id: Some(operation_id),
            sync_cursor,
        }))
        .await?;
    Ok(())
}

#[test]
fn postgres_live_execution_recorder_commits_ledger_audit_and_outbox_atomically() {
    run_live_postgres_test("execution_recorder_commit", |pool| async move {
        seed_user(&pool, "tenant_recorder", "user_recorder").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder",
            "tenant_recorder",
            "user_recorder",
            "idem_recorder",
        );
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_recorder_1",
                "trace_recorder",
                1,
                1_748_250_001_000,
                "user_recorder",
                "tenant_recorder",
                "progress_recorder",
            ),
            summary("confirmed by reviewer"),
        );
        let outbox = outbox_envelope("tenant_recorder", "trace_recorder", 1_748_250_010_000);

        let report = recorder
            .record_confirmation(&action, 1_748_250_000_000, "op_recorder", &event, &outbox)
            .await?;

        assert_eq!(report.operation.operation_id, "op_recorder");
        assert!(!report.duplicate);
        let outbox_id = report.outbox_id.expect("outbox should be enqueued");
        assert!(outbox_id > 0);

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder", "idem_recorder")
            .await?
            .expect("operation should commit");
        assert_eq!(operation.operation_id, "op_recorder");

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder", "trace_recorder")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_recorder_1");

        let outbox_row = sqlx::query(
            r#"
            SELECT aggregate_id, status
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(outbox_id)
        .fetch_one(&pool)
        .await?;
        let aggregate_id: String = outbox_row.try_get("aggregate_id")?;
        let status: String = outbox_row.try_get("status")?;
        assert_eq!(aggregate_id, "trace_recorder");
        assert_eq!(status, "pending");

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_duplicate_confirmation_skips_side_effects() {
    run_live_postgres_test(
        "execution_recorder_duplicate_confirmation",
        |pool| async move {
            seed_user(&pool, "tenant_recorder_dup", "user_recorder_dup").await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let audit = PostgresAuditEventRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_dup",
                "tenant_recorder_dup",
                "user_recorder_dup",
                "idem_recorder_dup",
            );
            let first_event = AuditEvent::confirmed_action(
                audit_context(
                    "evt_recorder_dup_1",
                    "trace_recorder_dup",
                    1,
                    1_748_250_001_000,
                    "user_recorder_dup",
                    "tenant_recorder_dup",
                    "progress_recorder_dup",
                ),
                summary("first confirmation"),
            );
            let second_event = AuditEvent::confirmed_action(
                audit_context(
                    "evt_recorder_dup_2",
                    "trace_recorder_dup",
                    2,
                    1_748_250_002_000,
                    "user_recorder_dup",
                    "tenant_recorder_dup",
                    "progress_recorder_dup",
                ),
                summary("duplicate confirmation"),
            );

            let first = recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_dup",
                    &first_event,
                    &outbox_envelope(
                        "tenant_recorder_dup",
                        "trace_recorder_dup",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            let duplicate = recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_dup_retry",
                    &second_event,
                    &outbox_envelope(
                        "tenant_recorder_dup",
                        "trace_recorder_dup",
                        1_748_250_011_000,
                    ),
                )
                .await?;

            assert!(!first.duplicate);
            assert!(first.outbox_id.is_some());
            assert!(duplicate.duplicate);
            assert_eq!(duplicate.outbox_id, None);
            assert_eq!(duplicate.operation.operation_id, "op_recorder_dup");

            let events = audit
                .find_by_tenant_and_trace_id("tenant_recorder_dup", "trace_recorder_dup")
                .await?;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].event_id, "evt_recorder_dup_1");

            let outbox_count: i64 = sqlx::query_scalar(
                r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
            )
            .bind("tenant_recorder_dup")
            .fetch_one(&pool)
            .await?;
            assert_eq!(outbox_count, 1);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_rejects_cross_tenant_event_and_outbox() {
    run_live_postgres_test("execution_recorder_tenant_mismatch", |pool| async move {
        seed_user(&pool, "tenant_recorder_safe", "user_recorder_safe").await?;
        seed_user(&pool, "tenant_recorder_other", "user_recorder_other").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_safe",
            "tenant_recorder_safe",
            "user_recorder_safe",
            "idem_recorder_safe",
        );
        let wrong_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_recorder_wrong_tenant",
                "trace_recorder_wrong_tenant",
                1,
                1_748_250_001_000,
                "user_recorder_other",
                "tenant_recorder_other",
                "progress_recorder_wrong_tenant",
            ),
            summary("wrong tenant event"),
        );

        let result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_safe",
                &wrong_event,
                &outbox_envelope(
                    "tenant_recorder_safe",
                    "trace_recorder_wrong_tenant",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result
                .as_ref()
                .err()
                .map(|error| error.to_string().contains("tenant mismatch"))
                .unwrap_or(false),
            "tenant mismatch should be rejected before persistence"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_safe", "idem_recorder_safe")
            .await?;
        assert_eq!(operation, None);

        let wrong_outbox_result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_safe",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_correct_tenant",
                        "trace_recorder_wrong_outbox",
                        1,
                        1_748_250_001_000,
                        "user_recorder_safe",
                        "tenant_recorder_safe",
                        "progress_recorder_wrong_outbox",
                    ),
                    summary("correct tenant event"),
                ),
                &outbox_envelope(
                    "tenant_recorder_other",
                    "trace_recorder_wrong_outbox",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            wrong_outbox_result
                .as_ref()
                .err()
                .map(|error| error.to_string().contains("tenant mismatch"))
                .unwrap_or(false),
            "outbox tenant mismatch should be rejected before persistence"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_safe", "idem_recorder_safe")
            .await?;
        assert_eq!(operation, None);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_records_dry_run_and_success_terminal_idempotently() {
    run_live_postgres_test("execution_recorder_success", |pool| async move {
        seed_user(&pool, "tenant_recorder_success", "user_recorder_success").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_success",
            "tenant_recorder_success",
            "user_recorder_success",
            "idem_recorder_success",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_success",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_success_1",
                        "trace_recorder_success",
                        1,
                        1_748_250_001_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_010_000,
                ),
            )
            .await?;

        let dry_run = recorder
            .record_dry_run(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    audit_context(
                        "evt_recorder_success_2",
                        "trace_recorder_success",
                        2,
                        1_748_250_002_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_011_000,
                ),
            )
            .await?;
        assert_eq!(dry_run.operation.status, ActionStatus::Executing);
        assert!(!dry_run.duplicate);
        assert!(dry_run.outbox_id.is_some());

        let success = recorder
            .record_success(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_success_3",
                        "trace_recorder_success",
                        3,
                        1_748_250_003_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_success",
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_012_000,
                ),
            )
            .await?;
        assert_eq!(success.operation.status, ActionStatus::Succeeded);
        assert!(!success.duplicate);
        assert!(success.outbox_id.is_some());

        let duplicate_success = recorder
            .record_success(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_004_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_success_4",
                        "trace_recorder_success",
                        4,
                        1_748_250_004_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied again")),
                    "lark_op_success_retry",
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_013_000,
                ),
            )
            .await?;
        assert_eq!(duplicate_success.operation.status, ActionStatus::Succeeded);
        assert!(duplicate_success.duplicate);
        assert_eq!(duplicate_success.outbox_id, None);

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_success", "idem_recorder_success")
            .await?
            .expect("operation should exist");
        assert_eq!(operation.status, ActionStatus::Succeeded);

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder_success", "trace_recorder_success")
            .await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, "evt_recorder_success_1");
        assert_eq!(events[1].event_id, "evt_recorder_success_2");
        assert_eq!(events[2].event_id, "evt_recorder_success_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_success")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_projects_ledger_status_to_review_inbox() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_projection",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection",
                "user_recorder_projection",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection",
                "tenant_recorder_projection",
                "user_recorder_projection",
                "idem_recorder_projection",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_1",
                            "trace_recorder_projection",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection",
                "user_recorder_projection",
                "proposed_recorder_projection",
                "inbox_recorder_projection",
                "op_recorder_projection",
                100,
            )
            .await?;

            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_2",
                            "trace_recorder_projection",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection")
            );

            let after_dry_run = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_dry_run[0].status, ReviewInboxItemStatus::Executing);
            assert_eq!(
                after_dry_run[0].ledger_status,
                Some(ActionStatus::Executing)
            );
            assert_eq!(after_dry_run[0].sync_cursor_value, 101);

            let success = recorder
                .record_success(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_003_000,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_projection_3",
                            "trace_recorder_projection",
                            3,
                            1_748_250_003_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("applied")),
                        "lark_op_projection",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_012_000,
                    ),
                )
                .await?;
            assert_eq!(
                success.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection")
            );

            let after_success = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_success[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(
                after_success[0].ledger_status,
                Some(ActionStatus::Succeeded)
            );
            assert_eq!(after_success[0].sync_cursor_value, 102);

            let duplicate_success = recorder
                .record_success(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_004_000,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_projection_4",
                            "trace_recorder_projection",
                            4,
                            1_748_250_004_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("applied again")),
                        "lark_op_projection_retry",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_013_000,
                    ),
                )
                .await?;
            assert!(duplicate_success.duplicate);
            assert_eq!(duplicate_success.inbox_item_id, None);

            let after_duplicate = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_duplicate[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(after_duplicate[0].sync_cursor_value, 102);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_projection_keeps_source_cursor_and_allows_new_source_sync() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_source_cursor",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_source_cursor",
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
                "idem_recorder_source_cursor",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_source_cursor",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_source_cursor_1",
                            "trace_recorder_source_cursor",
                            1,
                            1_748_250_001_000,
                            "user_recorder_source_cursor",
                            "tenant_recorder_source_cursor",
                            "action_recorder_source_cursor",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_source_cursor",
                        "trace_recorder_source_cursor",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
                "proposed_recorder_source_cursor",
                "inbox_recorder_source_cursor",
                "op_recorder_source_cursor",
                100,
            )
            .await?;

            let projection_now_ms = 1_748_250_002_000;
            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_source_cursor",
                    "idem_recorder_source_cursor",
                    projection_now_ms,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_source_cursor_2",
                            "trace_recorder_source_cursor",
                            2,
                            projection_now_ms,
                            "user_recorder_source_cursor",
                            "tenant_recorder_source_cursor",
                            "action_recorder_source_cursor",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_source_cursor",
                        "trace_recorder_source_cursor",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_source_cursor")
            );

            let projected_row = sqlx::query(
                r#"
                SELECT source_cursor_value, sync_cursor_value
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder_source_cursor")
            .bind("inbox_recorder_source_cursor")
            .fetch_one(&pool)
            .await?;
            let source_cursor_value: i64 = projected_row.try_get("source_cursor_value")?;
            let projected_sync_cursor_value: i64 = projected_row.try_get("sync_cursor_value")?;
            assert_eq!(source_cursor_value, 100);
            assert_eq!(projected_sync_cursor_value, 101);

            let upserted = repository
                .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
                    id: "inbox_recorder_source_cursor",
                    tenant_id: "tenant_recorder_source_cursor",
                    user_id: "user_recorder_source_cursor",
                    proposed_action_id: "proposed_recorder_source_cursor",
                    status: ReviewInboxItemStatus::Confirmed,
                    ledger_status: Some("confirmed"),
                    operation_id: Some("op_recorder_source_cursor"),
                    sync_cursor: 101,
                }))
                .await?;
            assert_eq!(upserted.as_deref(), Some("inbox_recorder_source_cursor"));

            let after_source_sync = sqlx::query(
                r#"
                SELECT source_cursor_value, sync_cursor_value
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder_source_cursor")
            .bind("inbox_recorder_source_cursor")
            .fetch_one(&pool)
            .await?;
            let next_source_cursor_value: i64 = after_source_sync.try_get("source_cursor_value")?;
            let next_sync_cursor_value: i64 = after_source_sync.try_get("sync_cursor_value")?;
            assert_eq!(next_source_cursor_value, 101);
            assert_eq!(next_sync_cursor_value, 102);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_advances_inbox_projection_with_same_millisecond_events() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_same_ms",
        |pool| async move {
            seed_user(&pool, "tenant_recorder_same_ms", "user_recorder_same_ms").await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_same_ms",
                "tenant_recorder_same_ms",
                "user_recorder_same_ms",
                "idem_recorder_same_ms",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_same_ms",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_same_ms_1",
                            "trace_recorder_same_ms",
                            1,
                            1_748_250_001_000,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_same_ms",
                        "trace_recorder_same_ms",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_same_ms",
                "user_recorder_same_ms",
                "proposed_recorder_same_ms",
                "inbox_recorder_same_ms",
                "op_recorder_same_ms",
                1_748_250_002_000,
            )
            .await?;

            let same_ms = 1_748_250_003_000;
            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_same_ms",
                    "idem_recorder_same_ms",
                    same_ms,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_same_ms_2",
                            "trace_recorder_same_ms",
                            2,
                            same_ms,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope("tenant_recorder_same_ms", "trace_recorder_same_ms", same_ms),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_same_ms")
            );
            let after_dry_run = repository
                .list_review_inbox_items("tenant_recorder_same_ms", "user_recorder_same_ms", 0, 10)
                .await?;
            assert_eq!(after_dry_run[0].status, ReviewInboxItemStatus::Executing);
            let executing_cursor = after_dry_run[0].sync_cursor_value;

            let success = recorder
                .record_success(
                    "tenant_recorder_same_ms",
                    "idem_recorder_same_ms",
                    same_ms,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_same_ms_3",
                            "trace_recorder_same_ms",
                            3,
                            same_ms,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        Some(summary("before")),
                        Some(summary("applied")),
                        "lark_op_same_ms",
                    ),
                    &outbox_envelope("tenant_recorder_same_ms", "trace_recorder_same_ms", same_ms),
                )
                .await?;
            assert_eq!(
                success.inbox_item_id.as_deref(),
                Some("inbox_recorder_same_ms")
            );

            let after_success = repository
                .list_review_inbox_items("tenant_recorder_same_ms", "user_recorder_same_ms", 0, 10)
                .await?;
            assert_eq!(after_success[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(
                after_success[0].ledger_status,
                Some(ActionStatus::Succeeded)
            );
            assert!(
                after_success[0].sync_cursor_value > executing_cursor,
                "DB-owned sync cursor must advance even when ledger events share a millisecond"
            );

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_projects_failure_to_review_inbox() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_failure",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection_failure",
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
                "idem_recorder_projection_failure",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection_failure",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_failure_1",
                            "trace_recorder_projection_failure",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
                "proposed_recorder_projection_failure",
                "inbox_recorder_projection_failure",
                "op_recorder_projection_failure",
                100,
            )
            .await?;
            recorder
                .record_dry_run(
                    "tenant_recorder_projection_failure",
                    "idem_recorder_projection_failure",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_failure_2",
                            "trace_recorder_projection_failure",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_011_000,
                    ),
                )
                .await?;

            let failed = recorder
                .record_failure(
                    "tenant_recorder_projection_failure",
                    "idem_recorder_projection_failure",
                    "adapter timeout",
                    1_748_250_003_000,
                    &AuditEvent::execution_failed(
                        audit_context(
                            "evt_recorder_projection_failure_3",
                            "trace_recorder_projection_failure",
                            3,
                            1_748_250_003_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        Some(summary("before")),
                        None,
                        "adapter_timeout",
                        "adapter timeout",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_012_000,
                    ),
                )
                .await?;
            assert_eq!(
                failed.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection_failure")
            );

            let rows = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection_failure",
                    "user_recorder_projection_failure",
                    0,
                    10,
                )
                .await?;
            assert_eq!(rows[0].status, ReviewInboxItemStatus::Failed);
            assert_eq!(rows[0].ledger_status, Some(ActionStatus::Failed));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_does_not_overwrite_terminal_inbox_projection() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_terminal_guard",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection_terminal",
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
                "idem_recorder_projection_terminal",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection_terminal",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_terminal_1",
                            "trace_recorder_projection_terminal",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection_terminal",
                            "tenant_recorder_projection_terminal",
                            "action_recorder_projection_terminal",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_terminal",
                        "trace_recorder_projection_terminal",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
                "proposed_recorder_projection_terminal",
                "inbox_recorder_projection_terminal",
                "op_recorder_projection_terminal",
                100,
            )
            .await?;
            repository
                .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
                    id: "inbox_recorder_projection_terminal",
                    tenant_id: "tenant_recorder_projection_terminal",
                    user_id: "user_recorder_projection_terminal",
                    proposed_action_id: "proposed_recorder_projection_terminal",
                    status: ReviewInboxItemStatus::Rejected,
                    ledger_status: Some("confirmed"),
                    operation_id: Some("op_recorder_projection_terminal"),
                    sync_cursor: 200,
                }))
                .await?;

            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_projection_terminal",
                    "idem_recorder_projection_terminal",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_terminal_2",
                            "trace_recorder_projection_terminal",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection_terminal",
                            "tenant_recorder_projection_terminal",
                            "action_recorder_projection_terminal",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_terminal",
                        "trace_recorder_projection_terminal",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(dry_run.operation.status, ActionStatus::Executing);
            assert_eq!(dry_run.inbox_item_id, None);

            let rows = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection_terminal",
                    "user_recorder_projection_terminal",
                    0,
                    10,
                )
                .await?;
            assert_eq!(rows[0].status, ReviewInboxItemStatus::Rejected);
            assert_eq!(rows[0].ledger_status, Some(ActionStatus::Confirmed));
            assert_eq!(rows[0].sync_cursor_value, 200);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_records_failure_terminal_idempotently() {
    run_live_postgres_test("execution_recorder_failure", |pool| async move {
        seed_user(&pool, "tenant_recorder_failure", "user_recorder_failure").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_failure",
            "tenant_recorder_failure",
            "user_recorder_failure",
            "idem_recorder_failure",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_failure",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_failure_1",
                        "trace_recorder_failure",
                        1,
                        1_748_250_001_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_010_000,
                ),
            )
            .await?;
        recorder
            .record_dry_run(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    audit_context(
                        "evt_recorder_failure_2",
                        "trace_recorder_failure",
                        2,
                        1_748_250_002_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_011_000,
                ),
            )
            .await?;

        let failed = recorder
            .record_failure(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                "stderr leaked refresh_token=raw-secret",
                1_748_250_003_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_recorder_failure_3",
                        "trace_recorder_failure",
                        3,
                        1_748_250_003_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_timeout",
                    "adapter timeout",
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_012_000,
                ),
            )
            .await?;
        assert_eq!(failed.operation.status, ActionStatus::Failed);
        assert_eq!(
            failed.operation.last_error.as_deref(),
            Some("adapter execution failed")
        );
        assert!(failed.outbox_id.is_some());

        let duplicate_failed = recorder
            .record_failure(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                "different retry error",
                1_748_250_004_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_recorder_failure_4",
                        "trace_recorder_failure",
                        4,
                        1_748_250_004_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_retry_timeout",
                    "different retry error",
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_013_000,
                ),
            )
            .await?;
        assert!(duplicate_failed.duplicate);
        assert_eq!(duplicate_failed.outbox_id, None);
        assert_eq!(
            duplicate_failed.operation.last_error.as_deref(),
            Some("adapter execution failed")
        );

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder_failure", "trace_recorder_failure")
            .await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "evt_recorder_failure_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_failure")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_reports_explicit_invalid_transition() {
    run_live_postgres_test("execution_recorder_invalid_transition", |pool| async move {
        seed_user(
            &pool,
            "tenant_recorder_invalid_transition",
            "user_recorder_invalid_transition",
        )
        .await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_invalid_transition",
            "tenant_recorder_invalid_transition",
            "user_recorder_invalid_transition",
            "idem_recorder_invalid_transition",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_invalid_transition",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_invalid_transition_1",
                        "trace_recorder_invalid_transition",
                        1,
                        1_748_250_001_000,
                        "user_recorder_invalid_transition",
                        "tenant_recorder_invalid_transition",
                        "progress_recorder_invalid_transition",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_invalid_transition",
                    "trace_recorder_invalid_transition",
                    1_748_250_010_000,
                ),
            )
            .await?;

        let result = recorder
            .record_success(
                "tenant_recorder_invalid_transition",
                "idem_recorder_invalid_transition",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_invalid_transition_2",
                        "trace_recorder_invalid_transition",
                        2,
                        1_748_250_003_000,
                        "user_recorder_invalid_transition",
                        "tenant_recorder_invalid_transition",
                        "progress_recorder_invalid_transition",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_invalid_transition",
                ),
                &outbox_envelope(
                    "tenant_recorder_invalid_transition",
                    "trace_recorder_invalid_transition",
                    1_748_250_012_000,
                ),
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::InvalidOperationStatusTransition {
                from: ActionStatus::Confirmed,
                to: ActionStatus::Succeeded,
            })
        ));

        let events = audit
            .find_by_tenant_and_trace_id(
                "tenant_recorder_invalid_transition",
                "trace_recorder_invalid_transition",
            )
            .await?;
        assert_eq!(events.len(), 1);

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_invalid_transition")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("execution_recorder_rollback", |pool| async move {
        seed_user(&pool, "tenant_recorder_rollback", "user_recorder_rollback").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_rollback",
            "tenant_recorder_rollback",
            "user_recorder_rollback",
            "idem_recorder_rollback",
        );
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_duplicate",
                "trace_recorder_rollback",
                1,
                1_748_250_001_000,
                "user_recorder_rollback",
                "tenant_recorder_rollback",
                "progress_recorder_rollback",
            ),
            summary("confirmed by reviewer"),
        );

        audit.append(&event, None).await?;

        let result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_rollback",
                &event,
                &outbox_envelope(
                    "tenant_recorder_rollback",
                    "trace_recorder_rollback",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should fail the whole transaction"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_rollback", "idem_recorder_rollback")
            .await?;
        assert_eq!(
            operation, None,
            "ledger insert must roll back when audit append fails"
        );

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_rollback")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 0, "outbox enqueue must roll back too");

        Ok(())
    });
}
