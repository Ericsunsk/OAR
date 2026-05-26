#![cfg(feature = "postgres")]

use std::env;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditScope, AuditStateSummary, AuditTarget,
};
use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::operation_ledger::{LedgerError, SubmitResult};
use oar_core::storage::postgres::{
    AuditOutboxEnvelope, PostgresAuditEventRepository, PostgresExecutionUnitOfWork,
    PostgresOperationLedgerRepository,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool, Row};

const MIGRATION_SQL: &str = include_str!("../migrations/0001_phase_0_6_identity_action_audit.sql");

static SCHEMA_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
}

fn confirmed_action(
    action_id: &str,
    tenant_id: &str,
    actor_user_id: &str,
    idempotency_key: &str,
) -> ConfirmedAction {
    ConfirmedAction::proposed(action_id, tenant_id, actor_user_id, idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

fn actor(actor_id: &str) -> AuditActor {
    AuditActor {
        kind: AuditActorKind::User,
        actor_id: actor_id.to_string(),
        display_name: Some("Reviewer".to_string()),
    }
}

fn scope(tenant_id: &str) -> AuditScope {
    AuditScope {
        tenant_id: tenant_id.to_string(),
        workspace_id: None,
    }
}

fn target(resource_id: &str) -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: resource_id.to_string(),
        action_type: "update_progress".to_string(),
    }
}

fn summary(text: &str) -> AuditStateSummary {
    AuditStateSummary {
        summary: text.to_string(),
        reference_ids: vec!["evidence_1".to_string()],
        content_hash: Some("sha256:abc123".to_string()),
    }
}

fn outbox_envelope(
    tenant_id: &str,
    trace_id: &str,
    next_attempt_at_ms: u64,
) -> AuditOutboxEnvelope {
    AuditOutboxEnvelope {
        tenant_id: tenant_id.to_string(),
        stream: "audit-events".to_string(),
        aggregate_id: trace_id.to_string(),
        payload: json!({ "trace_id": trace_id }),
        next_attempt_at_ms,
    }
}

fn unique_schema_name(test_name: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let sequence = SCHEMA_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let sanitized_name: String = test_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();

    format!(
        "oar_live_{}_{}_{}_{}",
        std::process::id(),
        now,
        sequence,
        sanitized_name
    )
    .to_ascii_lowercase()
}

async fn create_schema_and_pool(database_url: &str, schema: &str) -> Result<PgPool, sqlx::Error> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;

    sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "SET search_path TO {schema};\n{MIGRATION_SQL}"
    )))
    .execute(&admin_pool)
    .await?;
    admin_pool.close().await;

    let schema_for_connection = schema.to_string();
    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _metadata| {
            let schema = schema_for_connection.clone();
            Box::pin(async move {
                sqlx::raw_sql(AssertSqlSafe(format!("SET search_path TO {schema}")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}

async fn drop_schema(database_url: &str, schema: &str) -> Result<(), sqlx::Error> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "DROP SCHEMA IF EXISTS {schema} CASCADE"
    )))
    .execute(&admin_pool)
    .await?;
    admin_pool.close().await;
    Ok(())
}

fn run_live_postgres_test<F, Fut>(test_name: &str, test: F)
where
    F: FnOnce(PgPool) -> Fut,
    Fut: Future<Output = TestResult>,
{
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        eprintln!("skip {test_name}: DATABASE_URL is not set");
        return;
    };

    runtime().block_on(async move {
        let schema = unique_schema_name(test_name);
        let pool = create_schema_and_pool(&database_url, &schema)
            .await
            .unwrap_or_else(|error| {
                panic!("failed to create live postgres schema {schema}: {error}")
            });

        let test_result = test(pool.clone()).await;
        pool.close().await;
        let cleanup_result = drop_schema(&database_url, &schema).await;

        if let Err(error) = cleanup_result {
            panic!("failed to drop live postgres schema {schema}: {error}");
        }
        test_result
            .unwrap_or_else(|error| panic!("live postgres test {test_name} failed: {error}"));
    });
}

