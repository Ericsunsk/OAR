use super::*;

#[test]
fn postgres_live_scheduler_job_retry_backoff_and_tenant_scope_are_guarded() {
    run_live_postgres_test("scheduler_retry_tenant_scope", |pool| async move {
        seed_user(&pool, "tenant_scheduler_retry", "user_scheduler_retry").await?;
        seed_user(&pool, "tenant_scheduler_other", "user_scheduler_other").await?;

        let repository = PostgresSchedulerJobRepository::new(pool.clone());
        repository
            .upsert_job(
                "job_scheduler_retry",
                "tenant_scheduler_retry",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;
        repository
            .upsert_job(
                "job_scheduler_other",
                "tenant_scheduler_other",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        let lease = match repository
            .try_acquire(
                "tenant_scheduler_retry",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_retry",
                8_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected retry lease, got {other:?}"),
        };

        let mut wrong_tenant_lease = lease.clone();
        wrong_tenant_lease.tenant_id = "tenant_scheduler_other".to_string();
        assert!(
            !repository
                .fail_for_lease(&wrong_tenant_lease, 5_500, "transient_timeout", 12_000)
                .await?,
            "scheduler finalize must be tenant scoped"
        );

        assert!(
            repository
                .fail_for_lease(&lease, 5_600, "transient_timeout", 12_000)
                .await?
        );

        let too_early = repository
            .try_acquire(
                "tenant_scheduler_retry",
                SchedulerJobKind::TokenRefreshSweep,
                11_999,
                "lease_too_early",
                14_000,
            )
            .await?;
        assert_eq!(
            too_early,
            SchedulerLeaseAcquire::NotDue {
                next_due_ms: 12_000
            }
        );

        let second = match repository
            .try_acquire(
                "tenant_scheduler_retry",
                SchedulerJobKind::TokenRefreshSweep,
                12_000,
                "lease_retry_2",
                16_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected retry due lease, got {other:?}"),
        };
        assert_eq!(second.attempt_count, 2);

        let stored = repository
            .get_job(
                "tenant_scheduler_retry",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("job should exist");
        assert_eq!(stored.last_safe_error_code.as_deref(), None);

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_job_rejects_unsafe_retry_error_code() {
    run_live_postgres_test("scheduler_safe_error_code", |pool| async move {
        seed_user(&pool, "tenant_scheduler_error", "user_scheduler_error").await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        repository
            .upsert_job(
                "job_scheduler_error",
                "tenant_scheduler_error",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        let lease = match repository
            .try_acquire(
                "tenant_scheduler_error",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_error",
                8_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected error-code lease, got {other:?}"),
        };

        let unsafe_result = repository
            .fail_for_lease(&lease, 5_500, "refresh_token leaked", 12_000)
            .await;
        assert!(matches!(
            unsafe_result,
            Err(PostgresRepositoryError::UnsafeSchedulerJobErrorCode)
        ));

        assert!(
            repository
                .fail_for_lease(&lease, 5_600, "transient_timeout", 12_000,)
                .await?
        );

        Ok(())
    });
}
