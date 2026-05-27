use super::harness::*;

#[derive(Default)]
struct AlwaysErrOutboxDispatcher;

impl AuditOutboxDispatcher for AlwaysErrOutboxDispatcher {
    type Error = &'static str;

    async fn deliver(
        &mut self,
        _message: &AuditOutboxMessage,
    ) -> Result<AuditOutboxDelivery, Self::Error> {
        Err("dispatch_failed")
    }
}

#[test]
fn postgres_live_audit_repository_orders_events_and_enforces_append_only() {
    run_live_postgres_test("audit_repository", |pool| async move {
        seed_user(&pool, "tenant_audit", "user_audit").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let second = AuditEvent::dry_run(
            audit_context(
                "evt_2",
                "trace_audit",
                2,
                1_748_250_002_000,
                "user_audit",
                "tenant_audit",
                "progress_audit",
            ),
            Some(summary("before")),
            Some(summary("projected")),
        );
        let first = AuditEvent::confirmed_action(
            audit_context(
                "evt_1",
                "trace_audit",
                1,
                1_748_250_001_000,
                "user_audit",
                "tenant_audit",
                "progress_audit",
            ),
            summary("confirmed"),
        );

        repository.append(&second, None).await?;
        repository.append(&first, None).await?;

        let events = repository
            .find_by_tenant_and_trace_id("tenant_audit", "trace_audit")
            .await?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, "evt_1");
        assert_eq!(events[1].event_id, "evt_2");
        assert_eq!(
            events[1]
                .execution
                .as_ref()
                .and_then(|execution| execution.message.as_deref()),
            None
        );

        let duplicate = repository.append(&events[0], None).await;
        assert!(
            duplicate.is_err(),
            "duplicate audit event IDs should be rejected"
        );

        let update_result = sqlx::query(
            r#"
            UPDATE audit_events
            SET actor_display_name = 'Mutated'
            WHERE event_id = $1
            "#,
        )
        .bind("evt_1")
        .execute(&pool)
        .await;
        assert!(
            update_result.is_err(),
            "audit_events update trigger should enforce append-only storage"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_trace_lookup_is_tenant_scoped() {
    run_live_postgres_test("audit_repository_tenant_scoped_trace", |pool| async move {
        seed_user(&pool, "tenant_audit_trace_a", "user_audit_trace_a").await?;
        seed_user(&pool, "tenant_audit_trace_b", "user_audit_trace_b").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let event_a = AuditEvent::confirmed_action(
            audit_context(
                "evt_audit_trace_a",
                "trace_shared_audit",
                1,
                1_748_250_001_000,
                "user_audit_trace_a",
                "tenant_audit_trace_a",
                "progress_shared_audit",
            ),
            summary("tenant a confirmed"),
        );
        let event_b = AuditEvent::confirmed_action(
            audit_context(
                "evt_audit_trace_b",
                "trace_shared_audit",
                1,
                1_748_250_001_100,
                "user_audit_trace_b",
                "tenant_audit_trace_b",
                "progress_shared_audit",
            ),
            summary("tenant b confirmed"),
        );

        repository.append(&event_a, None).await?;
        repository.append(&event_b, None).await?;

        let tenant_a = repository
            .find_by_tenant_and_trace_id("tenant_audit_trace_a", "trace_shared_audit")
            .await?;
        let tenant_b = repository
            .find_by_tenant_and_trace_id("tenant_audit_trace_b", "trace_shared_audit")
            .await?;
        let missing = repository
            .find_by_tenant_and_trace_id("tenant_audit_trace_missing", "trace_shared_audit")
            .await?;

        assert_eq!(tenant_a, vec![event_a]);
        assert_eq!(tenant_b, vec![event_b]);
        assert!(missing.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_audit_roundtrip() {
    run_live_postgres_test("token_refresh_audit_roundtrip", |pool| async move {
        seed_user(&pool, "tenant_refresh_audit", "user_refresh_audit").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let event = token_refresh_audit_event(
            TokenRefreshAuditContext {
                trace_id: "trace_token_refresh_audit".to_string(),
                sequence: 7,
                occurred_at_ms: 1_748_250_007_000,
                actor: actor("user_refresh_audit"),
                workspace_id: None,
            },
            &TokenRefreshAuditSummary {
                grant_id: TokenGrantId("grant_refresh_audit".to_string()),
                tenant_id: TenantId("tenant_refresh_audit".to_string()),
                status: TokenRefreshReportStatus::Succeeded,
                decision: None,
                command: Some(TokenRefreshCommandKind::RotateGrantCas),
                safe_error: None,
            },
        );

        repository.append(&event, None).await?;

        let events = repository
            .find_by_tenant_and_trace_id("tenant_refresh_audit", "trace_token_refresh_audit")
            .await?;
        assert_eq!(events.len(), 1);

        let persisted = &events[0];
        assert_eq!(persisted.event_type, AuditEventType::ExecutionSucceeded);
        assert_eq!(persisted.scope.tenant_id, "tenant_refresh_audit");
        assert_eq!(persisted.target.resource_type, "token_grant");
        assert_eq!(persisted.target.action_type, "token_refresh.rotate");

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
              jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string().to_lowercase();
        assert!(!payload_text.contains("access_token"));
        assert!(!payload_text.contains("refresh_token"));
        assert!(!payload_text.contains("authorization"));
        assert!(!payload_text.contains("fingerprint"));
        assert!(!payload_text.contains("encrypted"));
        assert!(!payload_text.contains("9, 9, 9"));

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_enqueue_sets_retry_defaults() {
    run_live_postgres_test("audit_outbox", |pool| async move {
        seed_user(&pool, "tenant_outbox", "user_outbox").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let payload = json!({
            "event_id": "evt_outbox",
            "trace_id": "trace_outbox",
        });
        let id = repository
            .enqueue_outbox(
                "tenant_outbox",
                "audit-events",
                "trace_outbox",
                &payload,
                1_748_250_010_000,
            )
            .await?;

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count, payload
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;

        let status: String = row.try_get("status")?;
        let attempt_count: i32 = row.try_get("attempt_count")?;
        let stored_payload: serde_json::Value = row.try_get("payload")?;

        assert_eq!(status, "pending");
        assert_eq!(attempt_count, 0);
        assert_eq!(stored_payload, payload);

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_enqueue_rejects_unsafe_payload_without_insert() {
    run_live_postgres_test("audit_outbox_unsafe_payload", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_unsafe_payload",
            "user_outbox_unsafe_payload",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let result = repository
            .enqueue_outbox(
                "tenant_outbox_unsafe_payload",
                "audit-events",
                "trace_outbox_unsafe_payload",
                &json!({
                    "trace_id": "trace_outbox_unsafe_payload",
                    "authorization": "Bearer secret_value"
                }),
                1_748_250_011_000,
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_outbox_unsafe_payload")
        .fetch_one(&pool)
        .await?;
        assert_eq!(count, 0);

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_claims_and_marks_delivery_states() {
    run_live_postgres_test("audit_outbox_claim", |pool| async move {
        seed_user(&pool, "tenant_outbox_claim", "user_outbox_claim").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let first_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_1",
                &json!({ "trace_id": "trace_1" }),
                1_000,
            )
            .await?;
        let second_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_2",
                &json!({ "trace_id": "trace_2" }),
                2_000,
            )
            .await?;
        let future_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_future",
                &json!({ "trace_id": "trace_future" }),
                10_000,
            )
            .await?;

        let first_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, first_id);
        assert_eq!(first_claim[0].attempt_count, 1);
        assert_eq!(first_claim[0].next_attempt_at_ms, Some(8_000));

        let second_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 5_000, 10, 9_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, second_id);

        assert!(
            repository
                .mark_outbox_sent("tenant_outbox_claim", first_id, 6_000)
                .await?
        );
        assert!(
            !repository
                .mark_outbox_sent("other_tenant", first_id, 6_000)
                .await?,
            "outbox delivery updates must be tenant scoped"
        );

        assert!(
            repository
                .mark_outbox_retryable("tenant_outbox_claim", second_id, 4_000)
                .await?
        );
        assert!(
            repository
                .mark_outbox_failed("tenant_outbox_claim", future_id)
                .await?
        );

        let final_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 10_000, 10, 12_000)
            .await?;

        assert_eq!(final_claim.len(), 1);
        assert_eq!(final_claim[0].id, second_id);
        assert_eq!(final_claim[0].attempt_count, 2);
        assert_eq!(final_claim[0].payload, json!({ "trace_id": "trace_2" }));

        let rows = sqlx::query(
            r#"
            SELECT id, status
            FROM audit_outbox
            ORDER BY id ASC
            "#,
        )
        .fetch_all(&pool)
        .await?;
        let statuses: Vec<(i64, String)> = rows
            .iter()
            .map(|row| Ok((row.try_get("id")?, row.try_get("status")?)))
            .collect::<Result<_, sqlx::Error>>()?;

        assert_eq!(
            statuses,
            vec![
                (first_id, "sent".to_string()),
                (second_id, "pending".to_string()),
                (future_id, "failed".to_string())
            ]
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_guarded_mark_rejects_stale_claim_after_reclaim() {
    run_live_postgres_test("audit_outbox_guarded_stale", |pool| async move {
        seed_user(&pool, "tenant_outbox_guard", "user_outbox_guard").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_guard",
                "audit-events",
                "trace_guarded",
                &json!({ "trace_id": "trace_guarded" }),
                1_000,
            )
            .await?;

        let first_claim = repository
            .claim_outbox("tenant_outbox_guard", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, message_id);
        assert_eq!(first_claim[0].attempt_count, 1);

        let second_claim = repository
            .claim_outbox("tenant_outbox_guard", "audit-events", 9_000, 1, 12_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, message_id);
        assert_eq!(second_claim[0].attempt_count, 2);

        assert!(
            !repository
                .mark_outbox_sent_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    first_claim[0].attempt_count,
                    8_000,
                    9_500,
                )
                .await?,
            "stale worker must not be able to mark a re-claimed message sent"
        );

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(row.try_get::<String, _>("status")?, "pending");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 2);

        assert!(
            repository
                .mark_outbox_sent_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    second_claim[0].attempt_count,
                    12_000,
                    12_500,
                )
                .await?,
            "current claimant should be able to finalize delivery"
        );

        assert!(
            !repository
                .mark_outbox_retryable_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    second_claim[0].attempt_count,
                    12_000,
                    13_000,
                )
                .await?,
            "terminal sent messages should not be reopened as retryable"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_worker_drains_mixed_delivery_outcomes() {
    run_live_postgres_test("audit_outbox_worker_mixed", |pool| async move {
        seed_user(&pool, "tenant_outbox_worker", "user_outbox_worker").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        repository
            .enqueue_outbox(
                "tenant_outbox_worker",
                "audit-events",
                "trace_sent",
                &json!({ "trace_id": "trace_sent" }),
                1_000,
            )
            .await?;
        repository
            .enqueue_outbox(
                "tenant_outbox_worker",
                "audit-events",
                "trace_retry",
                &json!({ "trace_id": "trace_retry" }),
                1_000,
            )
            .await?;
        repository
            .enqueue_outbox(
                "tenant_outbox_worker",
                "audit-events",
                "trace_failed",
                &json!({ "trace_id": "trace_failed" }),
                1_000,
            )
            .await?;

        let dispatcher = LiveOutboxDispatcher::new([
            AuditOutboxDelivery::Sent,
            AuditOutboxDelivery::Retryable,
            AuditOutboxDelivery::Failed,
        ]);
        let mut ticks = vec![5_000_u64, 5_100, 5_200];
        ticks.reverse();
        let mut worker = PostgresAuditOutboxWorker::new(
            repository,
            dispatcher,
            move || ticks.pop().unwrap_or(5_999),
            AuditOutboxDrainConfig::new(
                "tenant_outbox_worker",
                "audit-events",
                10,
                3_000,
                7_000,
                3,
            ),
        );

        let report = worker.drain_once().await?;

        assert_eq!(report.claimed, 3);
        assert_eq!(report.sent, 1);
        assert_eq!(report.retryable, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.exhausted, 0);
        assert_eq!(report.stale, 0);

        let rows = sqlx::query(
            r#"
            SELECT aggregate_id, status, attempt_count,
                   floor(extract(epoch from next_attempt_at) * 1000)::bigint AS next_attempt_at_ms
            FROM audit_outbox
            WHERE tenant_id = $1
            ORDER BY id ASC
            "#,
        )
        .bind("tenant_outbox_worker")
        .fetch_all(&pool)
        .await?;
        let states: Vec<(String, String, i32, Option<i64>)> = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get("aggregate_id")?,
                    row.try_get("status")?,
                    row.try_get("attempt_count")?,
                    row.try_get("next_attempt_at_ms")?,
                ))
            })
            .collect::<Result<_, sqlx::Error>>()?;

        assert_eq!(
            states,
            vec![
                ("trace_sent".to_string(), "sent".to_string(), 1, Some(8_000)),
                (
                    "trace_retry".to_string(),
                    "pending".to_string(),
                    1,
                    Some(12_200)
                ),
                ("trace_failed".to_string(), "failed".to_string(), 1, None),
            ]
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_worker_err_below_retry_cap_stays_retryable() {
    run_live_postgres_test("audit_outbox_worker_err_retryable", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_worker_retry_cap",
            "user_outbox_worker_retry_cap",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_worker_retry_cap",
                "audit-events",
                "trace_retry_cap",
                &json!({ "trace_id": "trace_retry_cap" }),
                1_000,
            )
            .await?;

        let dispatcher = AlwaysErrOutboxDispatcher;
        let mut ticks = vec![5_000_u64, 5_100];
        ticks.reverse();
        let mut worker = PostgresAuditOutboxWorker::new(
            repository,
            dispatcher,
            move || ticks.pop().unwrap_or(5_100),
            AuditOutboxDrainConfig::new(
                "tenant_outbox_worker_retry_cap",
                "audit-events",
                10,
                3_000,
                7_000,
                3,
            ),
        );

        let report = worker.drain_once().await?;

        assert_eq!(report.claimed, 1);
        assert_eq!(report.sent, 0);
        assert_eq!(report.retryable, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.exhausted, 0);
        assert_eq!(report.stale, 0);

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count,
                   floor(extract(epoch from next_attempt_at) * 1000)::bigint AS next_attempt_at_ms
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;

        assert_eq!(row.try_get::<String, _>("status")?, "pending");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 1);
        assert_eq!(
            row.try_get::<Option<i64>, _>("next_attempt_at_ms")?,
            Some(12_100)
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_worker_err_at_retry_cap_marks_failed() {
    run_live_postgres_test("audit_outbox_worker_err_exhausted", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_worker_exhausted",
            "user_outbox_worker_exhausted",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_worker_exhausted",
                "audit-events",
                "trace_exhausted",
                &json!({ "trace_id": "trace_exhausted" }),
                1_000,
            )
            .await?;

        let dispatcher = AlwaysErrOutboxDispatcher;
        let mut ticks = vec![5_000_u64, 5_100, 12_100];
        ticks.reverse();
        let mut worker = PostgresAuditOutboxWorker::new(
            repository,
            dispatcher,
            move || ticks.pop().unwrap_or(12_100),
            AuditOutboxDrainConfig::new(
                "tenant_outbox_worker_exhausted",
                "audit-events",
                10,
                3_000,
                7_000,
                2,
            ),
        );

        let first_report = worker.drain_once().await?;
        assert_eq!(first_report.claimed, 1);
        assert_eq!(first_report.retryable, 1);
        assert_eq!(first_report.failed, 0);
        assert_eq!(first_report.exhausted, 0);

        let second_report = worker.drain_once().await?;
        assert_eq!(second_report.claimed, 1);
        assert_eq!(second_report.retryable, 0);
        assert_eq!(second_report.failed, 1);
        assert_eq!(second_report.exhausted, 1);
        assert_eq!(second_report.stale, 0);

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count,
                   floor(extract(epoch from next_attempt_at) * 1000)::bigint AS next_attempt_at_ms
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;

        assert_eq!(row.try_get::<String, _>("status")?, "failed");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 2);
        assert_eq!(row.try_get::<Option<i64>, _>("next_attempt_at_ms")?, None);

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_guarded_finalize_only_succeeds_once_for_same_claim() {
    run_live_postgres_test("audit_outbox_guarded_single_finalize", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_single_finalize",
            "user_outbox_single_finalize",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool);
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_single_finalize",
                "audit-events",
                "trace_single_finalize",
                &json!({ "trace_id": "trace_single_finalize" }),
                1_000,
            )
            .await?;
        let claim = repository
            .claim_outbox(
                "tenant_outbox_single_finalize",
                "audit-events",
                5_000,
                1,
                8_000,
            )
            .await?;
        assert_eq!(claim.len(), 1);
        assert_eq!(claim[0].attempt_count, 1);

        let first_mark = repository
            .mark_outbox_sent_for_attempt(
                "tenant_outbox_single_finalize",
                message_id,
                claim[0].attempt_count,
                8_000,
                8_100,
            )
            .await?;
        let duplicate_mark = repository
            .mark_outbox_sent_for_attempt(
                "tenant_outbox_single_finalize",
                message_id,
                claim[0].attempt_count,
                8_000,
                8_200,
            )
            .await?;

        assert!(first_mark);
        assert!(
            !duplicate_mark,
            "guarded finalize should be compare-and-set, not idempotent reopen"
        );
        assert!(
            !repository
                .mark_outbox_failed_for_attempt(
                    "tenant_outbox_single_finalize",
                    message_id,
                    claim[0].attempt_count,
                    8_000,
                )
                .await?,
            "terminal sent row should reject later failed mark for the same claim"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_retryable_then_reclaim_increments_attempt() {
    run_live_postgres_test("audit_outbox_retry_reclaim", |pool| async move {
        seed_user(&pool, "tenant_outbox_reclaim", "user_outbox_reclaim").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_reclaim",
                "audit-events",
                "trace_reclaim",
                &json!({ "trace_id": "trace_reclaim" }),
                1_000,
            )
            .await?;
        let first_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, message_id);
        assert_eq!(first_claim[0].attempt_count, 1);

        assert!(
            repository
                .mark_outbox_retryable_for_attempt(
                    "tenant_outbox_reclaim",
                    message_id,
                    first_claim[0].attempt_count,
                    8_000,
                    12_000,
                )
                .await?
        );

        let too_early_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 11_999, 1, 15_000)
            .await?;
        assert!(too_early_claim.is_empty());

        let second_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 12_000, 1, 16_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, message_id);
        assert_eq!(second_claim[0].attempt_count, 2);
        assert_eq!(second_claim[0].next_attempt_at_ms, Some(16_000));

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(row.try_get::<String, _>("status")?, "pending");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 2);

        Ok(())
    });
}
