use super::super::*;

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

        tenant_mismatch_or_row_not_found(
            &self.pool,
            "SELECT 1 FROM device_sessions WHERE id = $1 LIMIT 1",
            &session.id.0,
            &session.tenant_id.0,
        )
        .await
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
