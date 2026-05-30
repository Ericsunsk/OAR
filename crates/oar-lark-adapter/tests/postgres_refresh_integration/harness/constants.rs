pub(crate) const MIGRATION_0001_SQL: &str =
    include_str!("../../../../oar-core/migrations/0001_phase_0_6_identity_action_audit.sql");
pub(crate) const MIGRATION_0002_SQL: &str =
    include_str!("../../../../oar-core/migrations/0002_review_inbox_domain.sql");

pub(crate) const TENANT_ID: &str = "tenant_adapter_pg_refresh";
pub(crate) const USER_ID: &str = "user_adapter_pg_refresh";
pub(crate) const IDENTITY_ID: &str = "identity_adapter_pg_refresh";
pub(crate) const GRANT_ID: &str = "grant_adapter_pg_refresh";
pub(crate) const TRACE_ID: &str = "trace_adapter_pg_refresh";
pub(crate) const ACTOR_ID: &str = "reviewer_adapter_pg_refresh";
pub(crate) const KEY_ID: &str = "key-adapter-pg";
pub(crate) const OLD_FP: &str = "fp-current-adapter-pg";
pub(crate) const SEED_ACCESS_TOKEN: &str = "uat-seed-sensitive-access";
pub(crate) const SEED_REFRESH_TOKEN: &str = "urt-seed-sensitive-refresh";
pub(crate) const NEW_ACCESS_TOKEN: &str = "uat-new-sensitive-access";
pub(crate) const NEW_REFRESH_TOKEN: &str = "urt-new-sensitive-refresh";
pub(crate) const CLIENT_SECRET: &str = "secret-sensitive-client";
