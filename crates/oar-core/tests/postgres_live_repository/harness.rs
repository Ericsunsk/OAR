#![cfg(feature = "postgres")]

pub(crate) use std::collections::VecDeque;
pub(crate) use std::env;
pub(crate) use std::future::Future;
pub(crate) use std::sync::atomic::{AtomicU64, Ordering};
pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};
pub(crate) use oar_core::action::capability::all_capabilities;
pub(crate) use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
pub(crate) use oar_core::action::execution_policy::{
    ActionActorBinding, ExecutionDenied, ExecutionPolicy,
};
pub(crate) use oar_core::action::executor::{
    ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution, ExecutionError,
};
pub(crate) use oar_core::action::operation_ledger::{LedgerError, SubmitResult};
pub(crate) use oar_core::action::postgres_executor::PostgresActionExecutor;
pub(crate) use oar_core::action::token_refresh_audit::{
    token_refresh_audit_event, TokenRefreshAuditContext,
};
pub(crate) use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession, SessionState};
pub(crate) use oar_core::domain::evidence::{
    EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind, EvidenceVisibilityScope,
};
pub(crate) use oar_core::domain::identity::{
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, OAuthTokens, ScopeBoundary,
    SecretString, Tenant, TenantId, TenantStatus, TokenGrant, TokenGrantId, TokenGrantState,
    WorkspaceUser, WorkspaceUserId, WorkspaceUserStatus,
};
pub(crate) use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionDecision, ProposedActionId, ProposedActionKind, RiskSeverity,
};
pub(crate) use oar_core::domain::review_inbox::{
    ReviewInboxItem, ReviewInboxItemId, ReviewInboxItemStatus,
};
pub(crate) use oar_core::domain::scheduler::{
    SchedulerJobKind, SchedulerJobOutcome, SchedulerJobStatus, SchedulerLeaseAcquire,
};
pub(crate) use oar_core::domain::token_refresh::service::{
    AsyncAuthRefreshAdapter, AuthRefreshAdapter,
};
pub(crate) use oar_core::domain::token_refresh::types::{
    EncryptedGrantBlob, EncryptedGrantMaterial, RefreshOutcome, TokenRefreshAuditSummary,
    TokenRefreshCommandKind, TokenRefreshCommandReport, TokenRefreshDecisionKind,
    TokenRefreshGrantSnapshot, TokenRefreshPlannedCommand, TokenRefreshReportStatus,
    TokenRefreshRepositoryCommand,
};
pub(crate) use oar_core::lark::auth::adapter::{
    AsyncFeishuAuthRefreshClient, FeishuAuthRefreshAdapter, FeishuAuthRefreshClient,
};
pub(crate) use oar_core::lark::auth::parser::parse_feishu_auth_refresh_response;
pub(crate) use oar_core::lark::auth::types::{FeishuAuthRefreshRequest, FeishuAuthRefreshResponse};
pub(crate) use oar_core::lark::fixtures::{
    AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON, AUTH_REFRESH_REAUTH_REQUIRED_JSON,
    AUTH_REFRESH_ROTATED_ENCRYPTED_JSON,
};
pub(crate) use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher, AuditOutboxDrainConfig, PostgresAuditOutboxWorker,
};
pub(crate) use oar_core::storage::postgres::tenant_maintenance::{
    PostgresTenantMaintenanceConfig, PostgresTenantMaintenanceWorker,
};
pub(crate) use oar_core::storage::postgres::{
    AuditOutboxEnvelope, AuditOutboxMessage, EncryptedTokenGrantRecord,
    InsertProposedActionDecisionRequest, PostgresAuditEventRepository,
    PostgresDeviceSessionRepository, PostgresExecutionRecorder, PostgresLarkIdentityRepository,
    PostgresOperationLedgerRepository, PostgresRepositoryError, PostgresReviewDecisionRecorder,
    PostgresReviewDecisionRecorderRequest, PostgresReviewInboxRepository,
    PostgresSchedulerJobRepository, PostgresTenantRepository, PostgresTokenGrantRepository,
    PostgresTokenRefreshOrchestrator, PostgresTokenRefreshRecorder,
    PostgresTokenRefreshScheduledSweep, PostgresTokenRefreshSweep,
    PostgresTokenRefreshSweepRequest, PostgresWorkspaceUserRepository, RotateEncryptedGrantRequest,
    TokenRefreshScheduledSweepConfig,
};
pub(crate) use serde_json::json;
pub(crate) use sqlx::postgres::PgPoolOptions;
pub(crate) use sqlx::{AssertSqlSafe, PgPool, Row};

