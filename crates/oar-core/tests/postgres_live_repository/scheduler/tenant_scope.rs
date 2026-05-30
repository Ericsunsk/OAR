use super::*;

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
