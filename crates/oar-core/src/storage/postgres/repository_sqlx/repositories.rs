use crate::domain::token_refresh::service::AsyncAuthRefreshAdapter;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PostgresTokenGrantRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresAuthLifecycleRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshOrchestrator<A>
where
    A: AsyncAuthRefreshAdapter,
{
    pub(super) adapter: A,
    pub(super) recorder: PostgresTokenRefreshRecorder,
    pub(super) audit: PostgresAuditEventRepository,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshSweep<A>
where
    A: AsyncAuthRefreshAdapter,
{
    pub(super) candidates: PostgresTokenGrantRepository,
    pub(super) orchestrator: PostgresTokenRefreshOrchestrator<A>,
}

#[derive(Debug, Clone)]
pub struct PostgresDeviceSessionRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresTenantRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresWorkspaceUserRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresLarkIdentityRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresIdentityRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresReviewInboxRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresSchedulerJobRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresOperationLedgerRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresOperationalRecoveryRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresExecutionRecorder {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresReviewDecisionRecorder {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshRecorder {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresAuditEventRepository {
    pub(super) pool: PgPool,
}
