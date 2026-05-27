pub(super) const MIGRATION_0001_SQL: &str =
    include_str!("../../../../oar-core/migrations/0001_phase_0_6_identity_action_audit.sql");
pub(super) const MIGRATION_0002_SQL: &str =
    include_str!("../../../../oar-core/migrations/0002_review_inbox_domain.sql");

pub(super) const TENANT_ID: &str = "tenant_adapter_pg_refresh";
pub(super) const USER_ID: &str = "user_adapter_pg_refresh";
pub(super) const IDENTITY_ID: &str = "identity_adapter_pg_refresh";
pub(super) const GRANT_ID: &str = "grant_adapter_pg_refresh";
pub(super) const TRACE_ID: &str = "trace_adapter_pg_refresh";
pub(super) const ACTOR_ID: &str = "reviewer_adapter_pg_refresh";
pub(super) const KEY_ID: &str = "key-adapter-pg";
pub(super) const OLD_FP: &str = "fp-current-adapter-pg";
pub(super) const SEED_ACCESS_TOKEN: &str = "uat-seed-sensitive-access";
pub(super) const SEED_REFRESH_TOKEN: &str = "urt-seed-sensitive-refresh";
pub(super) const NEW_ACCESS_TOKEN: &str = "uat-new-sensitive-access";
pub(super) const NEW_REFRESH_TOKEN: &str = "urt-new-sensitive-refresh";
pub(super) const CLIENT_SECRET: &str = "secret-sensitive-client";
