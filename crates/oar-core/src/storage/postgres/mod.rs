pub mod audit_sql;
pub mod device_session_sql;
pub mod identity_sql;
pub mod operation_ledger_sql;
pub mod token_grant_sql;

#[cfg(feature = "postgres")]
pub mod audit_outbox_worker;

#[cfg(feature = "postgres")]
mod repository_sqlx;

#[cfg(feature = "postgres")]
pub use repository_sqlx::{
    AuditOutboxEnvelope, AuditOutboxMessage, EncryptedTokenGrantRecord,
    PostgresAuditEventRepository, PostgresDeviceSessionRepository, PostgresExecutionUnitOfWork,
    PostgresExecutionUnitOfWorkReport, PostgresIdentityRepository, PostgresLarkIdentityRepository,
    PostgresOarUserRepository, PostgresOperationLedgerRepository, PostgresRepositoryError,
    PostgresTenantRepository, PostgresTokenGrantRepository, PostgresTokenRefreshCommandSink,
    PostgresTokenRefreshOrchestrator, PostgresTokenRefreshOrchestratorReport,
    PostgresTokenRefreshSweep, PostgresTokenRefreshSweepReport, PostgresTokenRefreshSweepRequest,
    PostgresTokenRefreshUnitOfWork, PostgresTokenRefreshUnitOfWorkReport, StoredDeviceSession,
    StoredLarkIdentity, StoredOarUser, StoredTenant,
};
