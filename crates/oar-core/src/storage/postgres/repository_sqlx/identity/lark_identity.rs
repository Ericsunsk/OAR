use super::super::*;

impl PostgresLarkIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, identity: &LarkIdentity) -> PgRepositoryResult<StoredLarkIdentity> {
        let row = match sqlx::query(UPSERT_LARK_IDENTITY)
            .bind(&identity.id.0)
            .bind(&identity.tenant_id.0)
            .bind(actor_kind_to_db(&identity.actor_kind))
            .bind(&identity.actor_external_id)
            .bind(&identity.display_name)
            .fetch_optional(&self.pool)
            .await
        {
            Ok(row) => row,
            Err(error) if is_unique_violation(&error) => {
                let conflicting_row = sqlx::query(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL)
                    .bind(&identity.tenant_id.0)
                    .bind(actor_kind_to_db(&identity.actor_kind))
                    .bind(&identity.actor_external_id)
                    .fetch_optional(&self.pool)
                    .await?;

                if let Some(conflicting_row) = conflicting_row.as_ref() {
                    let conflicting = stored_lark_identity_from_row(conflicting_row)?;
                    if conflicting.id != identity.id.0 {
                        return Err(
                            PostgresRepositoryError::LarkIdentityActorExternalBindingConflict {
                                tenant_id: identity.tenant_id.0.clone(),
                                actor_kind: identity.actor_kind,
                                actor_external_id: identity.actor_external_id.clone(),
                            },
                        );
                    }
                }

                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(row) = row.as_ref() {
            return stored_lark_identity_from_row(row);
        }

        tenant_mismatch_or_row_not_found(
            &self.pool,
            "SELECT 1 FROM lark_identities WHERE id = $1 LIMIT 1",
            &identity.id.0,
            &identity.tenant_id.0,
        )
        .await
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        identity_id: &str,
    ) -> PgRepositoryResult<Option<StoredLarkIdentity>> {
        let row = sqlx::query(GET_LARK_IDENTITY_BY_ID)
            .bind(tenant_id)
            .bind(identity_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_lark_identity_from_row).transpose()
    }

    pub async fn get_by_actor_external_id(
        &self,
        tenant_id: &str,
        actor_kind: ActorKind,
        actor_external_id: &str,
    ) -> PgRepositoryResult<Option<StoredLarkIdentity>> {
        let row = sqlx::query(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL)
            .bind(tenant_id)
            .bind(actor_kind_to_db(&actor_kind))
            .bind(actor_external_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_lark_identity_from_row).transpose()
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error.as_database_error() {
        Some(db_error) => db_error
            .code()
            .map(|code| code.as_ref() == "23505")
            .unwrap_or(false),
        None => false,
    }
}
