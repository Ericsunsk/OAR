use super::*;

impl PostgresDeviceSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_with_identity_hash(
        &self,
        session: &crate::domain::device_sync::DeviceSession,
        session_identity_hash: &str,
    ) -> PgRepositoryResult<StoredDeviceSession> {
        let row = sqlx::query(UPSERT_DEVICE_SESSION)
            .bind(&session.id.0)
            .bind(&session.tenant_id.0)
            .bind(&session.user_id.0)
            .bind(device_entry_point_to_db(&session.entry_point))
            .bind(device_session_state_to_db(&session.state))
            .bind(&session.cursor.stream)
            .bind(session.cursor.value as i64)
            .bind(system_time_to_ms(session.cursor.updated_at)? as i64)
            .bind(session_identity_hash)
            .bind(system_time_to_ms(session.last_seen_at)? as i64)
            .bind(option_system_time_to_i64_ms(session.revoked_at)?)
            .bind(option_system_time_to_i64_ms(session.expired_at)?)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row.as_ref() {
            return stored_device_session_from_row(row);
        }

        let conflicting_tenant = sqlx::query("SELECT 1 FROM device_sessions WHERE id = $1 LIMIT 1")
            .bind(&session.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: session.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
            });
        }

        Err(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        session_id: &str,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(GET_DEVICE_SESSION_BY_ID)
            .bind(tenant_id)
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn advance_cursor_cas(
        &self,
        tenant_id: &str,
        session_id: &str,
        expected_cursor: u64,
        next_cursor: u64,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let now_ms = system_time_to_ms(now)? as i64;
        let row = sqlx::query(ADVANCE_DEVICE_SESSION_CURSOR_CAS)
            .bind(tenant_id)
            .bind(session_id)
            .bind(next_cursor as i64)
            .bind(now_ms)
            .bind(expected_cursor as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn revoke(
        &self,
        tenant_id: &str,
        session_id: &str,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(REVOKE_DEVICE_SESSION)
            .bind(tenant_id)
            .bind(session_id)
            .bind(system_time_to_ms(now)? as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn expire(
        &self,
        tenant_id: &str,
        session_id: &str,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(EXPIRE_DEVICE_SESSION)
            .bind(tenant_id)
            .bind(session_id)
            .bind(system_time_to_ms(now)? as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }
}

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
}

impl PostgresOarUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, user: &OarUser) -> PgRepositoryResult<StoredOarUser> {
        let row = sqlx::query(UPSERT_OAR_USER)
            .bind(&user.id.0)
            .bind(&user.tenant_id.0)
            .bind(&user.display_name)
            .bind(oar_user_status_to_db(&user.status))
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row.as_ref() {
            return stored_oar_user_from_row(row);
        }

        let conflicting_tenant = sqlx::query("SELECT 1 FROM oar_users WHERE id = $1 LIMIT 1")
            .bind(&user.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: user.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
            });
        }

        Err(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> PgRepositoryResult<Option<StoredOarUser>> {
        let row = sqlx::query(GET_OAR_USER_BY_ID)
            .bind(tenant_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_oar_user_from_row).transpose()
    }
}

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

        let conflicting_tenant = sqlx::query("SELECT 1 FROM lark_identities WHERE id = $1 LIMIT 1")
            .bind(&identity.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: identity.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
            });
        }

        Err(sqlx::Error::RowNotFound.into())
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

    pub fn users(&self) -> PostgresOarUserRepository {
        PostgresOarUserRepository::new(self.pool.clone())
    }

    pub fn identities(&self) -> PostgresLarkIdentityRepository {
        PostgresLarkIdentityRepository::new(self.pool.clone())
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
