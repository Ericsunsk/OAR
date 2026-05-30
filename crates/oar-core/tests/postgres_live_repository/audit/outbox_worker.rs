use super::*;

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
