#![cfg(feature = "postgres")]

use std::collections::VecDeque;
use std::env;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};
use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::execution_policy::{ExecutionDenied, ExecutionPolicy};
use oar_core::action::executor::{
    ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution, ExecutionError,
};
use oar_core::action::operation_ledger::{LedgerError, SubmitResult};
use oar_core::action::postgres_executor::PostgresActionExecutor;
use oar_core::action::token_refresh_audit::{token_refresh_audit_event, TokenRefreshAuditContext};
use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession, SessionState};
use oar_core::domain::identity::{
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, OAuthTokens, OarUser, OarUserId,
    OarUserStatus, ScopeBoundary, SecretString, Tenant, TenantId, TenantStatus, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::{
    AuthRefreshAdapter, EncryptedGrantBlob, EncryptedGrantMaterial, RefreshOutcome,
    TokenRefreshAuditSummary, TokenRefreshCommandKind, TokenRefreshCommandReport,
    TokenRefreshDecisionKind, TokenRefreshGrantSnapshot, TokenRefreshPlannedCommand,
    TokenRefreshReportStatus, TokenRefreshRepositoryCommand,
};
use oar_core::lark::auth::{
    parse_lark_auth_refresh_response, LarkAuthRefreshAdapter, LarkAuthRefreshClient,
    LarkAuthRefreshRequest, LarkAuthRefreshResponse,
};
use oar_core::lark::fixtures::{
    AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON, AUTH_REFRESH_REAUTH_REQUIRED_JSON,
    AUTH_REFRESH_ROTATED_ENCRYPTED_JSON,
};
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher, AuditOutboxDrainConfig, PostgresAuditOutboxWorker,
};
use oar_core::storage::postgres::{
    AuditOutboxEnvelope, AuditOutboxMessage, EncryptedTokenGrantRecord,
    PostgresAuditEventRepository, PostgresDeviceSessionRepository, PostgresExecutionUnitOfWork,
    PostgresLarkIdentityRepository, PostgresOarUserRepository, PostgresOperationLedgerRepository,
    PostgresRepositoryError, PostgresTenantRepository, PostgresTokenGrantRepository,
    PostgresTokenRefreshOrchestrator, PostgresTokenRefreshSweep, PostgresTokenRefreshSweepRequest,
    PostgresTokenRefreshUnitOfWork, RotateEncryptedGrantRequest,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool, Row};

const MIGRATION_SQL: &str = include_str!("../migrations/0001_phase_0_6_identity_action_audit.sql");

static SCHEMA_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
struct LiveMockAdapter {
    state: Arc<Mutex<LiveMockAdapterState>>,
}

#[derive(Default)]
struct LiveMockAdapterState {
    dry_run_calls: usize,
    execute_calls: usize,
    execute_error: Option<AdapterError>,
}

impl LiveMockAdapter {
    fn succeeding() -> Self {
        Self::default()
    }

    fn failing(code: &str, message: &str) -> Self {
        let adapter = Self::default();
        adapter.state.lock().expect("adapter mutex").execute_error =
            Some(AdapterError::new(code, message));
        adapter
    }

    fn dry_run_calls(&self) -> usize {
        self.state.lock().expect("adapter mutex").dry_run_calls
    }

    fn execute_calls(&self) -> usize {
        self.state.lock().expect("adapter mutex").execute_calls
    }
}

impl ActionAdapter for LiveMockAdapter {
    fn dry_run(&mut self, _action: &ConfirmedAction) -> Result<AdapterDryRun, AdapterError> {
        self.state.lock().expect("adapter mutex").dry_run_calls += 1;
        Ok(AdapterDryRun {
            before: Some(summary("before")),
            after: Some(summary("dry-run projected")),
        })
    }

    fn execute(&mut self, _action: &ConfirmedAction) -> Result<AdapterExecution, AdapterError> {
        let mut state = self.state.lock().expect("adapter mutex");
        state.execute_calls += 1;
        if let Some(error) = state.execute_error.clone() {
            return Err(error);
        }

        Ok(AdapterExecution {
            adapter_operation_id: "lark-op-live".to_string(),
            before: Some(summary("before")),
            after: Some(summary("applied")),
        })
    }
}

#[derive(Clone)]
struct LiveOutboxDispatcher {
    outcomes: Arc<Mutex<Vec<AuditOutboxDelivery>>>,
}

impl LiveOutboxDispatcher {
    fn new(outcomes: impl IntoIterator<Item = AuditOutboxDelivery>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
        }
    }
}

impl AuditOutboxDispatcher for LiveOutboxDispatcher {
    type Error = ();

    async fn deliver(
        &mut self,
        _message: &AuditOutboxMessage,
    ) -> Result<AuditOutboxDelivery, Self::Error> {
        let mut outcomes = self.outcomes.lock().expect("outbox dispatcher mutex");
        if outcomes.is_empty() {
            return Ok(AuditOutboxDelivery::Sent);
        }

        Ok(outcomes.remove(0))
    }
}

#[derive(Clone)]
struct LiveRefreshAdapter {
    outcome: RefreshOutcome,
    calls: Arc<Mutex<usize>>,
}

impl LiveRefreshAdapter {
    fn new(outcome: RefreshOutcome) -> Self {
        Self {
            outcome,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().expect("refresh adapter mutex")
    }
}

impl AuthRefreshAdapter for LiveRefreshAdapter {
    fn refresh(&mut self, _snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let mut calls = self.calls.lock().expect("refresh adapter mutex");
        *calls += 1;
        self.outcome.clone()
    }
}

#[derive(Clone)]
struct SequenceRefreshAdapter {
    outcomes: Arc<Mutex<VecDeque<RefreshOutcome>>>,
    called_grant_ids: Arc<Mutex<Vec<String>>>,
}

impl SequenceRefreshAdapter {
    fn new(outcomes: impl IntoIterator<Item = RefreshOutcome>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
            called_grant_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn called_grant_ids(&self) -> Vec<String> {
        self.called_grant_ids
            .lock()
            .expect("sequence refresh adapter mutex")
            .clone()
    }
}

impl AuthRefreshAdapter for SequenceRefreshAdapter {
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        self.called_grant_ids
            .lock()
            .expect("sequence refresh adapter calls mutex")
            .push(snapshot.grant_id.0.clone());
        self.outcomes
            .lock()
            .expect("sequence refresh adapter outcomes mutex")
            .pop_front()
            .expect("sequence refresh outcome")
    }
}

#[derive(Clone)]
struct FixtureClient {
    fixture: &'static str,
    calls: Arc<Mutex<usize>>,
}

impl FixtureClient {
    fn new(fixture: &'static str) -> Self {
        Self {
            fixture,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().expect("fixture client mutex")
    }
}

impl LarkAuthRefreshClient for FixtureClient {
    type Error = &'static str;

    fn refresh(
        &mut self,
        _request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error> {
        let mut calls = self.calls.lock().expect("fixture client mutex");
        *calls += 1;
        parse_lark_auth_refresh_response(self.fixture).map_err(|_| "fixture_parse_failed")
    }
}

fn assert_no_auth_refresh_sensitive_payload(payload_text: &str) {
    for needle in [
        "tok_",
        "access_token",
        "refresh_token",
        "authorization_code",
        "Authorization",
        "Bearer",
        "encrypted_primary",
        "encrypted_renewal",
        "fp_prev_v1",
        "fp_rotated_v2",
    ] {
        assert!(
            !payload_text.contains(needle),
            "audit payload leaked auth refresh marker: {needle}"
        );
    }
}

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

fn audit_context(
    event_id: &str,
    trace_id: &str,
    sequence: u64,
    occurred_at_ms: u64,
    actor_id: &str,
    tenant_id: &str,
    resource_id: &str,
) -> AuditEventContext {
    AuditEventContext {
        event_id: event_id.to_string(),
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms,
        subject: AuditSubject {
            actor: actor(actor_id),
            scope: scope(tenant_id),
            target: target(resource_id),
        },
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

fn postgres_action_executor(
    pool: PgPool,
    adapter: LiveMockAdapter,
) -> PostgresActionExecutor<LiveMockAdapter, impl FnMut() -> u64> {
    let mut tick = 1_748_260_000_000_u64;
    PostgresActionExecutor::new(
        adapter,
        move || {
            tick += 1_000;
            tick
        },
        PostgresExecutionUnitOfWork::new(pool.clone()),
        PostgresAuditEventRepository::new(pool),
    )
}

fn token_grant(tenant_id: &str, scopes: &[&str], state: TokenGrantState) -> TokenGrant {
    TokenGrant {
        id: TokenGrantId("grant_live".to_string()),
        tenant_id: TenantId(tenant_id.to_string()),
        identity_id: LarkIdentityId("identity_live".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        state,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-token"),
            refresh_token: Some(SecretString::new("refresh-token")),
        },
        revocation_reason: None,
    }
}

fn progress_update_policy() -> ExecutionPolicy {
    ExecutionPolicy::new(
        ["okr.progress.update"],
        [ActorKind::User, ActorKind::Service],
    )
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

async fn seed_identity(
    pool: &PgPool,
    tenant_id: &str,
    identity_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO lark_identities (id, tenant_id, actor_kind, actor_external_id, display_name)
        VALUES ($1, $2, 'user', $3, $4)
        "#,
    )
    .bind(identity_id)
    .bind(tenant_id)
    .bind(format!("ext_{identity_id}"))
    .bind(format!("Identity {identity_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

fn encrypted_token_grant_record(
    tenant_id: &str,
    id: &str,
    identity_id: &str,
    state: TokenGrantState,
    fingerprint: &str,
) -> EncryptedTokenGrantRecord {
    EncryptedTokenGrantRecord {
        id: id.to_string(),
        tenant_id: tenant_id.to_string(),
        identity_id: identity_id.to_string(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["okr.progress.write".to_string()],
        state,
        issued_at_ms: 1_748_250_000_000,
        expires_at_ms: Some(1_748_260_000_000),
        refreshed_at_ms: Some(1_748_250_000_000),
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: Some("old-error".to_string()),
        encrypted_oauth_grant: vec![0x01, 0x02, 0x03],
        oauth_grant_key_id: "key-v1".to_string(),
        oauth_grant_fingerprint: fingerprint.to_string(),
        revocation_reason: None,
    }
}

fn rotate_grant_request<'a>(
    tenant_id: &'a str,
    id: &'a str,
    expected_fingerprint: &'a str,
    encrypted_oauth_grant: &'a [u8],
) -> RotateEncryptedGrantRequest<'a> {
    RotateEncryptedGrantRequest {
        tenant_id,
        id,
        expected_fingerprint,
        expires_at_ms: Some(1_748_270_000_000),
        refreshed_at_ms: 1_748_260_500_000,
        encrypted_oauth_grant,
        oauth_grant_key_id: "key-v2",
        oauth_grant_fingerprint: "fp-new",
    }
}

fn planned_token_refresh_command(
    command: TokenRefreshRepositoryCommand,
) -> TokenRefreshPlannedCommand {
    let (grant_id, tenant_id) = match &command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            ..
        }
        | TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            ..
        }
        | TokenRefreshRepositoryCommand::MarkReauthRequired {
            grant_id,
            tenant_id,
            ..
        } => (grant_id.clone(), tenant_id.clone()),
    };
    let command_kind = command.kind();
    let safe_error = match &command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. }
        | TokenRefreshRepositoryCommand::MarkReauthRequired { safe_error, .. } => {
            Some(safe_error.clone())
        }
        TokenRefreshRepositoryCommand::RotateGrantCas { .. } => None,
    };