const MIGRATION_0001_SQL: &str =
    include_str!("../../migrations/0001_phase_0_6_identity_action_audit.sql");
const MIGRATION_0002_SQL: &str = include_str!("../../migrations/0002_review_inbox_domain.sql");
const MIGRATION_0003_SQL: &str = include_str!("../../migrations/0003_agent_model_settings.sql");

static SCHEMA_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub(crate) struct LiveMockAdapter {
    state: Arc<Mutex<LiveMockAdapterState>>,
}

#[derive(Default)]
struct LiveMockAdapterState {
    dry_run_calls: usize,
    execute_calls: usize,
    execute_error: Option<AdapterError>,
}

impl LiveMockAdapter {
    pub(crate) fn succeeding() -> Self {
        Self::default()
    }

    pub(crate) fn failing(code: &str, message: &str) -> Self {
        let adapter = Self::default();
        adapter.state.lock().expect("adapter mutex").execute_error =
            Some(AdapterError::from_safe_message(code, message));
        adapter
    }

    pub(crate) fn dry_run_calls(&self) -> usize {
        self.state.lock().expect("adapter mutex").dry_run_calls
    }

    pub(crate) fn execute_calls(&self) -> usize {
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
pub(crate) struct LiveOutboxDispatcher {
    outcomes: Arc<Mutex<Vec<AuditOutboxDelivery>>>,
}

impl LiveOutboxDispatcher {
    pub(crate) fn new(outcomes: impl IntoIterator<Item = AuditOutboxDelivery>) -> Self {
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
pub(crate) struct LiveRefreshAdapter {
    outcome: RefreshOutcome,
    calls: Arc<Mutex<usize>>,
}

impl LiveRefreshAdapter {
    pub(crate) fn new(outcome: RefreshOutcome) -> Self {
        Self {
            outcome,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn calls(&self) -> usize {
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

#[async_trait::async_trait]
impl AsyncAuthRefreshAdapter for LiveRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        AuthRefreshAdapter::refresh(self, snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct SequenceRefreshAdapter {
    outcomes: Arc<Mutex<VecDeque<RefreshOutcome>>>,
    called_grant_ids: Arc<Mutex<Vec<String>>>,
}

impl SequenceRefreshAdapter {
    pub(crate) fn new(outcomes: impl IntoIterator<Item = RefreshOutcome>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
            called_grant_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn called_grant_ids(&self) -> Vec<String> {
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

#[async_trait::async_trait]
impl AsyncAuthRefreshAdapter for SequenceRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        AuthRefreshAdapter::refresh(self, snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct FixtureClient {
    fixture: &'static str,
    calls: Arc<Mutex<usize>>,
}

impl FixtureClient {
    pub(crate) fn new(fixture: &'static str) -> Self {
        Self {
            fixture,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn calls(&self) -> usize {
        *self.calls.lock().expect("fixture client mutex")
    }
}

impl FeishuAuthRefreshClient for FixtureClient {
    type Error = &'static str;

    fn refresh(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, Self::Error> {
        let mut calls = self.calls.lock().expect("fixture client mutex");
        *calls += 1;
        parse_feishu_auth_refresh_response(self.fixture).map_err(|_| "fixture_parse_failed")
    }
}

#[async_trait::async_trait]
impl AsyncFeishuAuthRefreshClient for FixtureClient {
    type Error = &'static str;

    async fn refresh(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, Self::Error> {
        FeishuAuthRefreshClient::refresh(self, request)
    }
}

pub(crate) fn assert_no_auth_refresh_sensitive_payload(payload_text: &str) {
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

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
}

pub(crate) fn confirmed_action(
    action_id: &str,
    tenant_id: &str,
    actor_user_id: &str,
    idempotency_key: &str,
) -> ConfirmedAction {
    ConfirmedAction::proposed(action_id, tenant_id, actor_user_id, idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

pub(crate) fn actor(actor_id: &str) -> AuditActor {
    AuditActor {
        kind: AuditActorKind::User,
        actor_id: actor_id.to_string(),
        display_name: Some("Reviewer".to_string()),
    }
}

pub(crate) fn scope(tenant_id: &str) -> AuditScope {
    AuditScope {
        tenant_id: tenant_id.to_string(),
        workspace_id: None,
    }
}

pub(crate) fn target(resource_id: &str) -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: resource_id.to_string(),
        action_type: "update_progress".to_string(),
    }
}

pub(crate) fn summary(text: &str) -> AuditStateSummary {
    AuditStateSummary {
        summary: text.to_string(),
        reference_ids: vec!["evidence_1".to_string()],
        content_hash: Some("sha256:abc123".to_string()),
    }
}

pub(crate) fn audit_context(
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

pub(crate) fn outbox_envelope(
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

pub(crate) fn postgres_action_executor(
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
        PostgresExecutionRecorder::new(pool.clone()),
        PostgresAuditEventRepository::new(pool),
    )
}

pub(crate) fn token_grant(tenant_id: &str, scopes: &[&str], state: TokenGrantState) -> TokenGrant {
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

pub(crate) fn actor_binding(actor_user_id: &str) -> ActionActorBinding {
    ActionActorBinding::new(actor_user_id, LarkIdentityId("identity_live".to_string()))
}

pub(crate) fn okr_progress_write_policy() -> ExecutionPolicy {
    ExecutionPolicy::from_capabilities(all_capabilities(), [ActorKind::User, ActorKind::Service])
}

pub(crate) fn unique_schema_name(test_name: &str) -> String {
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

pub(crate) async fn create_schema_and_pool(
    database_url: &str,
    schema: &str,
) -> Result<PgPool, sqlx::Error> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;

    sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(format!("SET search_path TO {schema}")))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0001_SQL.to_string()))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0002_SQL.to_string()))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0003_SQL.to_string()))
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

pub(crate) async fn drop_schema(database_url: &str, schema: &str) -> Result<(), sqlx::Error> {
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

pub(crate) fn run_live_postgres_test<F, Fut>(test_name: &str, test: F)
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

pub(crate) async fn seed_user(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
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
        INSERT INTO workspace_users (id, tenant_id, display_name, status)
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

pub(crate) async fn seed_identity(
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

pub(crate) fn encrypted_token_grant_record(
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

pub(crate) fn rotate_grant_request<'a>(
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

pub(crate) fn planned_token_refresh_command(
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
        }
        | TokenRefreshRepositoryCommand::MarkConfigRequired {
            grant_id,
            tenant_id,
            ..
        } => (grant_id.clone(), tenant_id.clone()),
    };
    let command_kind = command.kind();
    let safe_error = match &command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. }
        | TokenRefreshRepositoryCommand::MarkReauthRequired { safe_error, .. }
        | TokenRefreshRepositoryCommand::MarkConfigRequired { safe_error, .. } => {
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
                TokenRefreshCommandKind::MarkConfigRequired => {
                    TokenRefreshDecisionKind::MarkConfigRequired
                }
            },
            command_kind,
            safe_error,
        },
    }
}

pub(crate) fn device_session(
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
        WorkspaceUserId(user_id.to_string()),
        DeviceEntryPoint::MacOs,
        stream.to_string(),
        cursor,
        now,
    )
}

pub(crate) async fn audit_outbox_count(pool: &PgPool, tenant_id: &str) -> Result<i64, sqlx::Error> {
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
