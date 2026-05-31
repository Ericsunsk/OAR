use super::*;

#[test]
fn postgres_live_scheduler_insert_job_if_missing_inserts_missing_job() {
    run_live_postgres_test("scheduler_insert_missing", |pool| async move {
        seed_user(&pool, "tenant_scheduler_insert", "user_scheduler_insert").await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        let inserted = repository
            .insert_job_if_missing(
                "job_scheduler_insert",
                "tenant_scheduler_insert",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        assert_eq!(inserted.id, "job_scheduler_insert");
        assert_eq!(inserted.tenant_id, "tenant_scheduler_insert");
        assert_eq!(inserted.job_kind, SchedulerJobKind::TokenRefreshSweep);
        assert_eq!(inserted.status, SchedulerJobStatus::Pending);
        assert_eq!(inserted.next_run_at_ms, 1_000);
        assert_eq!(inserted.attempt_count, 0);
        assert_eq!(inserted.lease_id, None);

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_insert_job_if_missing_does_not_reset_pending_schedule() {
    run_live_postgres_test("scheduler_insert_pending_no_reset", |pool| async move {
        seed_user(
            &pool,
            "tenant_scheduler_insert_pending",
            "user_scheduler_insert_pending",
        )
        .await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        repository
            .upsert_job(
                "job_scheduler_insert_pending",
                "tenant_scheduler_insert_pending",
                SchedulerJobKind::TokenRefreshSweep,
                10_000,
            )
            .await?;

        let existing = repository
            .insert_job_if_missing(
                "job_scheduler_insert_pending_replacement",
                "tenant_scheduler_insert_pending",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        assert_eq!(existing.id, "job_scheduler_insert_pending");
        assert_eq!(existing.status, SchedulerJobStatus::Pending);
        assert_eq!(existing.next_run_at_ms, 10_000);
        assert_eq!(existing.attempt_count, 0);
        assert_eq!(existing.lease_id, None);

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_insert_job_if_missing_does_not_reset_running_lease() {
    run_live_postgres_test("scheduler_insert_running_no_reset", |pool| async move {
        seed_user(
            &pool,
            "tenant_scheduler_insert_running",
            "user_scheduler_insert_running",
        )
        .await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        repository
            .upsert_job(
                "job_scheduler_insert_running",
                "tenant_scheduler_insert_running",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        let lease = match repository
            .try_acquire(
                "tenant_scheduler_insert_running",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_scheduler_insert_running",
                8_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected scheduler lease, got {other:?}"),
        };

        let existing = repository
            .insert_job_if_missing(
                "job_scheduler_insert_running_replacement",
                "tenant_scheduler_insert_running",
                SchedulerJobKind::TokenRefreshSweep,
                50_000,
            )
            .await?;

        assert_eq!(existing.id, "job_scheduler_insert_running");
        assert_eq!(existing.status, SchedulerJobStatus::Running);
        assert_eq!(existing.next_run_at_ms, 1_000);
        assert_eq!(
            existing.lease_id.as_deref(),
            Some("lease_scheduler_insert_running")
        );
        assert_eq!(existing.lease_until_ms, Some(8_000));
        assert_eq!(existing.attempt_count, 1);

        assert!(
            repository.complete_for_lease(&lease, 8_100, 20_000).await?,
            "current scheduler lease should remain finalizable after insert_job_if_missing"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_insert_job_if_missing_is_concurrency_safe() {
    run_live_postgres_test("scheduler_insert_concurrent", |pool| async move {
        seed_user(
            &pool,
            "tenant_scheduler_insert_concurrent",
            "user_scheduler_insert_concurrent",
        )
        .await?;

        let first_repository = PostgresSchedulerJobRepository::new(pool.clone());
        let second_repository = PostgresSchedulerJobRepository::new(pool.clone());
        let first_task = tokio::spawn(async move {
            first_repository
                .insert_job_if_missing(
                    "job_scheduler_insert_concurrent",
                    "tenant_scheduler_insert_concurrent",
                    SchedulerJobKind::TokenRefreshSweep,
                    1_000,
                )
                .await
        });
        let second_task = tokio::spawn(async move {
            second_repository
                .insert_job_if_missing(
                    "job_scheduler_insert_concurrent_racer",
                    "tenant_scheduler_insert_concurrent",
                    SchedulerJobKind::TokenRefreshSweep,
                    2_000,
                )
                .await
        });

        let first = first_task.await??;
        let second = second_task.await??;
        assert_eq!(first.id, second.id);
        assert_eq!(first.next_run_at_ms, second.next_run_at_ms);

        let row_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM scheduler_jobs
            WHERE tenant_id = $1
              AND job_kind = $2
            "#,
        )
        .bind("tenant_scheduler_insert_concurrent")
        .bind("token_refresh_sweep")
        .fetch_one(&pool)
        .await?;
        assert_eq!(row_count, 1);

        Ok(())
    });
}
