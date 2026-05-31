use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditScope, AuditStateSummary,
    AuditSubject, AuditTarget,
};

use super::super::*;

const AUTH_LOGOUT_GRANT_RESOURCE_TYPE: &str = "token_grant";

impl PostgresAuthLifecycleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn revoke_logout_session_and_last_device_grants(
        &self,
        request: PostgresAuthLogoutRevokeRequest<'_>,
    ) -> PgRepositoryResult<PostgresAuthLogoutRevokeReport> {
        let occurred_at_ms = request.occurred_at_ms as i64;
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            SELECT id
            FROM device_sessions
            WHERE tenant_id = $1
              AND user_id = $2
              AND state = 'active'
              AND revoked_at IS NULL
              AND expired_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(request.tenant_id)
        .bind(request.user_id)
        .fetch_all(&mut *tx)
        .await?;

        sqlx::query(REVOKE_DEVICE_SESSION)
            .bind(request.tenant_id)
            .bind(request.session_id)
            .bind(occurred_at_ms)
            .fetch_optional(&mut *tx)
            .await?;

        let remaining_active_sessions = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM device_sessions
            WHERE tenant_id = $1
              AND user_id = $2
              AND state = 'active'
              AND revoked_at IS NULL
              AND expired_at IS NULL
            "#,
        )
        .bind(request.tenant_id)
        .bind(request.user_id)
        .fetch_one(&mut *tx)
        .await?;

        let mut revoked_grant_ids = Vec::new();
        if remaining_active_sessions == 0 {
            revoked_grant_ids = revoke_user_grants_in_tx(&mut tx, &request, occurred_at_ms).await?;
            for (index, grant_id) in revoked_grant_ids.iter().enumerate() {
                let event = logout_grant_revoke_audit_event(&request, grant_id, (index + 1) as u64);
                super::super::audit::append_audit_event_in_tx(&mut tx, &event, None).await?;
            }
        }

        tx.commit().await?;
        Ok(PostgresAuthLogoutRevokeReport { revoked_grant_ids })
    }
}

async fn revoke_user_grants_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    request: &PostgresAuthLogoutRevokeRequest<'_>,
    occurred_at_ms: i64,
) -> PgRepositoryResult<Vec<String>> {
    let revoked_grant_ids = sqlx::query_scalar::<_, String>(
        r#"
        WITH revoked AS (
        UPDATE token_grants tg
        SET state = 'revoked',
            revoked_at = to_timestamp($3::double precision / 1000.0),
            revocation_reason = $4,
            updated_at = to_timestamp($3::double precision / 1000.0)
        FROM lark_identities li
        WHERE tg.tenant_id = $1
          AND tg.identity_id = li.id
          AND li.tenant_id = tg.tenant_id
          AND tg.actor_kind = 'user'
          AND tg.scope_boundary = 'user'
          AND tg.state <> 'revoked'
          AND (
              ($5::text IS NOT NULL AND tg.id = $5)
              OR (li.actor_kind = 'user' AND li.actor_external_id = $2)
          )
          RETURNING tg.id
        )
        SELECT id FROM revoked ORDER BY id
        "#,
    )
    .bind(request.tenant_id)
    .bind(request.user_id)
    .bind(occurred_at_ms)
    .bind(request.revocation_reason)
    .bind(request.grant_id_hint)
    .fetch_all(&mut **tx)
    .await?;
    Ok(revoked_grant_ids)
}

fn logout_grant_revoke_audit_event(
    request: &PostgresAuthLogoutRevokeRequest<'_>,
    grant_id: &str,
    sequence: u64,
) -> AuditEvent {
    AuditEvent::execution_succeeded(
        AuditEventContext {
            event_id: format!("{}-evt-{sequence}", request.audit_trace_id),
            trace_id: request.audit_trace_id.to_string(),
            sequence,
            occurred_at_ms: request.occurred_at_ms,
            subject: AuditSubject {
                actor: AuditActor {
                    kind: AuditActorKind::User,
                    actor_id: request.user_id.to_string(),
                    display_name: None,
                },
                scope: AuditScope {
                    tenant_id: request.tenant_id.to_string(),
                    workspace_id: None,
                },
                target: AuditTarget {
                    resource_type: AUTH_LOGOUT_GRANT_RESOURCE_TYPE.to_string(),
                    resource_id: grant_id.to_string(),
                    action_type: request.audit_action_type.to_string(),
                },
            },
        },
        Some(AuditStateSummary {
            summary: "Local Feishu token grant was active before last-device logout revoke."
                .to_string(),
            reference_ids: Vec::new(),
            content_hash: None,
        }),
        Some(AuditStateSummary {
            summary:
                "Local Feishu token grant was revoked after the last active OAR device signed out."
                    .to_string(),
            reference_ids: Vec::new(),
            content_hash: None,
        }),
        format!("token-grant-revoke:{grant_id}"),
    )
}
