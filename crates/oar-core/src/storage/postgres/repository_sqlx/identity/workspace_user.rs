use super::super::*;

impl PostgresWorkspaceUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, user: &WorkspaceUser) -> PgRepositoryResult<StoredWorkspaceUser> {
        let row = sqlx::query(UPSERT_WORKSPACE_USER)
            .bind(&user.id.0)
            .bind(&user.tenant_id.0)
            .bind(&user.display_name)
            .bind(workspace_user_status_to_db(&user.status))
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row.as_ref() {
            return stored_workspace_user_from_row(row);
        }

        tenant_mismatch_or_row_not_found(
            &self.pool,
            "SELECT 1 FROM workspace_users WHERE id = $1 LIMIT 1",
            &user.id.0,
            &user.tenant_id.0,
        )
        .await
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> PgRepositoryResult<Option<StoredWorkspaceUser>> {
        let row = sqlx::query(GET_WORKSPACE_USER_BY_ID)
            .bind(tenant_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_workspace_user_from_row).transpose()
    }
}