    TokenRefreshPlannedCommand {
        command,
        report: TokenRefreshCommandReport {
            grant_id,
            tenant_id,
            decision_kind: match command_kind {
                TokenRefreshCommandKind::RotateGrantCas => TokenRefreshDecisionKind::RotateGrantCas,
                TokenRefreshCommandKind::MarkNeedsRefresh => {
                    TokenRefreshDecisionKind::MarkNeedsRefresh
                }
                TokenRefreshCommandKind::MarkReauthRequired => {
                    TokenRefreshDecisionKind::MarkReauthRequired
                }
            },
            command_kind,
            safe_error,
        },
    }
}

fn device_session(
    tenant_id: &str,
    user_id: &str,
    session_id: &str,
    stream: &str,
    cursor: u64,
    now: SystemTime,
) -> DeviceSession {
    DeviceSession::new(
        DeviceSessionId(session_id.to_string()),
        TenantId(tenant_id.to_string()),
        OarUserId(user_id.to_string()),
        DeviceEntryPoint::MacOs,
        stream.to_string(),
        cursor,
        now,
    )
}

async fn audit_outbox_count(pool: &PgPool, tenant_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_outbox
        WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
}

#[test]
fn postgres_live_action_executor_records_success_audit_and_outbox() {
    run_live_postgres_test("action_executor_success", |pool| async move {
        seed_user(&pool, "tenant_executor_success", "user_executor_success").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_executor_success",
            "tenant_executor_success",
            "user_executor_success",
            "idem_executor_success",
        );

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Succeeded);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 3);
        assert_eq!(
            report.events[0].event_type,
            AuditEventType::ConfirmedActionRecorded
        );
        assert_eq!(report.events[1].event_type, AuditEventType::DryRunExecuted);
        assert_eq!(
            report.events[2].event_type,
            AuditEventType::ExecutionSucceeded
        );
        assert_eq!(
            report.events[2]
                .execution
                .as_ref()
                .and_then(|execution| execution.adapter_operation_id.as_deref()),
            Some("lark-op-live")
        );

        let persisted = audit
            .find_by_trace_id("trace-idem_executor_success")
            .await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_success").await?,
            3
        );