async fn seed_user(pool: &PgPool, tenant_id: &str, user_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenants (id, display_name, status)
        VALUES ($1, $2, 'active')
        "#,
    )
    .bind(tenant_id)
    .bind(format!("Tenant {tenant_id}"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO oar_users (id, tenant_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(format!("User {user_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

#[test]
fn postgres_repository_rejects_unconfirmed_action_before_db_access() {
    runtime().block_on(async {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/oar_unreachable")
            .expect("lazy pool should parse static database url");
        let repository = PostgresOperationLedgerRepository::new(pool);
        let proposed = ConfirmedAction::proposed("action", "tenant", "user", "idem");

        let error = repository
            .submit_confirmed_action(&proposed, 0, "op")
            .await
            .expect_err("proposed actions should be rejected before database access");

        assert!(error
            .to_string()
            .contains("action must be confirmed before persistence"));
    });
}

#[test]
fn postgres_live_operation_repository_preserves_idempotent_transitions() {
    run_live_postgres_test("operation_repository", |pool| async move {
        seed_user(&pool, "tenant_live", "user_live").await?;

        let repository = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action("action_live_1", "tenant_live", "user_live", "idem_live");

        let first = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_1")
            .await?;
        let second = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_2")
            .await?;

        let created = match first {
            SubmitResult::Created(record) => record,
            SubmitResult::Existing(_) => panic!("first submit should create an operation"),
        };
        let duplicate = match second {
            SubmitResult::Existing(record) => record,
            SubmitResult::Created(_) => panic!("duplicate submit should return existing operation"),
        };
        let same_operation_id_retry = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_1")
            .await?;
        let same_operation_id_duplicate = match same_operation_id_retry {
            SubmitResult::Existing(record) => record,
            SubmitResult::Created(_) => {
                panic!("duplicate submit should not be inferred from matching operation_id")
            }
        };

        assert_eq!(created.operation_id, "op_live_1");
        assert_eq!(duplicate.operation_id, created.operation_id);
        assert_eq!(
            same_operation_id_duplicate.operation_id,
            created.operation_id
        );
        assert_eq!(duplicate.status, ActionStatus::Confirmed);

        let executing = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_001_000)
            .await
            .map_err(|error| format!("mark_executing failed: {error:?}"))?;
        let duplicate_executing = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_002_000)
            .await
            .map_err(|error| format!("duplicate mark_executing failed: {error:?}"))?;
        assert_eq!(executing.operation_id, duplicate_executing.operation_id);
        assert_eq!(duplicate_executing.status, ActionStatus::Executing);

        let succeeded = repository
            .mark_succeeded("tenant_live", "idem_live", 1_748_250_003_000)
            .await
            .map_err(|error| format!("mark_succeeded failed: {error:?}"))?;
        let duplicate_succeeded = repository
            .mark_succeeded("tenant_live", "idem_live", 1_748_250_004_000)
            .await
            .map_err(|error| format!("duplicate mark_succeeded failed: {error:?}"))?;
        assert_eq!(succeeded.operation_id, duplicate_succeeded.operation_id);
        assert_eq!(duplicate_succeeded.status, ActionStatus::Succeeded);

        let invalid_retry = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_005_000)
            .await;
        assert_eq!(
            invalid_retry,
            Err(LedgerError::InvalidTransition {
                from: ActionStatus::Succeeded,
                to: ActionStatus::Executing,
            })
        );

        let missing = repository
            .mark_executing("tenant_live", "missing_idem", 1_748_250_006_000)
            .await;
        assert_eq!(
            missing,
            Err(LedgerError::UnknownIdempotencyKey(
                "missing_idem".to_string()
            ))
        );

        Ok(())
    });
}

#[test]
fn postgres_live_operation_lookup_is_tenant_scoped() {
    run_live_postgres_test("operation_tenant_scope", |pool| async move {
        seed_user(&pool, "tenant_a", "user_a").await?;
        seed_user(&pool, "tenant_b", "user_b").await?;

        let repository = PostgresOperationLedgerRepository::new(pool);
        let action_a = confirmed_action("action_a", "tenant_a", "user_a", "shared_idem");
        let action_b = confirmed_action("action_b", "tenant_b", "user_b", "shared_idem");

        repository
            .submit_confirmed_action(&action_a, 1_748_250_000_000, "op_a")
            .await?;
        repository
            .submit_confirmed_action(&action_b, 1_748_250_000_000, "op_b")
            .await?;

        let record_a = repository
            .get_by_idempotency_key("tenant_a", "shared_idem")
            .await?
            .expect("tenant A record should exist");
        let record_b = repository
            .get_by_idempotency_key("tenant_b", "shared_idem")
            .await?
            .expect("tenant B record should exist");

        assert_eq!(record_a.operation_id, "op_a");
        assert_eq!(record_a.action_id, "action_a");
        assert_eq!(record_b.operation_id, "op_b");
        assert_eq!(record_b.action_id, "action_b");

        Ok(())
    });
}

#[test]
fn postgres_live_audit_repository_orders_events_and_enforces_append_only() {
    run_live_postgres_test("audit_repository", |pool| async move {
        seed_user(&pool, "tenant_audit", "user_audit").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let second = AuditEvent::dry_run(
            "evt_2",
            "trace_audit",
            2,
            1_748_250_002_000,
            actor("user_audit"),
            scope("tenant_audit"),
            target("progress_audit"),
            Some(summary("before")),
            Some(summary("projected")),
        );
        let first = AuditEvent::confirmed_action(
            "evt_1",
            "trace_audit",
            1,
            1_748_250_001_000,
            actor("user_audit"),
            scope("tenant_audit"),
            target("progress_audit"),
            summary("confirmed"),
        );

        repository.append(&second, None).await?;
        repository.append(&first, None).await?;

        let events = repository.find_by_trace_id("trace_audit").await?;
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
fn postgres_live_execution_uow_commits_ledger_audit_and_outbox_atomically() {
    run_live_postgres_test("execution_uow_commit", |pool| async move {
        seed_user(&pool, "tenant_uow", "user_uow").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action("action_uow", "tenant_uow", "user_uow", "idem_uow");
        let event = AuditEvent::confirmed_action(
            "evt_uow_1",
            "trace_uow",
            1,
            1_748_250_001_000,
            actor("user_uow"),
            scope("tenant_uow"),
            target("progress_uow"),
            summary("confirmed by reviewer"),
        );
        let outbox = outbox_envelope("tenant_uow", "trace_uow", 1_748_250_010_000);

        let report = uow
            .record_confirmation(&action, 1_748_250_000_000, "op_uow", &event, &outbox)
            .await?;

        assert_eq!(report.operation.operation_id, "op_uow");
        assert!(!report.duplicate);
        let outbox_id = report.outbox_id.expect("outbox should be enqueued");
        assert!(outbox_id > 0);

        let operation = ledger
            .get_by_idempotency_key("tenant_uow", "idem_uow")
            .await?
            .expect("operation should commit");
        assert_eq!(operation.operation_id, "op_uow");

        let events = audit.find_by_trace_id("trace_uow").await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_uow_1");

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
        assert_eq!(aggregate_id, "trace_uow");
        assert_eq!(status, "pending");

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_duplicate_confirmation_skips_side_effects() {
    run_live_postgres_test("execution_uow_duplicate_confirmation", |pool| async move {
        seed_user(&pool, "tenant_uow_dup", "user_uow_dup").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_dup",
            "tenant_uow_dup",
            "user_uow_dup",
            "idem_uow_dup",
        );
        let first_event = AuditEvent::confirmed_action(
            "evt_uow_dup_1",
            "trace_uow_dup",
            1,
            1_748_250_001_000,
            actor("user_uow_dup"),
            scope("tenant_uow_dup"),
            target("progress_uow_dup"),
            summary("first confirmation"),
        );
        let second_event = AuditEvent::confirmed_action(
            "evt_uow_dup_2",
            "trace_uow_dup",
            2,
            1_748_250_002_000,
            actor("user_uow_dup"),
            scope("tenant_uow_dup"),
            target("progress_uow_dup"),
            summary("duplicate confirmation"),
        );

        let first = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_dup",
                &first_event,
                &outbox_envelope("tenant_uow_dup", "trace_uow_dup", 1_748_250_010_000),
            )
            .await?;
        let duplicate = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_dup_retry",
                &second_event,
                &outbox_envelope("tenant_uow_dup", "trace_uow_dup", 1_748_250_011_000),
            )
            .await?;

        assert!(!first.duplicate);
        assert!(first.outbox_id.is_some());
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.outbox_id, None);
        assert_eq!(duplicate.operation.operation_id, "op_uow_dup");

        let events = audit.find_by_trace_id("trace_uow_dup").await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_uow_dup_1");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_dup")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_rejects_cross_tenant_event_and_outbox() {
    run_live_postgres_test("execution_uow_tenant_mismatch", |pool| async move {
        seed_user(&pool, "tenant_uow_safe", "user_uow_safe").await?;
        seed_user(&pool, "tenant_uow_other", "user_uow_other").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_safe",
            "tenant_uow_safe",
            "user_uow_safe",
            "idem_uow_safe",
        );
        let wrong_event = AuditEvent::confirmed_action(
            "evt_uow_wrong_tenant",
            "trace_uow_wrong_tenant",
            1,
            1_748_250_001_000,
            actor("user_uow_other"),
            scope("tenant_uow_other"),
            target("progress_uow_wrong_tenant"),
            summary("wrong tenant event"),
        );

        let result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_safe",
                &wrong_event,
                &outbox_envelope(
                    "tenant_uow_safe",
                    "trace_uow_wrong_tenant",
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
            .get_by_idempotency_key("tenant_uow_safe", "idem_uow_safe")
            .await?;
        assert_eq!(operation, None);

        let wrong_outbox_result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_safe",
                &AuditEvent::confirmed_action(
                    "evt_uow_correct_tenant",
                    "trace_uow_wrong_outbox",
                    1,
                    1_748_250_001_000,
                    actor("user_uow_safe"),
                    scope("tenant_uow_safe"),
                    target("progress_uow_wrong_outbox"),
                    summary("correct tenant event"),
                ),
                &outbox_envelope(
                    "tenant_uow_other",
                    "trace_uow_wrong_outbox",
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
            .get_by_idempotency_key("tenant_uow_safe", "idem_uow_safe")
            .await?;
        assert_eq!(operation, None);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_records_dry_run_and_success_terminal_idempotently() {
    run_live_postgres_test("execution_uow_success", |pool| async move {
        seed_user(&pool, "tenant_uow_success", "user_uow_success").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_success",
            "tenant_uow_success",
            "user_uow_success",
            "idem_uow_success",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_success",
            &AuditEvent::confirmed_action(
                "evt_uow_success_1",
                "trace_uow_success",
                1,
                1_748_250_001_000,
                actor("user_uow_success"),
                scope("tenant_uow_success"),
                target("progress_uow_success"),
                summary("confirmed"),
            ),
            &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_010_000),
        )
        .await?;

        let dry_run = uow
            .record_dry_run(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    "evt_uow_success_2",
                    "trace_uow_success",
                    2,
                    1_748_250_002_000,
                    actor("user_uow_success"),
                    scope("tenant_uow_success"),
                    target("progress_uow_success"),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_011_000),
            )
            .await?;
        assert_eq!(dry_run.operation.status, ActionStatus::Executing);
        assert!(!dry_run.duplicate);
        assert!(dry_run.outbox_id.is_some());

        let success = uow
            .record_success(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    "evt_uow_success_3",
                    "trace_uow_success",
                    3,
                    1_748_250_003_000,
                    actor("user_uow_success"),
                    scope("tenant_uow_success"),
                    target("progress_uow_success"),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_success",
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_012_000),
            )
            .await?;
        assert_eq!(success.operation.status, ActionStatus::Succeeded);
        assert!(!success.duplicate);
        assert!(success.outbox_id.is_some());

        let duplicate_success = uow
            .record_success(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_004_000,
                &AuditEvent::execution_succeeded(
                    "evt_uow_success_4",
                    "trace_uow_success",
                    4,
                    1_748_250_004_000,
                    actor("user_uow_success"),
                    scope("tenant_uow_success"),
                    target("progress_uow_success"),
                    Some(summary("before")),
                    Some(summary("applied again")),
                    "lark_op_success_retry",
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_013_000),
            )
            .await?;
        assert_eq!(duplicate_success.operation.status, ActionStatus::Succeeded);
        assert!(duplicate_success.duplicate);
        assert_eq!(duplicate_success.outbox_id, None);

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_success", "idem_uow_success")
            .await?
            .expect("operation should exist");
        assert_eq!(operation.status, ActionStatus::Succeeded);

        let events = audit.find_by_trace_id("trace_uow_success").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, "evt_uow_success_1");
        assert_eq!(events[1].event_id, "evt_uow_success_2");
        assert_eq!(events[2].event_id, "evt_uow_success_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_success")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_records_failure_terminal_idempotently() {
    run_live_postgres_test("execution_uow_failure", |pool| async move {
        seed_user(&pool, "tenant_uow_failure", "user_uow_failure").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_failure",
            "tenant_uow_failure",
            "user_uow_failure",
            "idem_uow_failure",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_failure",
            &AuditEvent::confirmed_action(
                "evt_uow_failure_1",
                "trace_uow_failure",
                1,
                1_748_250_001_000,
                actor("user_uow_failure"),
                scope("tenant_uow_failure"),
                target("progress_uow_failure"),
                summary("confirmed"),
            ),
            &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_010_000),
        )
        .await?;
        uow.record_dry_run(
            "tenant_uow_failure",
            "idem_uow_failure",
            1_748_250_002_000,
            &AuditEvent::dry_run(
                "evt_uow_failure_2",
                "trace_uow_failure",
                2,
                1_748_250_002_000,
                actor("user_uow_failure"),
                scope("tenant_uow_failure"),
                target("progress_uow_failure"),
                Some(summary("before")),
                Some(summary("projected")),
            ),
            &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_011_000),
        )
        .await?;

        let failed = uow
            .record_failure(
                "tenant_uow_failure",
                "idem_uow_failure",
                "adapter timeout",
                1_748_250_003_000,
                &AuditEvent::execution_failed(
                    "evt_uow_failure_3",
                    "trace_uow_failure",
                    3,
                    1_748_250_003_000,
                    actor("user_uow_failure"),
                    scope("tenant_uow_failure"),
                    target("progress_uow_failure"),
                    Some(summary("before")),
                    None,
                    "adapter_timeout",
                    "adapter timeout",
                ),
                &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_012_000),
            )
            .await?;
        assert_eq!(failed.operation.status, ActionStatus::Failed);
        assert_eq!(
            failed.operation.last_error.as_deref(),
            Some("adapter timeout")
        );
        assert!(failed.outbox_id.is_some());

        let duplicate_failed = uow
            .record_failure(
                "tenant_uow_failure",
                "idem_uow_failure",
                "different retry error",
                1_748_250_004_000,
                &AuditEvent::execution_failed(
                    "evt_uow_failure_4",
                    "trace_uow_failure",
                    4,
                    1_748_250_004_000,
                    actor("user_uow_failure"),
                    scope("tenant_uow_failure"),
                    target("progress_uow_failure"),
                    Some(summary("before")),
                    None,
                    "adapter_retry_timeout",
                    "different retry error",
                ),
                &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_013_000),
            )
            .await?;
        assert!(duplicate_failed.duplicate);
        assert_eq!(duplicate_failed.outbox_id, None);
        assert_eq!(
            duplicate_failed.operation.last_error.as_deref(),
            Some("adapter timeout")
        );

        let events = audit.find_by_trace_id("trace_uow_failure").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "evt_uow_failure_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_failure")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("execution_uow_rollback", |pool| async move {
        seed_user(&pool, "tenant_uow_rollback", "user_uow_rollback").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_rollback",
            "tenant_uow_rollback",
            "user_uow_rollback",
            "idem_uow_rollback",
        );
        let event = AuditEvent::confirmed_action(
            "evt_duplicate",
            "trace_uow_rollback",
            1,
            1_748_250_001_000,
            actor("user_uow_rollback"),
            scope("tenant_uow_rollback"),
            target("progress_uow_rollback"),
            summary("confirmed by reviewer"),
        );

        audit.append(&event, None).await?;

        let result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_rollback",
                &event,
                &outbox_envelope(
                    "tenant_uow_rollback",
                    "trace_uow_rollback",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should fail the whole transaction"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_rollback", "idem_uow_rollback")
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
        .bind("tenant_uow_rollback")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 0, "outbox enqueue must roll back too");

        Ok(())
    });
}
