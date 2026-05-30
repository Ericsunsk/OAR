use super::*;

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
