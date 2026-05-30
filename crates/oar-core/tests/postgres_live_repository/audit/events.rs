use super::*;

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