        let linked_event_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_events
            WHERE trace_id = $1 AND operation_id = $2
            "#,
        )
        .bind("trace-idem_executor_success")
        .bind(&report.operation.operation_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(linked_event_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_duplicate_retry_skips_adapter_and_side_effects() {
    run_live_postgres_test("action_executor_duplicate", |pool| async move {
        seed_user(&pool, "tenant_executor_dup", "user_executor_dup").await?;

        let adapter = LiveMockAdapter::succeeding();
        let action = confirmed_action(
            "action_executor_dup",
            "tenant_executor_dup",
            "user_executor_dup",
            "idem_executor_dup",
        );
        let mut first_executor = postgres_action_executor(pool.clone(), adapter.clone());
        let mut retry_executor = postgres_action_executor(pool.clone(), adapter.clone());

        let first = first_executor
            .execute_confirmed_action(&action)
            .await
            .unwrap();
        let retry = retry_executor
            .execute_confirmed_action(&action)
            .await
            .unwrap();

        assert!(!first.duplicate);
        assert!(retry.duplicate);
        assert!(retry.events.is_empty());
        assert_eq!(first.operation.operation_id, retry.operation.operation_id);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let events = audit.find_by_trace_id("trace-idem_executor_dup").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(audit_outbox_count(&pool, "tenant_executor_dup").await?, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_resumes_after_confirmation_only_crash() {
    run_live_postgres_test("action_executor_confirmation_resume", |pool| async move {
        seed_user(&pool, "tenant_executor_resume", "user_executor_resume").await?;

        let action = confirmed_action(
            "action_executor_resume",
            "tenant_executor_resume",
            "user_executor_resume",
            "idem_executor_resume",
        );
        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        uow.record_confirmation(
            &action,
            1_748_260_000_000,
            "op-idem_executor_resume",
            &AuditEvent::confirmed_action(
                audit_context(
                    "trace-idem_executor_resume-evt-1",
                    "trace-idem_executor_resume",
                    1,
                    1_748_260_001_000,
                    "user_executor_resume",
                    "tenant_executor_resume",
                    "action_executor_resume",
                ),
                summary("confirmed before crash"),
            ),
            &outbox_envelope(
                "tenant_executor_resume",
                "trace-idem_executor_resume",
                1_748_260_002_000,
            ),
        )
        .await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Succeeded);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 2);
        assert_eq!(report.events[0].event_type, AuditEventType::DryRunExecuted);
        assert_eq!(
            report.events[1].event_type,
            AuditEventType::ExecutionSucceeded
        );

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit.find_by_trace_id("trace-idem_executor_resume").await?;
        assert_eq!(persisted.len(), 3);
        assert_eq!(
            persisted
                .iter()
                .map(|event| event.event_type.clone())
                .collect::<Vec<_>>(),
            vec![
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
                AuditEventType::ExecutionSucceeded
            ]
        );
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_resume").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_records_adapter_failure_as_terminal_state() {
    run_live_postgres_test("action_executor_failure", |pool| async move {
        seed_user(&pool, "tenant_executor_failure", "user_executor_failure").await?;

        let adapter = LiveMockAdapter::failing("adapter_timeout", "network timeout");
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_executor_failure",
            "tenant_executor_failure",
            "user_executor_failure",
            "idem_executor_failure",
        );

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Failed);
        assert_eq!(
            report.operation.last_error.as_deref(),
            Some("network timeout")
        );
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 3);
        assert_eq!(report.events[2].event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report.events[2]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("adapter_timeout")
        );

        let persisted = audit
            .find_by_trace_id("trace-idem_executor_failure")
            .await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_failure").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_policy_denial_records_safe_audit_without_adapter_call() {
    run_live_postgres_test("action_executor_policy_denied", |pool| async move {
        seed_user(&pool, "tenant_executor_policy", "user_executor_policy").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let action = confirmed_action(
            "action_executor_policy",
            "tenant_executor_policy",
            "user_executor_policy",
            "idem_executor_policy",
        );
        let policy = progress_update_policy();
        let grant = token_grant(
            "tenant_executor_policy",
            &["offline_access"],
            TokenGrantState::Valid,
        );

        let result = executor
            .execute_confirmed_action_with_policy(
                &action,
                "okr.progress.update",
                "okr.progress.write",
                &grant,
                &policy,
            )
            .await;

        assert_eq!(adapter.dry_run_calls(), 0);
        assert_eq!(adapter.execute_calls(), 0);

        let report = match result {
            Err(ExecutionError::PolicyDenied(report)) => report,
            other => panic!("expected policy denial, got {other:?}"),
        };
        assert_eq!(
            report.denial,
            ExecutionDenied::MissingScope {
                required_scope: "okr.progress.write".to_string()
            }
        );
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].event_type, AuditEventType::ExecutionDenied);
        assert_eq!(
            report.events[0]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("policy_denied")
        );
        let message = report.events[0]
            .execution
            .as_ref()
            .and_then(|execution| execution.message.as_deref())
            .unwrap_or_default();
        assert!(message.contains("policy"));
        assert!(message.contains("okr.progress.write"));
        assert!(!message.contains("access-token"));
        assert!(!message.contains("refresh-token"));

        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        assert_eq!(
            ledger
                .get_by_idempotency_key("tenant_executor_policy", "idem_executor_policy")
                .await?,
            None
        );

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit.find_by_trace_id("trace-idem_executor_policy").await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_policy").await?,
            0
        );

        Ok(())
    });
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
            .find_by_trace_id("trace_token_refresh_audit")
            .await?;
        assert_eq!(events.len(), 1);

        let persisted = &events[0];
        assert_eq!(persisted.event_type, AuditEventType::ExecutionSucceeded);
        assert_eq!(persisted.scope.tenant_id, "tenant_refresh_audit");
        assert_eq!(persisted.target.resource_type, "token_grant");
        assert_eq!(persisted.target.action_type, "token_refresh.rotate");

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT payload
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
            AuditOutboxDrainConfig::new("tenant_outbox_worker", "audit-events", 10, 3_000, 7_000),
        );

        let report = worker.drain_once().await?;

        assert_eq!(report.claimed, 3);
        assert_eq!(report.sent, 1);
        assert_eq!(report.retryable, 1);
        assert_eq!(report.failed, 1);
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

#[test]
fn postgres_live_execution_uow_commits_ledger_audit_and_outbox_atomically() {
    run_live_postgres_test("execution_uow_commit", |pool| async move {
        seed_user(&pool, "tenant_uow", "user_uow").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action("action_uow", "tenant_uow", "user_uow", "idem_uow");
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_1",
                "trace_uow",
                1,
                1_748_250_001_000,
                "user_uow",
                "tenant_uow",
                "progress_uow",
            ),
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
            audit_context(
                "evt_uow_dup_1",
                "trace_uow_dup",
                1,
                1_748_250_001_000,
                "user_uow_dup",
                "tenant_uow_dup",
                "progress_uow_dup",
            ),
            summary("first confirmation"),
        );
        let second_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_dup_2",
                "trace_uow_dup",
                2,
                1_748_250_002_000,
                "user_uow_dup",
                "tenant_uow_dup",
                "progress_uow_dup",
            ),
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
            audit_context(
                "evt_uow_wrong_tenant",
                "trace_uow_wrong_tenant",
                1,
                1_748_250_001_000,
                "user_uow_other",
                "tenant_uow_other",
                "progress_uow_wrong_tenant",
            ),
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
                    audit_context(
                        "evt_uow_correct_tenant",
                        "trace_uow_wrong_outbox",
                        1,
                        1_748_250_001_000,
                        "user_uow_safe",
                        "tenant_uow_safe",
                        "progress_uow_wrong_outbox",
                    ),
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
                audit_context(
                    "evt_uow_success_1",
                    "trace_uow_success",
                    1,
                    1_748_250_001_000,
                    "user_uow_success",
                    "tenant_uow_success",
                    "progress_uow_success",
                ),
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
                    audit_context(
                        "evt_uow_success_2",
                        "trace_uow_success",
                        2,
                        1_748_250_002_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
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
                    audit_context(
                        "evt_uow_success_3",
                        "trace_uow_success",
                        3,
                        1_748_250_003_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
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
                    audit_context(
                        "evt_uow_success_4",
                        "trace_uow_success",
                        4,
                        1_748_250_004_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
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
                audit_context(
                    "evt_uow_failure_1",
                    "trace_uow_failure",
                    1,
                    1_748_250_001_000,
                    "user_uow_failure",
                    "tenant_uow_failure",
                    "progress_uow_failure",
                ),
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
                audit_context(
                    "evt_uow_failure_2",
                    "trace_uow_failure",
                    2,
                    1_748_250_002_000,
                    "user_uow_failure",
                    "tenant_uow_failure",
                    "progress_uow_failure",
                ),
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
                    audit_context(
                        "evt_uow_failure_3",
                        "trace_uow_failure",
                        3,
                        1_748_250_003_000,
                        "user_uow_failure",
                        "tenant_uow_failure",
                        "progress_uow_failure",
                    ),
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
                    audit_context(
                        "evt_uow_failure_4",
                        "trace_uow_failure",
                        4,
                        1_748_250_004_000,
                        "user_uow_failure",
                        "tenant_uow_failure",
                        "progress_uow_failure",
                    ),
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
fn postgres_live_execution_uow_reports_explicit_invalid_transition() {
    run_live_postgres_test("execution_uow_invalid_transition", |pool| async move {
        seed_user(
            &pool,
            "tenant_uow_invalid_transition",
            "user_uow_invalid_transition",
        )
        .await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_invalid_transition",
            "tenant_uow_invalid_transition",
            "user_uow_invalid_transition",
            "idem_uow_invalid_transition",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_invalid_transition",
            &AuditEvent::confirmed_action(
                audit_context(
                    "evt_uow_invalid_transition_1",
                    "trace_uow_invalid_transition",
                    1,
                    1_748_250_001_000,
                    "user_uow_invalid_transition",
                    "tenant_uow_invalid_transition",
                    "progress_uow_invalid_transition",
                ),
                summary("confirmed"),
            ),
            &outbox_envelope(
                "tenant_uow_invalid_transition",
                "trace_uow_invalid_transition",
                1_748_250_010_000,
            ),
        )
        .await?;

        let result = uow
            .record_success(
                "tenant_uow_invalid_transition",
                "idem_uow_invalid_transition",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_uow_invalid_transition_2",
                        "trace_uow_invalid_transition",
                        2,
                        1_748_250_003_000,
                        "user_uow_invalid_transition",
                        "tenant_uow_invalid_transition",
                        "progress_uow_invalid_transition",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_invalid_transition",
                ),
                &outbox_envelope(
                    "tenant_uow_invalid_transition",
                    "trace_uow_invalid_transition",
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
            .find_by_trace_id("trace_uow_invalid_transition")
            .await?;
        assert_eq!(events.len(), 1);

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_invalid_transition")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

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
            audit_context(
                "evt_duplicate",
                "trace_uow_rollback",
                1,
                1_748_250_001_000,
                "user_uow_rollback",
                "tenant_uow_rollback",
                "progress_uow_rollback",
            ),
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

#[test]
fn postgres_live_token_grant_rotate_cas_succeeds_and_updates_fields() {
    run_live_postgres_test("token_grant_rotate_success", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_ok", "user_tg_rotate_ok").await?;
        seed_identity(&pool, "tenant_tg_rotate_ok", "identity_tg_rotate_ok").await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_ok",
            "grant_tg_rotate_ok",
            "identity_tg_rotate_ok",
            TokenGrantState::NeedsRefresh,
            "fp-old",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_ok",
                "grant_tg_rotate_ok",
                "fp-old",
                &[0xAA, 0xBB, 0xCC],
            ))
            .await?
            .expect("rotation should return updated row");

        assert_eq!(rotated.state, TokenGrantState::Valid);
        assert_eq!(rotated.oauth_grant_fingerprint, "fp-new");
        assert_eq!(rotated.oauth_grant_key_id, "key-v2");
        assert_eq!(rotated.encrypted_oauth_grant, vec![0xAA, 0xBB, 0xCC]);
        assert_eq!(rotated.expires_at_ms, Some(1_748_270_000_000));
        assert_eq!(rotated.refreshed_at_ms, Some(1_748_260_500_000));
        assert_eq!(rotated.last_refresh_error, None);
        assert_eq!(rotated.revoked_at_ms, None);
        assert_eq!(rotated.reauth_required_at_ms, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_with_stale_fingerprint_is_noop() {
    run_live_postgres_test("token_grant_rotate_stale_fp", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_stale", "user_tg_rotate_stale").await?;
        seed_identity(&pool, "tenant_tg_rotate_stale", "identity_tg_rotate_stale").await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_stale",
            "grant_tg_rotate_stale",
            "identity_tg_rotate_stale",
            TokenGrantState::Valid,
            "fp-current",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_stale",
                "grant_tg_rotate_stale",
                "fp-stale",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_blocked_after_revoke() {
    run_live_postgres_test("token_grant_rotate_blocked_revoked", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_revoked", "user_tg_rotate_revoked").await?;
        seed_identity(
            &pool,
            "tenant_tg_rotate_revoked",
            "identity_tg_rotate_revoked",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_revoked",
            "grant_tg_rotate_revoked",
            "identity_tg_rotate_revoked",
            TokenGrantState::Valid,
            "fp-revoked",
        );
        repository.upsert_encrypted_grant(&initial).await?;
        repository
            .revoke(
                "tenant_tg_rotate_revoked",
                "grant_tg_rotate_revoked",
                1_748_260_000_000,
                "user disconnected",
            )
            .await?
            .expect("revoke should update row");

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_revoked",
                "grant_tg_rotate_revoked",
                "fp-revoked",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_blocked_after_reauth_required() {
    run_live_postgres_test("token_grant_rotate_blocked_reauth", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_reauth", "user_tg_rotate_reauth").await?;
        seed_identity(
            &pool,
            "tenant_tg_rotate_reauth",
            "identity_tg_rotate_reauth",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_reauth",
            "grant_tg_rotate_reauth",
            "identity_tg_rotate_reauth",
            TokenGrantState::Valid,
            "fp-reauth",
        );
        repository.upsert_encrypted_grant(&initial).await?;
        repository
            .mark_reauth_required(
                "tenant_tg_rotate_reauth",
                "grant_tg_rotate_reauth",
                "fp-reauth",
                1_748_260_000_000,
                "invalid_grant",
            )
            .await?
            .expect("mark reauth required should update row");

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_reauth",
                "grant_tg_rotate_reauth",
                "fp-reauth",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_mark_refresh_failed_sets_needs_refresh_and_error() {
    run_live_postgres_test("token_grant_refresh_failed", |pool| async move {
        seed_user(&pool, "tenant_tg_refresh_failed", "user_tg_refresh_failed").await?;
        seed_identity(
            &pool,
            "tenant_tg_refresh_failed",
            "identity_tg_refresh_failed",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_refresh_failed",
            "grant_tg_refresh_failed",
            "identity_tg_refresh_failed",
            TokenGrantState::Valid,
            "fp-refresh-fail",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let updated = repository
            .mark_refresh_failed(
                "tenant_tg_refresh_failed",
                "grant_tg_refresh_failed",
                "fp-refresh-fail",
                1_748_260_010_000,
                "network timeout",
            )
            .await?
            .expect("refresh failure should return updated row");

        assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            updated.last_refresh_error.as_deref(),
            Some("network timeout")
        );
        assert_eq!(updated.refreshed_at_ms, Some(1_748_260_010_000));
        assert_eq!(updated.oauth_grant_fingerprint, "fp-refresh-fail");

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_candidate_selection_scopes_filters_and_orders() {
    run_live_postgres_test("token_refresh_candidate_selection", |pool| async move {
        let due_before_ms = 1_748_300_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);

        seed_user(&pool, "tenant_tg_candidates", "user_tg_candidates").await?;
        seed_identity(&pool, "tenant_tg_candidates", "identity_tg_candidates").await?;
        seed_user(
            &pool,
            "tenant_tg_candidates_other",
            "user_tg_candidates_other",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tg_candidates_other",
            "identity_tg_candidates_other",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());

        let mut due_valid = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_valid",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-due-valid",
        );
        due_valid.expires_at_ms = Some(due_before_ms - 1_000);
        due_valid.last_refresh_error = None;
        repository.upsert_encrypted_grant(&due_valid).await?;

        let mut due_needs_refresh = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_needs_refresh",
            "identity_tg_candidates",
            TokenGrantState::NeedsRefresh,
            "fp-due-needs",
        );
        due_needs_refresh.expires_at_ms = Some(due_before_ms + 500_000);
        repository
            .upsert_encrypted_grant(&due_needs_refresh)
            .await?;

        let mut due_expired = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_expired",
            "identity_tg_candidates",
            TokenGrantState::Expired,
            "fp-due-expired",
        );
        due_expired.expires_at_ms = Some(due_before_ms + 100_000);
        repository.upsert_encrypted_grant(&due_expired).await?;

        let mut future_valid = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_future_valid",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-future-valid",
        );
        future_valid.expires_at_ms = Some(due_before_ms + 86_400_000);
        repository.upsert_encrypted_grant(&future_valid).await?;

        let revoked = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_revoked",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-revoked-candidate",
        );
        repository.upsert_encrypted_grant(&revoked).await?;
        repository
            .revoke(
                "tenant_tg_candidates",
                "grant_revoked",
                due_before_ms - 500,
                "manual revoke",
            )
            .await?
            .expect("revoke should update row");

        let reauth_required = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_reauth_required",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-reauth-candidate",
        );
        repository.upsert_encrypted_grant(&reauth_required).await?;
        repository
            .mark_reauth_required(
                "tenant_tg_candidates",
                "grant_reauth_required",
                "fp-reauth-candidate",
                due_before_ms - 250,
                "invalid_grant",
            )
            .await?
            .expect("mark reauth required should update row");

        let mut empty_encrypted = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_empty_encrypted",
            "identity_tg_candidates",
            TokenGrantState::NeedsRefresh,
            "fp-empty-encrypted",
        );
        empty_encrypted.encrypted_oauth_grant = Vec::new();
        repository.upsert_encrypted_grant(&empty_encrypted).await?;

        let mut other_tenant_due = encrypted_token_grant_record(
            "tenant_tg_candidates_other",
            "grant_other_tenant_due",
            "identity_tg_candidates_other",
            TokenGrantState::NeedsRefresh,
            "fp-other-tenant",
        );
        other_tenant_due.expires_at_ms = Some(due_before_ms - 2_000);
        repository.upsert_encrypted_grant(&other_tenant_due).await?;

        let candidates = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 32)
            .await?;

        let ids: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.grant_id.0.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "grant_due_expired",
                "grant_due_needs_refresh",
                "grant_due_valid"
            ]
        );

        for snapshot in &candidates {
            assert_eq!(snapshot.tenant_id.0, "tenant_tg_candidates");
            assert!(snapshot.expected_fingerprint.starts_with("fp-"));
            assert!(snapshot.has_refresh_material);
            assert_eq!(snapshot.revoked_at, None);
            assert_eq!(snapshot.reauth_required_at, None);
        }
        assert_eq!(candidates[0].state, TokenGrantState::Expired);
        assert_eq!(candidates[1].state, TokenGrantState::NeedsRefresh);
        assert_eq!(candidates[2].state, TokenGrantState::Valid);

        let limited = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 2)
            .await?;
        let limited_ids: Vec<&str> = limited
            .iter()
            .map(|candidate| candidate.grant_id.0.as_str())
            .collect();
        assert_eq!(
            limited_ids,
            vec!["grant_due_expired", "grant_due_needs_refresh"]
        );

        let none = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 0)
            .await?;
        assert!(none.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_sweep_run_once_rotates_candidates_with_sequenced_audit() {
    run_live_postgres_test("token_refresh_sweep_run_once", |pool| async move {
        let due_before_ms = 1_748_550_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);
        let now = UNIX_EPOCH + std::time::Duration::from_millis(1_748_550_500_000);

        seed_user(&pool, "tenant_tr_sweep_success", "user_tr_sweep_success").await?;
        seed_identity(
            &pool,
            "tenant_tr_sweep_success",
            "identity_tr_sweep_success",
        )
        .await?;
        seed_user(&pool, "tenant_tr_sweep_other", "user_tr_sweep_other").await?;
        seed_identity(&pool, "tenant_tr_sweep_other", "identity_tr_sweep_other").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut expired = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_expired",
            "identity_tr_sweep_success",
            TokenGrantState::Expired,
            "fp-sweep-expired-old",
        );
        expired.expires_at_ms = Some(due_before_ms + 100_000);
        grant_repo.upsert_encrypted_grant(&expired).await?;

        let mut due_valid = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_due_valid",
            "identity_tr_sweep_success",
            TokenGrantState::Valid,
            "fp-sweep-valid-old",
        );
        due_valid.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due_valid).await?;

        let mut future_valid = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_future_valid",
            "identity_tr_sweep_success",
            TokenGrantState::Valid,
            "fp-sweep-future-old",
        );
        future_valid.expires_at_ms = Some(due_before_ms + 86_400_000);
        grant_repo.upsert_encrypted_grant(&future_valid).await?;

        let mut other_tenant_due = encrypted_token_grant_record(
            "tenant_tr_sweep_other",
            "grant_sweep_other_tenant",
            "identity_tr_sweep_other",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-other-old",
        );
        other_tenant_due.expires_at_ms = Some(due_before_ms - 2_000);
        grant_repo.upsert_encrypted_grant(&other_tenant_due).await?;

        let adapter = SequenceRefreshAdapter::new([
            RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xA1],
                    encrypted_renewal: vec![0xB1],
                },
                key_id: "key-sweep-v2-a".to_string(),
                new_fingerprint: "fp-sweep-expired-new".to_string(),
                refreshed_at: now,
                expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_650_000_000)),
            },
            RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xA2],
                    encrypted_renewal: vec![0xB2],
                },
                key_id: "key-sweep-v2-b".to_string(),
                new_fingerprint: "fp-sweep-valid-new".to_string(),
                refreshed_at: now,
                expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_660_000_000)),
            },
        ]);
        let mut sweep = PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone());

        let report = sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: "tenant_tr_sweep_success".to_string(),
                due_before,
                limit: 8,
                now,
                audit_trace_id: "trace_token_refresh_sweep_success".to_string(),
                audit_sequence_start: 51,
                occurred_at_ms: 1_748_550_500_111,
                actor: actor("user_tr_sweep_success"),
                workspace_id: Some("workspace_tr_sweep_success".to_string()),
            })
            .await?;

        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.attempted_count, 2);
        assert_eq!(report.reports.len(), 2);
        assert_eq!(
            adapter.called_grant_ids(),
            vec!["grant_sweep_expired", "grant_sweep_due_valid"]
        );
        assert!(report.reports.iter().all(|item| {
            item.service_report.status == TokenRefreshReportStatus::Succeeded
                && item.service_report.adapter_called
                && item.service_report.sink_called
                && item.event.trace_id == "trace_token_refresh_sweep_success"
        }));
        assert_eq!(report.reports[0].event.sequence, 51);
        assert_eq!(report.reports[1].event.sequence, 52);
        assert_eq!(
            report.reports[0].event.scope.workspace_id.as_deref(),
            Some("workspace_tr_sweep_success")
        );

        let expired_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_expired")
            .await?
            .expect("expired sweep grant should exist");
        assert_eq!(expired_stored.state, TokenGrantState::Valid);
        assert_eq!(
            expired_stored.oauth_grant_fingerprint,
            "fp-sweep-expired-new"
        );
        assert_eq!(expired_stored.oauth_grant_key_id, "key-sweep-v2-a");

        let valid_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_due_valid")
            .await?
            .expect("valid sweep grant should exist");
        assert_eq!(valid_stored.state, TokenGrantState::Valid);
        assert_eq!(valid_stored.oauth_grant_fingerprint, "fp-sweep-valid-new");
        assert_eq!(valid_stored.oauth_grant_key_id, "key-sweep-v2-b");

        let future_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_future_valid")
            .await?
            .expect("future sweep grant should exist");
        assert_eq!(future_stored.oauth_grant_fingerprint, "fp-sweep-future-old");

        let other_stored = grant_repo
            .get_by_id("tenant_tr_sweep_other", "grant_sweep_other_tenant")
            .await?
            .expect("other tenant sweep grant should exist");
        assert_eq!(other_stored.oauth_grant_fingerprint, "fp-sweep-other-old");

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_trace_id("trace_token_refresh_sweep_success")
            .await?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 51);
        assert_eq!(events[1].sequence, 52);
        assert_eq!(events[0].target.action_type, "token_refresh.rotate");
        assert_eq!(events[1].target.action_type, "token_refresh.rotate");

        let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT payload
            FROM audit_events
            WHERE trace_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind("trace_token_refresh_sweep_success")
        .fetch_all(&pool)
        .await?;
        for payload in payloads {
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);
            assert!(!payload_text.contains("fp-sweep"));
            assert!(!payload_text.contains("encrypted"));
            assert!(!payload_text.contains("fingerprint"));
        }

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_sweep_limit_zero_short_circuits_without_adapter_or_audit() {
    run_live_postgres_test("token_refresh_sweep_limit_zero", |pool| async move {
        let due_before_ms = 1_748_560_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);
        let now = UNIX_EPOCH + std::time::Duration::from_millis(1_748_560_500_000);

        seed_user(&pool, "tenant_tr_sweep_zero", "user_tr_sweep_zero").await?;
        seed_identity(&pool, "tenant_tr_sweep_zero", "identity_tr_sweep_zero").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut due = encrypted_token_grant_record(
            "tenant_tr_sweep_zero",
            "grant_sweep_zero_due",
            "identity_tr_sweep_zero",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-zero-old",
        );
        due.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due).await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![0xC1],
                encrypted_renewal: vec![0xD1],
            },
            key_id: "key-sweep-zero-unused".to_string(),
            new_fingerprint: "fp-sweep-zero-new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }]);
        let mut sweep = PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone());

        let report = sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: "tenant_tr_sweep_zero".to_string(),
                due_before,
                limit: 0,
                now,
                audit_trace_id: "trace_token_refresh_sweep_zero".to_string(),
                audit_sequence_start: 71,
                occurred_at_ms: 1_748_560_500_111,
                actor: actor("user_tr_sweep_zero"),
                workspace_id: None,
            })
            .await?;

        assert_eq!(report.candidate_count, 0);
        assert_eq!(report.attempted_count, 0);
        assert!(report.reports.is_empty());
        assert!(adapter.called_grant_ids().is_empty());

        let stored = grant_repo
            .get_by_id("tenant_tr_sweep_zero", "grant_sweep_zero_due")
            .await?
            .expect("limit zero grant should remain");
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-sweep-zero-old");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_trace_id("trace_token_refresh_sweep_zero")
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_rotate() {
    run_live_postgres_test("token_grant_apply_refresh_rotate", |pool| async move {
        seed_user(
            &pool,
            "tenant_tg_apply_refresh_rotate",
            "user_tg_apply_refresh_rotate",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tg_apply_refresh_rotate",
            "identity_tg_apply_refresh_rotate",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_apply_refresh_rotate",
            "grant_tg_apply_refresh_rotate",
            "identity_tg_apply_refresh_rotate",
            TokenGrantState::NeedsRefresh,
            "fp-apply-old",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let updated = repository
            .apply_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                grant_id: TokenGrantId("grant_tg_apply_refresh_rotate".to_string()),
                tenant_id: TenantId("tenant_tg_apply_refresh_rotate".to_string()),
                expected_fingerprint: "fp-apply-old".to_string(),
                expires_at_ms: Some(1_748_280_000_000),
                refreshed_at_ms: 1_748_270_500_000,
                encrypted_grant_blob: EncryptedGrantBlob(vec![0xD0, 0xD1, 0xD2]),
                grant_key_id: "key-v2".to_string(),
                new_fingerprint: "fp-apply-new".to_string(),
            })
            .await?
            .expect("apply refresh rotate should return updated row");

        assert_eq!(updated.state, TokenGrantState::Valid);
        assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-new");
        assert_eq!(updated.oauth_grant_key_id, "key-v2");
        assert_eq!(updated.encrypted_oauth_grant, vec![0xD0, 0xD1, 0xD2]);
        assert_eq!(updated.expires_at_ms, Some(1_748_280_000_000));
        assert_eq!(updated.refreshed_at_ms, Some(1_748_270_500_000));
        assert_eq!(updated.last_refresh_error, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_mark_needs_refresh() {
    run_live_postgres_test(
        "token_grant_apply_refresh_needs_refresh",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tg_apply_refresh_needs",
                "user_tg_apply_refresh_needs",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tg_apply_refresh_needs",
                "identity_tg_apply_refresh_needs",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tg_apply_refresh_needs",
                "grant_tg_apply_refresh_needs",
                "identity_tg_apply_refresh_needs",
                TokenGrantState::Valid,
                "fp-apply-needs",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let updated = repository
                .apply_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                    grant_id: TokenGrantId("grant_tg_apply_refresh_needs".to_string()),
                    tenant_id: TenantId("tenant_tg_apply_refresh_needs".to_string()),
                    expected_fingerprint: "fp-apply-needs".to_string(),
                    refreshed_at_ms: 1_748_270_700_000,
                    safe_error: "refresh_token=rt_fake Authorization: Bearer at_fake".to_string(),
                })
                .await?
                .expect("apply refresh mark needs refresh should return updated row");

            assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                updated.last_refresh_error.as_deref(),
                Some("<redacted refresh error>")
            );
            assert_eq!(updated.refreshed_at_ms, Some(1_748_270_700_000));
            assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-needs");

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_mark_reauth_required() {
    run_live_postgres_test(
        "token_grant_apply_refresh_reauth_required",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tg_apply_refresh_reauth",
                "user_tg_apply_refresh_reauth",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tg_apply_refresh_reauth",
                "identity_tg_apply_refresh_reauth",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tg_apply_refresh_reauth",
                "grant_tg_apply_refresh_reauth",
                "identity_tg_apply_refresh_reauth",
                TokenGrantState::Valid,
                "fp-apply-reauth",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let updated = repository
                .apply_refresh_command(TokenRefreshRepositoryCommand::MarkReauthRequired {
                    grant_id: TokenGrantId("grant_tg_apply_refresh_reauth".to_string()),
                    tenant_id: TenantId("tenant_tg_apply_refresh_reauth".to_string()),
                    expected_fingerprint: "fp-apply-reauth".to_string(),
                    reauth_required_at_ms: 1_748_270_900_000,
                    safe_error: "invalid_grant".to_string(),
                })
                .await?
                .expect("apply refresh mark reauth required should return updated row");

            assert_eq!(updated.state, TokenGrantState::ReauthRequired);
            assert_eq!(updated.last_refresh_error.as_deref(), Some("invalid_grant"));
            assert_eq!(updated.reauth_required_at_ms, Some(1_748_270_900_000));
            assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-reauth");

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_replaces_sync_sink_successfully() {
    run_live_postgres_test(
        "token_refresh_orchestrator_no_sync_sink_success",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_service_success",
                "user_tr_service_success",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_service_success",
                "identity_tr_service_success",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tr_service_success",
                "grant_tr_service_success",
                "identity_tr_service_success",
                TokenGrantState::NeedsRefresh,
                "fp-service-old",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let refreshed_at =
                SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_302_000_000);
            let adapter = LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-v3".to_string(),
                new_fingerprint: "fp-service-new".to_string(),
                refreshed_at,
                expires_at: Some(
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_402_000_000),
                ),
            });
            let mut orchestrator =
                PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter.clone());

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_service_success".to_string()),
                        tenant_id: TenantId("tenant_tr_service_success".to_string()),
                        expected_fingerprint: "fp-service-old".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    refreshed_at,
                    TokenRefreshAuditContext {
                        trace_id: "trace_tr_service_success".to_string(),
                        sequence: 1,
                        occurred_at_ms: 1_748_302_000_001,
                        actor: actor("user_tr_service_success"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert!(report.service_report.adapter_called);
            assert!(report.service_report.sink_called);
            assert_eq!(orchestrator.adapter().calls(), 1);
            let report_debug = format!("{:?}", report.service_report);
            let audit_debug = format!("{:?}", report.service_report.audit_summary());
            assert!(!report_debug.contains("9, 9, 9"));
            assert!(!report_debug.contains("8, 8, 8"));
            assert!(!audit_debug.contains("9, 9, 9"));
            assert!(!audit_debug.contains("8, 8, 8"));
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");

            let updated = repository
                .get_by_id("tenant_tr_service_success", "grant_tr_service_success")
                .await?
                .expect("token grant should exist after rotation");
            assert_eq!(updated.state, TokenGrantState::Valid);
            assert_eq!(updated.oauth_grant_fingerprint, "fp-service-new");
            assert_eq!(updated.oauth_grant_key_id, "key-v3");
            assert_eq!(
                updated.encrypted_oauth_grant,
                vec![0, 0, 0, 3, 9, 9, 9, 0, 0, 0, 3, 8, 8, 8]
            );

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_replaces_sync_sink_stale_fingerprint_noop() {
    run_live_postgres_test(
        "token_refresh_orchestrator_no_sync_sink_stale_fp",
        |pool| async move {
            seed_user(&pool, "tenant_tr_service_noop", "user_tr_service_noop").await?;
            seed_identity(&pool, "tenant_tr_service_noop", "identity_tr_service_noop").await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tr_service_noop",
                "grant_tr_service_noop",
                "identity_tr_service_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_303_000_000);
            let adapter = LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-v4".to_string(),
                new_fingerprint: "fp-noop-new".to_string(),
                refreshed_at: now,
                expires_at: None,
            });
            let mut orchestrator =
                PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter.clone());

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_service_noop".to_string()),
                        tenant_id: TenantId("tenant_tr_service_noop".to_string()),
                        expected_fingerprint: "fp-stale".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_tr_service_noop".to_string(),
                        sequence: 1,
                        occurred_at_ms: 1_748_303_000_001,
                        actor: actor("user_tr_service_noop"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::ConflictNoop
            );
            assert!(report.service_report.adapter_called);
            assert!(report.service_report.sink_called);
            assert_eq!(orchestrator.adapter().calls(), 1);
            assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
            let report_debug = format!("{:?}", report.service_report);
            assert!(!report_debug.contains("9, 9, 9"));
            assert!(!report_debug.contains("8, 8, 8"));

            let stored = repository
                .get_by_id("tenant_tr_service_noop", "grant_tr_service_noop")
                .await?
                .expect("token grant should remain after stale fingerprint noop");
            assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
            assert_eq!(stored.oauth_grant_fingerprint, "fp-current");
            assert_eq!(stored.oauth_grant_key_id, "key-v1");
            assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_rotate_success() {
    run_live_postgres_test("token_refresh_orchestrator_success", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_success", "user_tr_orch_success").await?;
        seed_identity(&pool, "tenant_tr_orch_success", "identity_tr_orch_success").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_success",
                "grant_tr_orch_success",
                "identity_tr_orch_success",
                TokenGrantState::NeedsRefresh,
                "fp-orch-old",
            ))
            .await?;

        let refreshed_at =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_510_000_000);
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-orch-v2".to_string(),
                new_fingerprint: "fp-orch-new".to_string(),
                refreshed_at,
                expires_at: None,
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_success".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_success".to_string()),
                    expected_fingerprint: "fp-orch-old".to_string(),
                    state: TokenGrantState::NeedsRefresh,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                refreshed_at,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_success".to_string(),
                    sequence: 21,
                    occurred_at_ms: 1_748_510_000_111,
                    actor: actor("user_tr_orch_success"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::Succeeded
        );
        assert_eq!(orchestrator.adapter().calls(), 1);
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_success", "grant_tr_orch_success")
            .await?
            .expect("grant should exist");
        assert_eq!(stored.state, TokenGrantState::Valid);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-orch-new");
        assert_eq!(stored.oauth_grant_key_id, "key-orch-v2");

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_fixture_rotates_successfully() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_rotate",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_rotate",
                "user_tr_orch_lark_fixture_rotate",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_rotate",
                "identity_tr_orch_lark_fixture_rotate",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_rotate",
                    "grant_tr_orch_lark_fixture_rotate",
                    "identity_tr_orch_lark_fixture_rotate",
                    TokenGrantState::NeedsRefresh,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_ROTATED_ENCRYPTED_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_600_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_rotate".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_rotate".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_rotate".to_string(),
                        sequence: 31,
                        occurred_at_ms: 1_779_465_600_111,
                        actor: actor("user_tr_orch_lark_fixture_rotate"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_rotate",
                    "grant_tr_orch_lark_fixture_rotate",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::Valid);
            assert_eq!(stored.oauth_grant_fingerprint, "fp_rotated_v2");
            assert_eq!(stored.oauth_grant_key_id, "kms-key-2026-05");
            assert_ne!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_reauth_marks_reauth_required() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_reauth",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_reauth",
                "user_tr_orch_lark_fixture_reauth",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_reauth",
                "identity_tr_orch_lark_fixture_reauth",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_reauth",
                    "grant_tr_orch_lark_fixture_reauth",
                    "identity_tr_orch_lark_fixture_reauth",
                    TokenGrantState::Valid,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_REAUTH_REQUIRED_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_700_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_reauth".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_reauth".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::Valid,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_reauth".to_string(),
                        sequence: 32,
                        occurred_at_ms: 1_779_465_700_111,
                        actor: actor("user_tr_orch_lark_fixture_reauth"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(
                report.service_report.safe_error.as_deref(),
                Some("invalid_grant")
            );
            assert_eq!(
                report.event.target.action_type,
                "token_refresh.mark_reauth_required"
            );
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_reauth",
                    "grant_tr_orch_lark_fixture_reauth",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::ReauthRequired);
            assert_eq!(stored.last_refresh_error.as_deref(), Some("invalid_grant"));

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            assert_no_auth_refresh_sensitive_payload(&payload.to_string());

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_plaintext_fixture_is_safe_transient() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_plaintext",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_plaintext",
                "user_tr_orch_lark_fixture_plaintext",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_plaintext",
                "identity_tr_orch_lark_fixture_plaintext",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_plaintext",
                    "grant_tr_orch_lark_fixture_plaintext",
                    "identity_tr_orch_lark_fixture_plaintext",
                    TokenGrantState::Valid,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_800_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_plaintext".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_plaintext".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::Valid,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_plaintext".to_string(),
                        sequence: 33,
                        occurred_at_ms: 1_779_465_800_111,
                        actor: actor("user_tr_orch_lark_fixture_plaintext"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(
                report.service_report.safe_error.as_deref(),
                Some("temporarily unavailable")
            );
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_plaintext",
                    "grant_tr_orch_lark_fixture_plaintext",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                stored.last_refresh_error.as_deref(),
                Some("temporarily unavailable")
            );

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
                SELECT payload
                FROM audit_events
                WHERE event_id = $1
                "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);
            assert!(!payload_text.contains("tok_access_live_should_never_parse"));
            assert!(!payload_text.contains("tok_refresh_live_should_never_parse"));
            assert!(!payload_text.contains("refresh_token="));
            assert!(!payload_text.contains("access_token="));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_stale_conflict_noop() {
    run_live_postgres_test("token_refresh_orchestrator_stale", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_noop", "user_tr_orch_noop").await?;
        seed_identity(&pool, "tenant_tr_orch_noop", "identity_tr_orch_noop").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_noop",
                "grant_tr_orch_noop",
                "identity_tr_orch_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            ))
            .await?;

        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_520_000_000);
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-orch-v2".to_string(),
                new_fingerprint: "fp-orch-noop-new".to_string(),
                refreshed_at: now,
                expires_at: None,
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_noop".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_noop".to_string()),
                    expected_fingerprint: "fp-stale".to_string(),
                    state: TokenGrantState::NeedsRefresh,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                now,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_noop".to_string(),
                    sequence: 22,
                    occurred_at_ms: 1_748_520_000_111,
                    actor: actor("user_tr_orch_noop"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::ConflictNoop
        );
        assert_eq!(orchestrator.adapter().calls(), 1);
        assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report
                .event
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_noop", "grant_tr_orch_noop")
            .await?
            .expect("grant should remain");
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-current");

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_transient_failure_redacts() {
    run_live_postgres_test("token_refresh_orchestrator_redacts", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_redact", "user_tr_orch_redact").await?;
        seed_identity(&pool, "tenant_tr_orch_redact", "identity_tr_orch_redact").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_redact",
                "grant_tr_orch_redact",
                "identity_tr_orch_redact",
                TokenGrantState::Valid,
                "fp-orch-redact",
            ))
            .await?;

        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_530_000_000);
        let raw = "refresh_token=rt_fake Authorization: Bearer at_fake";
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::TransientFailure {
                safe_error: raw.to_string(),
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_redact".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_redact".to_string()),
                    expected_fingerprint: "fp-orch-redact".to_string(),
                    state: TokenGrantState::Valid,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                now,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_redact".to_string(),
                    sequence: 23,
                    occurred_at_ms: 1_748_530_000_111,
                    actor: actor("user_tr_orch_redact"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::Succeeded
        );
        assert_eq!(
            report.service_report.safe_error.as_deref(),
            Some("<redacted refresh error>")
        );
        assert_eq!(orchestrator.adapter().calls(), 1);

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_redact", "grant_tr_orch_redact")
            .await?
            .expect("grant should remain");
        assert_eq!(
            stored.last_refresh_error.as_deref(),
            Some("<redacted refresh error>")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string();
        assert!(!payload_text.contains("refresh_token=rt_fake"));
        assert!(!payload_text.contains("Bearer at_fake"));
        assert!(!payload_text.contains(raw));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_short_circuit_revoked() {
    run_live_postgres_test(
        "token_refresh_orchestrator_short_circuit",
        |pool| async move {
            seed_user(&pool, "tenant_tr_orch_short", "user_tr_orch_short").await?;
            seed_identity(&pool, "tenant_tr_orch_short", "identity_tr_orch_short").await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_short",
                    "grant_tr_orch_short",
                    "identity_tr_orch_short",
                    TokenGrantState::Valid,
                    "fp-short",
                ))
                .await?;

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_540_000_000);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LiveRefreshAdapter::new(RefreshOutcome::Success {
                    rotated_material: EncryptedGrantMaterial {
                        encrypted_primary: vec![9, 9, 9],
                        encrypted_renewal: vec![8, 8, 8],
                    },
                    key_id: "key-never-used".to_string(),
                    new_fingerprint: "fp-never-used".to_string(),
                    refreshed_at: now,
                    expires_at: None,
                }),
            );

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_short".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_short".to_string()),
                        expected_fingerprint: "fp-short".to_string(),
                        state: TokenGrantState::Revoked,
                        has_refresh_material: true,
                        revoked_at: Some(now),
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_short".to_string(),
                        sequence: 24,
                        occurred_at_ms: 1_748_540_000_111,
                        actor: actor("user_tr_orch_short"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::ShortCircuited(
                    oar_core::domain::token_refresh::TokenRefreshShortCircuitReason::Revoked
                )
            );
            assert_eq!(orchestrator.adapter().calls(), 0);
            assert_eq!(report.event.event_type, AuditEventType::ExecutionDenied);

            let stored = grant_repo
                .get_by_id("tenant_tr_orch_short", "grant_tr_orch_short")
                .await?
                .expect("grant should remain");
            assert_eq!(stored.oauth_grant_fingerprint, "fp-short");
            assert_eq!(stored.state, TokenGrantState::Valid);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_uow_rotate_success() {
    run_live_postgres_test("token_refresh_uow_rotate_success", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_success", "user_tr_uow_success").await?;
        seed_identity(&pool, "tenant_tr_uow_success", "identity_tr_uow_success").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_success",
                "grant_tr_uow_success",
                "identity_tr_uow_success",
                TokenGrantState::NeedsRefresh,
                "fp-uow-old",
            ))
            .await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let report = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_success".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_success".to_string()),
                    expected_fingerprint: "fp-uow-old".to_string(),
                    expires_at_ms: Some(1_748_480_000_000),
                    refreshed_at_ms: 1_748_470_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x11, 0x22, 0x33]),
                    grant_key_id: "key-uow-v2".to_string(),
                    new_fingerprint: "fp-uow-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_success".to_string(),
                    sequence: 11,
                    occurred_at_ms: 1_748_470_000_001,
                    actor: actor("user_tr_uow_success"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report
                .apply_result
                .expect("rotate should apply")
                .fingerprint,
            "fp-uow-new"
        );
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_success", "grant_tr_uow_success")
            .await?
            .expect("grant should exist");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-uow-new");
        assert_eq!(stored.state, TokenGrantState::Valid);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_trace_id("trace_token_refresh_uow_success")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target.action_type, "token_refresh.rotate");

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
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
fn postgres_live_token_refresh_uow_stale_fingerprint_conflict_noop() {
    run_live_postgres_test("token_refresh_uow_stale_fingerprint", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_noop", "user_tr_uow_noop").await?;
        seed_identity(&pool, "tenant_tr_uow_noop", "identity_tr_uow_noop").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_noop",
                "grant_tr_uow_noop",
                "identity_tr_uow_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            ))
            .await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let report = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_noop".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_noop".to_string()),
                    expected_fingerprint: "fp-stale".to_string(),
                    expires_at_ms: Some(1_748_490_000_000),
                    refreshed_at_ms: 1_748_480_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![9, 9, 9]),
                    grant_key_id: "key-uow-v2".to_string(),
                    new_fingerprint: "fp-noop-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_noop".to_string(),
                    sequence: 12,
                    occurred_at_ms: 1_748_480_000_001,
                    actor: actor("user_tr_uow_noop"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(report.apply_result, None);
        assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report
                .event
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_noop", "grant_tr_uow_noop")
            .await?
            .expect("grant should remain");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-current");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");
        assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_trace_id("trace_token_refresh_uow_noop")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            events[0]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
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
fn postgres_live_token_refresh_uow_mark_needs_refresh_redacts_audit_error() {
    run_live_postgres_test("token_refresh_uow_mark_needs_redacts", |pool| async move {
        seed_user(
            &pool,
            "tenant_tr_uow_needs_redact",
            "user_tr_uow_needs_redact",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_uow_needs_redact",
            "identity_tr_uow_needs_redact",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_needs_redact",
                "grant_tr_uow_needs_redact",
                "identity_tr_uow_needs_redact",
                TokenGrantState::Valid,
                "fp-uow-needs-redact",
            ))
            .await?;

        let report = PostgresTokenRefreshUnitOfWork::new(pool.clone())
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                    grant_id: TokenGrantId("grant_tr_uow_needs_redact".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_needs_redact".to_string()),
                    expected_fingerprint: "fp-uow-needs-redact".to_string(),
                    refreshed_at_ms: 1_748_485_000_000,
                    safe_error: "refresh_token=rt_fake Authorization: Bearer at_fake".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_needs_redact".to_string(),
                    sequence: 13,
                    occurred_at_ms: 1_748_485_000_001,
                    actor: actor("user_tr_uow_needs_redact"),
                    workspace_id: None,
                },
            )
            .await?;

        let updated = grant_repo
            .get_by_id("tenant_tr_uow_needs_redact", "grant_tr_uow_needs_redact")
            .await?
            .expect("grant should exist after needs-refresh mark");
        assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            updated.last_refresh_error.as_deref(),
            Some("<redacted refresh error>")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT payload
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string().to_lowercase();
        assert!(payload_text.contains("<redacted refresh error>"));
        assert!(!payload_text.contains("refresh_token"));
        assert!(!payload_text.contains("authorization"));
        assert!(!payload_text.contains("bearer"));
        assert!(!payload_text.contains("rt_fake"));
        assert!(!payload_text.contains("at_fake"));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_rejects_mismatched_plan_without_mutation() {
    run_live_postgres_test("token_refresh_uow_plan_mismatch", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_mismatch", "user_tr_uow_mismatch").await?;
        seed_identity(&pool, "tenant_tr_uow_mismatch", "identity_tr_uow_mismatch").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_mismatch",
                "grant_tr_uow_mismatch",
                "identity_tr_uow_mismatch",
                TokenGrantState::Valid,
                "fp-uow-mismatch",
            ))
            .await?;

        let mut planned =
            planned_token_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id: TokenGrantId("grant_tr_uow_mismatch".to_string()),
                tenant_id: TenantId("tenant_tr_uow_mismatch".to_string()),
                expected_fingerprint: "fp-uow-mismatch".to_string(),
                refreshed_at_ms: 1_748_486_000_000,
                safe_error: "temporarily unavailable".to_string(),
            });
        planned.report.tenant_id = TenantId("tenant_tr_uow_other".to_string());

        let result = PostgresTokenRefreshUnitOfWork::new(pool.clone())
            .apply_planned_command_with_audit(
                planned,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_mismatch".to_string(),
                    sequence: 14,
                    occurred_at_ms: 1_748_486_000_001,
                    actor: actor("user_tr_uow_mismatch"),
                    workspace_id: None,
                },
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
                field: "tenant_id",
                ..
            })
        ));

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_mismatch", "grant_tr_uow_mismatch")
            .await?
            .expect("grant should remain after rejected plan");
        assert_eq!(stored.state, TokenGrantState::Valid);
        assert_eq!(stored.last_refresh_error.as_deref(), Some("old-error"));

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_trace_id("trace_token_refresh_uow_mismatch")
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("token_refresh_uow_rollback", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_rollback", "user_tr_uow_rollback").await?;
        seed_identity(&pool, "tenant_tr_uow_rollback", "identity_tr_uow_rollback").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_rollback",
                "grant_tr_uow_rollback",
                "identity_tr_uow_rollback",
                TokenGrantState::NeedsRefresh,
                "fp-uow-rollback-old",
            ))
            .await?;

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let duplicate_event = AuditEvent::execution_succeeded(
            AuditEventContext {
                event_id: "trace_token_refresh_uow_rollback-evt-100".to_string(),
                trace_id: "trace_token_refresh_uow_rollback".to_string(),
                sequence: 100,
                occurred_at_ms: 1_748_499_999_000,
                subject: AuditSubject {
                    actor: actor("user_tr_uow_rollback"),
                    scope: scope("tenant_tr_uow_rollback"),
                    target: AuditTarget {
                        resource_type: "token_grant".to_string(),
                        resource_id: "grant_tr_uow_rollback".to_string(),
                        action_type: "token_refresh.rotate".to_string(),
                    },
                },
            },
            None,
            Some(summary("duplicate guard")),
            "noop",
        );
        audit.append(&duplicate_event, None).await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let result = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_rollback".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_rollback".to_string()),
                    expected_fingerprint: "fp-uow-rollback-old".to_string(),
                    expires_at_ms: Some(1_748_500_000_000),
                    refreshed_at_ms: 1_748_490_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x44, 0x55, 0x66]),
                    grant_key_id: "key-uow-v3".to_string(),
                    new_fingerprint: "fp-uow-rollback-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_rollback".to_string(),
                    sequence: 100,
                    occurred_at_ms: 1_748_490_000_001,
                    actor: AuditActor {
                        kind: AuditActorKind::Service,
                        actor_id: "svc_token_refresher".to_string(),
                        display_name: Some("Token Refresher".to_string()),
                    },
                    workspace_id: None,
                },
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should roll back grant mutation"
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_rollback", "grant_tr_uow_rollback")
            .await?
            .expect("grant should still exist after rollback");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-uow-rollback-old");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");
        assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_upsert_lookup_and_tenant_scope() {
    run_live_postgres_test("device_session_upsert_lookup_scope", |pool| async move {
        seed_user(&pool, "tenant_ds_a", "user_ds_a").await?;
        seed_user(&pool, "tenant_ds_b", "user_ds_b").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_270_000_000);
        let session = device_session(
            "tenant_ds_a",
            "user_ds_a",
            "session_ds_01",
            "okr_evidence",
            7,
            now,
        );

        let stored = repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-01")
            .await?;
        assert_eq!(stored.tenant_id, "tenant_ds_a");
        assert_eq!(stored.id, "session_ds_01");
        assert_eq!(stored.sync_cursor_value, 7);
        assert_eq!(stored.session_identity_hash, "sha256:session-ds-01");
        assert_eq!(stored.state, SessionState::Active);

        let found = repository
            .get_by_id("tenant_ds_a", "session_ds_01")
            .await?
            .expect("session should be found in tenant A");
        assert_eq!(found.id, "session_ds_01");
        assert_eq!(found.tenant_id, "tenant_ds_a");

        let hidden_from_other_tenant = repository.get_by_id("tenant_ds_b", "session_ds_01").await?;
        assert_eq!(hidden_from_other_tenant, None);

        let conflicting_tenant_session = device_session(
            "tenant_ds_b",
            "user_ds_b",
            "session_ds_01",
            "okr_evidence",
            8,
            now + std::time::Duration::from_secs(10),
        );
        let cross_tenant_result = repository
            .upsert_with_identity_hash(&conflicting_tenant_session, "sha256:session-ds-02")
            .await;
        match cross_tenant_result {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_ds_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch, got {other:?}"),
        }

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_advance_cursor_cas_and_terminal_state_guards() {
    run_live_postgres_test("device_session_advance_cursor_cas", |pool| async move {
        seed_user(&pool, "tenant_ds_cas", "user_ds_cas").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_280_000_000);
        let session = device_session(
            "tenant_ds_cas",
            "user_ds_cas",
            "session_ds_cas",
            "okr_evidence",
            10,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-cas")
            .await?;

        let advanced = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                10,
                11,
                base + std::time::Duration::from_secs(10),
            )
            .await?;
        assert_eq!(
            advanced.expect("advance should succeed").sync_cursor_value,
            11
        );

        let stale = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                10,
                12,
                base + std::time::Duration::from_secs(20),
            )
            .await?;
        assert_eq!(stale, None);

        let non_monotonic = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                11,
                11,
                base + std::time::Duration::from_secs(25),
            )
            .await?;
        assert_eq!(non_monotonic, None);

        let revoked = repository
            .revoke(
                "tenant_ds_cas",
                "session_ds_cas",
                base + std::time::Duration::from_secs(30),
            )
            .await?;
        assert_eq!(
            revoked.expect("revoke should apply").state,
            SessionState::Revoked
        );

        let blocked_after_revoke = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                11,
                12,
                base + std::time::Duration::from_secs(40),
            )
            .await?;
        assert_eq!(blocked_after_revoke, None);

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_expire_blocks_cursor_and_revoke_is_idempotent() {
    run_live_postgres_test("device_session_expire_and_revoke", |pool| async move {
        seed_user(&pool, "tenant_ds_exp", "user_ds_exp").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_290_000_000);
        let session = device_session(
            "tenant_ds_exp",
            "user_ds_exp",
            "session_ds_exp",
            "okr_evidence",
            20,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-exp")
            .await?;

        let expired = repository
            .expire(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(10),
            )
            .await?;
        assert_eq!(
            expired.expect("expire should apply").state,
            SessionState::Expired
        );

        let blocked_after_expire = repository
            .advance_cursor_cas(
                "tenant_ds_exp",
                "session_ds_exp",
                20,
                21,
                base + std::time::Duration::from_secs(20),
            )
            .await?;
        assert_eq!(blocked_after_expire, None);

        let first_revoke = repository
            .revoke(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(30),
            )
            .await?;
        assert_eq!(
            first_revoke
                .expect("revoke after expire should apply")
                .state,
            SessionState::Revoked
        );

        let second_revoke = repository
            .revoke(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(40),
            )
            .await?;
        assert_eq!(second_revoke, None);

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_upsert_after_revoke_preserves_terminal_state() {
    run_live_postgres_test("device_session_upsert_terminal_guard", |pool| async move {
        seed_user(&pool, "tenant_ds_term", "user_ds_term").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_300_000_000);
        let session = device_session(
            "tenant_ds_term",
            "user_ds_term",
            "session_ds_term",
            "okr_evidence",
            30,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-term-01")
            .await?;

        let revoked = repository
            .revoke(
                "tenant_ds_term",
                "session_ds_term",
                base + std::time::Duration::from_secs(10),
            )
            .await?
            .expect("revoke should apply");
        let revoked_at = revoked.revoked_at.expect("revoked timestamp should be set");
        assert_eq!(revoked.state, SessionState::Revoked);

        let mut attempted_reactivation = session.clone();
        attempted_reactivation.cursor.value = 99;
        attempted_reactivation.last_seen_at = base + std::time::Duration::from_secs(30);
        attempted_reactivation.state = SessionState::Active;
        attempted_reactivation.revoked_at = None;

        let stored = repository
            .upsert_with_identity_hash(&attempted_reactivation, "sha256:session-ds-term-02")
            .await?;

        assert_eq!(stored.state, SessionState::Revoked);
        assert_eq!(stored.revoked_at, Some(revoked_at));
        assert_eq!(stored.sync_cursor_value, revoked.sync_cursor_value);
        assert_eq!(stored.session_identity_hash, "sha256:session-ds-term-01");

        Ok(())
    });
}

#[test]
fn postgres_live_identity_repositories_upsert_lookup_and_tenant_conflict_guards() {
    run_live_postgres_test("identity_repo_upsert_lookup_conflict", |pool| async move {
        let tenant_repo = PostgresTenantRepository::new(pool.clone());
        let user_repo = PostgresOarUserRepository::new(pool.clone());
        let identity_repo = PostgresLarkIdentityRepository::new(pool.clone());

        let tenant_a = Tenant {
            id: TenantId("tenant_identity_a".to_string()),
            display_name: "Tenant A".to_string(),
            status: TenantStatus::Active,
        };
        let tenant_b = Tenant {
            id: TenantId("tenant_identity_b".to_string()),
            display_name: "Tenant B".to_string(),
            status: TenantStatus::Suspended,
        };

        let stored_tenant_a = tenant_repo.upsert(&tenant_a).await?;
        let stored_tenant_b = tenant_repo.upsert(&tenant_b).await?;
        assert_eq!(stored_tenant_a.status, TenantStatus::Active);
        assert_eq!(stored_tenant_b.status, TenantStatus::Suspended);

        let fetched_tenant = tenant_repo
            .get_by_id("tenant_identity_b")
            .await?
            .expect("tenant should exist");
        assert_eq!(fetched_tenant.display_name, "Tenant B");
        assert_eq!(fetched_tenant.status, TenantStatus::Suspended);

        let user_a = OarUser {
            id: OarUserId("user_identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            display_name: "User Shared A".to_string(),
            status: OarUserStatus::Active,
        };
        let stored_user_a = user_repo.upsert(&user_a).await?;
        assert_eq!(stored_user_a.tenant_id, "tenant_identity_a");
        assert_eq!(stored_user_a.status, OarUserStatus::Active);

        let fetched_user_a = user_repo
            .get_by_id("tenant_identity_a", "user_identity_shared")
            .await?
            .expect("user should exist for tenant A");
        assert_eq!(fetched_user_a.display_name, "User Shared A");
        assert_eq!(
            user_repo
                .get_by_id("tenant_identity_b", "user_identity_shared")
                .await?,
            None
        );

        let conflicting_user = OarUser {
            id: OarUserId("user_identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_b".to_string()),
            display_name: "User Shared B".to_string(),
            status: OarUserStatus::Disabled,
        };
        match user_repo.upsert(&conflicting_user).await {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_identity_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch for oar_users, got {other:?}"),
        }

        let identity_a = LarkIdentity {
            id: LarkIdentityId("identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            actor_kind: ActorKind::User,
            actor_external_id: "ext-shared-a".to_string(),
            display_name: Some("Identity Shared A".to_string()),
        };
        let stored_identity_a = identity_repo.upsert(&identity_a).await?;
        assert_eq!(stored_identity_a.tenant_id, "tenant_identity_a");
        assert_eq!(stored_identity_a.actor_kind, ActorKind::User);

        let fetched_identity_a = identity_repo
            .get_by_id("tenant_identity_a", "identity_shared")
            .await?
            .expect("identity should exist for tenant A");
        assert_eq!(fetched_identity_a.actor_external_id, "ext-shared-a");

        let fetched_by_external = identity_repo
            .get_by_actor_external_id("tenant_identity_a", ActorKind::User, "ext-shared-a")
            .await?
            .expect("identity should be discoverable by external actor id");
        assert_eq!(fetched_by_external.id, "identity_shared");
        assert_eq!(
            identity_repo
                .get_by_actor_external_id("tenant_identity_b", ActorKind::User, "ext-shared-a")
                .await?,
            None
        );

        let duplicate_external_binding = LarkIdentity {
            id: LarkIdentityId("identity_duplicate_external".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            actor_kind: ActorKind::User,
            actor_external_id: "ext-shared-a".to_string(),
            display_name: Some("Identity Duplicate External".to_string()),
        };
        match identity_repo.upsert(&duplicate_external_binding).await {
            Err(PostgresRepositoryError::LarkIdentityActorExternalBindingConflict {
                tenant_id,
                actor_kind,
                actor_external_id,
            }) => {
                assert_eq!(tenant_id, "tenant_identity_a");
                assert_eq!(actor_kind, ActorKind::User);
                assert_eq!(actor_external_id, "ext-shared-a");
            }
            other => panic!(
                "expected typed actor external binding conflict for lark_identities, got {other:?}"
            ),
        }

        let conflicting_identity = LarkIdentity {
            id: LarkIdentityId("identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_b".to_string()),
            actor_kind: ActorKind::Bot,
            actor_external_id: "ext-shared-b".to_string(),
            display_name: Some("Identity Shared B".to_string()),
        };
        match identity_repo.upsert(&conflicting_identity).await {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_identity_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch for lark_identities, got {other:?}"),
        }

        Ok(())
    });
}
