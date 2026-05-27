use super::super::*;

impl PostgresIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn tenants(&self) -> PostgresTenantRepository {
        PostgresTenantRepository::new(self.pool.clone())
    }

    pub fn users(&self) -> PostgresWorkspaceUserRepository {
        PostgresWorkspaceUserRepository::new(self.pool.clone())
    }

    pub fn identities(&self) -> PostgresLarkIdentityRepository {
        PostgresLarkIdentityRepository::new(self.pool.clone())
    }
}
