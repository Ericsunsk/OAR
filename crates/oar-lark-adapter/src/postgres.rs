use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;
use sqlx::{PgPool, Row};

use crate::material::{AsyncFeishuGrantMaterialStore, StoredFeishuGrantMaterial};

#[derive(Clone)]
pub struct PostgresFeishuGrantMaterialStore {
    pool: PgPool,
}

impl PostgresFeishuGrantMaterialStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl fmt::Debug for PostgresFeishuGrantMaterialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresFeishuGrantMaterialStore")
            .field("pool", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PostgresFeishuGrantMaterialStoreError {
    #[error("grant material is unavailable")]
    Unavailable,
}

#[async_trait]
impl AsyncFeishuGrantMaterialStore for PostgresFeishuGrantMaterialStore {
    type Error = PostgresFeishuGrantMaterialStoreError;

    async fn load(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error> {
        let row = sqlx::query(
            r#"
                    SELECT
                        id,
                        tenant_id,
                        encrypted_oauth_grant,
                        oauth_grant_key_id,
                        oauth_grant_fingerprint,
                        NULLIF(array_to_string(scopes, ' '), '') AS scope
                    FROM token_grants
                    WHERE tenant_id = $1
                      AND id = $2
                    LIMIT 1
                    "#,
        )
        .bind(&request.tenant_id)
        .bind(&request.grant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?;

        let Some(row) = row else {
            return Err(PostgresFeishuGrantMaterialStoreError::Unavailable);
        };

        Ok(StoredFeishuGrantMaterial {
            grant_id: row
                .try_get("id")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
            tenant_id: row
                .try_get("tenant_id")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
            encrypted_oauth_grant: row
                .try_get("encrypted_oauth_grant")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
            oauth_grant_key_id: row
                .try_get("oauth_grant_key_id")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
            oauth_grant_fingerprint: row
                .try_get("oauth_grant_fingerprint")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
            scope: row
                .try_get::<Option<String>, _>("scope")
                .map_err(|_| PostgresFeishuGrantMaterialStoreError::Unavailable)?,
        })
    }
}
