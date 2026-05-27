use super::super::*;

impl PostgresTokenGrantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_encrypted_grant(
        &self,
        grant: &EncryptedTokenGrantRecord,
    ) -> PgRepositoryResult<EncryptedTokenGrantRecord> {
        let row = sqlx::query(UPSERT_TOKEN_GRANT)
            .bind(&grant.id)
            .bind(&grant.tenant_id)
            .bind(&grant.identity_id)
            .bind(actor_kind_to_db(&grant.actor_kind))
            .bind(scope_boundary_to_db(&grant.scope_boundary))
            .bind(&grant.scopes)
            .bind(token_grant_state_to_db(&grant.state))
            .bind(grant.issued_at_ms as i64)
            .bind(option_u64_to_i64(grant.expires_at_ms))
            .bind(option_u64_to_i64(grant.refreshed_at_ms))
            .bind(option_u64_to_i64(grant.revoked_at_ms))
            .bind(option_u64_to_i64(grant.reauth_required_at_ms))
            .bind(&grant.last_refresh_error)
            .bind(&grant.encrypted_oauth_grant)
            .bind(&grant.oauth_grant_key_id)
            .bind(&grant.oauth_grant_fingerprint)
            .bind(&grant.revocation_reason)
            .fetch_one(&self.pool)
            .await?;
        encrypted_token_grant_from_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(GET_TOKEN_GRANT_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn apply_refresh_command(
        &self,
        command: TokenRefreshRepositoryCommand,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        match command {
            TokenRefreshRepositoryCommand::RotateGrantCas {
                grant_id,
                tenant_id,
                expected_fingerprint,
                expires_at_ms,
                refreshed_at_ms,
                encrypted_grant_blob,
                grant_key_id,
                new_fingerprint,
            } => {
                self.rotate_encrypted_grant(RotateEncryptedGrantRequest {
                    tenant_id: &tenant_id.0,
                    id: &grant_id.0,
                    expected_fingerprint: &expected_fingerprint,
                    expires_at_ms,
                    refreshed_at_ms,
                    encrypted_oauth_grant: &encrypted_grant_blob.0,
                    oauth_grant_key_id: &grant_key_id,
                    oauth_grant_fingerprint: &new_fingerprint,
                })
                .await
            }
            TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                expected_fingerprint,
                refreshed_at_ms,
                safe_error,
            } => {
                self.mark_refresh_failed(
                    &tenant_id.0,
                    &grant_id.0,
                    &expected_fingerprint,
                    refreshed_at_ms,
                    &safe_error,
                )
                .await
            }
            TokenRefreshRepositoryCommand::MarkReauthRequired {
                grant_id,
                tenant_id,
                expected_fingerprint,
                reauth_required_at_ms,
                safe_error,
            } => {
                self.mark_reauth_required(
                    &tenant_id.0,
                    &grant_id.0,
                    &expected_fingerprint,
                    reauth_required_at_ms,
                    &safe_error,
                )
                .await
            }
            TokenRefreshRepositoryCommand::MarkConfigRequired {
                grant_id,
                tenant_id,
                expected_fingerprint,
                refreshed_at_ms,
                safe_error,
            } => {
                self.mark_refresh_failed(
                    &tenant_id.0,
                    &grant_id.0,
                    &expected_fingerprint,
                    refreshed_at_ms,
                    &safe_error,
                )
                .await
            }
        }
    }

    pub async fn rotate_encrypted_grant(
        &self,
        request: RotateEncryptedGrantRequest<'_>,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(ROTATE_TOKEN_GRANT)
            .bind(request.tenant_id)
            .bind(request.id)
            .bind(request.expected_fingerprint)
            .bind(option_u64_to_i64(request.expires_at_ms))
            .bind(request.refreshed_at_ms as i64)
            .bind(request.encrypted_oauth_grant)
            .bind(request.oauth_grant_key_id)
            .bind(request.oauth_grant_fingerprint)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn mark_refresh_failed(
        &self,
        tenant_id: &str,
        id: &str,
        expected_fingerprint: &str,
        refreshed_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let reason = sanitize_refresh_error_for_storage(reason);
        let row = sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(refreshed_at_ms as i64)
            .bind(&reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn mark_reauth_required(
        &self,
        tenant_id: &str,
        id: &str,
        expected_fingerprint: &str,
        reauth_required_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let reason = sanitize_refresh_error_for_storage(reason);
        let row = sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(reauth_required_at_ms as i64)
            .bind(&reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn revoke(
        &self,
        tenant_id: &str,
        id: &str,
        revoked_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(REVOKE_TOKEN_GRANT)
            .bind(tenant_id)
            .bind(id)
            .bind(revoked_at_ms as i64)
            .bind(reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn list_refresh_candidate_snapshots(
        &self,
        tenant_id: &str,
        due_before: SystemTime,
        limit: u32,
    ) -> PgRepositoryResult<Vec<TokenRefreshGrantSnapshot>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let due_before_ms = system_time_to_ms(due_before)? as i64;
        let rows = sqlx::query(LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS)
            .bind(tenant_id)
            .bind(due_before_ms)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(token_refresh_snapshot_from_row).collect()
    }
}
