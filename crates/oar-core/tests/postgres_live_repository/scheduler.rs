use super::harness::*;

#[test]
fn postgres_live_scheduler_job_claims_single_due_lease() {
    run_live_postgres_test("scheduler_single_claim", |pool| async move {
        seed_user(&pool, "tenant_scheduler_claim", "user_scheduler_claim").await?;

        let repository = PostgresSchedulerJobRepository::new(pool.clone());
        let inserted = repository
            .upsert_job(
                "job_scheduler_claim",
                "tenant_scheduler_claim",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;
        assert_eq!(inserted.status, SchedulerJobStatus::Pending);

        let first = repository
            .try_acquire(
                "tenant_scheduler_claim",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_a",
                8_000,
            )
            .await?;
        let second = repository
            .try_acquire(
                "tenant_scheduler_claim",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_b",
                8_500,
            )
            .await?;

        match first {
            SchedulerLeaseAcquire::Acquired(lease) => {
                assert_eq!(lease.id, "job_scheduler_claim");
                assert_eq!(lease.tenant_id, "tenant_scheduler_claim");
                assert_eq!(lease.lease_id, "lease_a");
                assert_eq!(lease.lease_until_ms, 8_000);
                assert_eq!(lease.attempt_count, 1);
            }
            other => panic!("expected first claim to acquire, got {other:?}"),
        }

        assert_eq!(
            second,
            SchedulerLeaseAcquire::Busy {
                retry_after_ms: 3_000
            }
        );

        let stored = repository
            .get_job(
                "tenant_scheduler_claim",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("job should exist");
        assert_eq!(stored.status, SchedulerJobStatus::Running);
        assert_eq!(stored.lease_id.as_deref(), Some("lease_a"));
        assert_eq!(stored.attempt_count, 1);

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_job_ids_are_tenant_scoped_and_upsert_reads_running_job() {
    run_live_postgres_test("scheduler_tenant_scoped_id", |pool| async move {
        seed_user(&pool, "tenant_scheduler_id_a", "user_scheduler_id_a").await?;
        seed_user(&pool, "tenant_scheduler_id_b", "user_scheduler_id_b").await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        let first = repository
            .upsert_job(
                "shared_scheduler_job_id",
                "tenant_scheduler_id_a",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;
        let second = repository
            .upsert_job(
                "shared_scheduler_job_id",
                "tenant_scheduler_id_b",
                SchedulerJobKind::TokenRefreshSweep,
                2_000,
            )
            .await?;

        assert_eq!(first.tenant_id, "tenant_scheduler_id_a");
        assert_eq!(second.tenant_id, "tenant_scheduler_id_b");
        assert_eq!(first.id, second.id);

        let lease = match repository
            .try_acquire(
                "tenant_scheduler_id_a",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_scheduler_id_a",
                8_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected tenant A lease, got {other:?}"),
        };

        let running = repository
            .upsert_job(
                "replacement_scheduler_job_id",
                "tenant_scheduler_id_a",
                SchedulerJobKind::TokenRefreshSweep,
                99_000,
            )
            .await?;
        assert_eq!(running.id, "shared_scheduler_job_id");
        assert_eq!(running.status, SchedulerJobStatus::Running);
        assert_eq!(running.next_run_at_ms, 1_000);
        assert_eq!(running.lease_id.as_deref(), Some("lease_scheduler_id_a"));

        let tenant_b = repository
            .get_job("tenant_scheduler_id_b", SchedulerJobKind::TokenRefreshSweep)
            .await?
            .expect("tenant B job should still exist");
        assert_eq!(tenant_b.status, SchedulerJobStatus::Pending);
        assert_eq!(tenant_b.next_run_at_ms, 2_000);
        assert_eq!(tenant_b.lease_id, None);

        assert!(
            repository.complete_for_lease(&lease, 8_100, 20_000).await?,
            "tenant A current lease should still finalize"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_scheduler_job_reclaims_stale_lease_and_rejects_old_finalize() {
    run_live_postgres_test("scheduler_stale_reclaim", |pool| async move {
        seed_user(&pool, "tenant_scheduler_reclaim", "user_scheduler_reclaim").await?;

        let repository = PostgresSchedulerJobRepository::new(pool);
        repository
            .upsert_job(
                "job_scheduler_reclaim",
                "tenant_scheduler_reclaim",
                SchedulerJobKind::TokenRefreshSweep,
                1_000,
            )
            .await?;

        let first = match repository
            .try_acquire(
                "tenant_scheduler_reclaim",
                SchedulerJobKind::TokenRefreshSweep,
                5_000,
                "lease_old",
                8_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected old lease, got {other:?}"),
        };

        let second = match repository
            .try_acquire(
                "tenant_scheduler_reclaim",
                SchedulerJobKind::TokenRefreshSweep,
                8_001,
                "lease_new",
                12_000,
            )
            .await?
        {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            other => panic!("expected stale lease reclaim, got {other:?}"),
        };

        assert_eq!(second.attempt_count, 2);
        assert!(
            !repository.complete_for_lease(&first, 8_100, 20_000).await?,
            "stale scheduler worker must not finalize a reclaimed job"
        );
        assert!(
            repository
                .complete_for_lease(&second, 12_100, 20_000)
                .await?,
            "current scheduler worker should finalize"
        );

        let stored = repository
            .get_job(
                "tenant_scheduler_reclaim",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("job should exist");
        assert_eq!(stored.status, SchedulerJobStatus::Pending);
        assert_eq!(stored.lease_id, None);
        assert_eq!(stored.lease_until_ms, None);
        assert_eq!(stored.next_run_at_ms, 20_000);
        assert_eq!(stored.attempt_count, 2);

        Ok(())
    });
}

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
