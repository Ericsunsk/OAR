use super::super::*;

impl PostgresTenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, tenant: &Tenant) -> PgRepositoryResult<StoredTenant> {
        let row = sqlx::query(UPSERT_TENANT)
            .bind(&tenant.id.0)
            .bind(&tenant.display_name)
            .bind(tenant_status_to_db(&tenant.status))
            .fetch_one(&self.pool)
            .await?;
        stored_tenant_from_row(&row)
    }

    pub async fn get_by_id(&self, tenant_id: &str) -> PgRepositoryResult<Option<StoredTenant>> {
        let row = sqlx::query(GET_TENANT_BY_ID)
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_tenant_from_row).transpose()
    }

    pub async fn list_active_ids(&self) -> PgRepositoryResult<Vec<String>> {
        let rows = sqlx::query(LIST_ACTIVE_TENANT_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.get::<String, _>("id"))
            .collect())
    }
}
