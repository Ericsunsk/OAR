use std::time::SystemTime;

use oar_core::storage::postgres::{
    EncryptedTokenGrantRecord, PostgresAuthLifecycleRepository, PostgresAuthLogoutRevokeRequest,
};

use crate::AuthenticatedContext;

use super::util::{stable_sha256_hex, system_time_to_ms_lossy};

const WORKSPACE_USER_PREFIX: &str = "feishu_user_";
const GRANT_PREFIX: &str = "feishu_grant_";
const LOGOUT_GRANT_REVOCATION_REASON: &str = "oar_session_logout_last_device";
const LOGOUT_GRANT_REVOKE_ACTION_TYPE: &str = "token_grant.revoke.logout_last_device";

pub(crate) async fn resolve_grant_id_for_user(
    pool: &sqlx::PgPool,
    auth_context: &AuthenticatedContext,
) -> Result<String, &'static str> {
    if let Some(grant_id) = grant_id_for_workspace_user_id(&auth_context.user_id) {
        let matched_grant_id = sqlx::query_scalar::<_, String>(
            r#"
            SELECT tg.id
            FROM token_grants tg
            INNER JOIN device_sessions ds
              ON ds.tenant_id = tg.tenant_id
             AND ds.user_id = $2
             AND ds.id = $3
            WHERE tg.tenant_id = $1
              AND tg.id = $4
              AND tg.actor_kind = 'user'
              AND tg.scope_boundary = 'user'
              AND tg.revoked_at IS NULL
              AND tg.reauth_required_at IS NULL
              AND ds.state = 'active'
              AND ds.revoked_at IS NULL
              AND ds.expired_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(&auth_context.tenant_id)
        .bind(&auth_context.user_id)
        .bind(&auth_context.session_id)
        .bind(&grant_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "查询用户授权失败")?;
        return matched_grant_id.ok_or("当前用户没有可用的授权 grant");
    }

    let grant_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT tg.id
        FROM token_grants tg
        INNER JOIN lark_identities li
          ON li.tenant_id = tg.tenant_id
         AND li.id = tg.identity_id
        WHERE tg.tenant_id = $1
          AND li.actor_kind = 'user'
          AND li.actor_external_id = $2
          AND tg.revoked_at IS NULL
          AND tg.reauth_required_at IS NULL
        ORDER BY tg.refreshed_at DESC NULLS LAST, tg.issued_at DESC
        LIMIT 2
        "#,
    )
    .bind(&auth_context.tenant_id)
    .bind(&auth_context.user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| "查询用户授权失败")?;

    match grant_ids.as_slice() {
        [] => Err("当前用户没有可用的授权 grant"),
        [grant_id] => Ok(grant_id.clone()),
        _ => Err("用户授权 grant 不唯一"),
    }
}

pub(crate) async fn resolve_lark_open_id_for_grant(
    pool: &sqlx::PgPool,
    auth_context: &AuthenticatedContext,
    token_grant: &EncryptedTokenGrantRecord,
) -> Result<String, &'static str> {
    let open_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT li.actor_external_id
        FROM lark_identities li
        WHERE li.tenant_id = $1
          AND li.id = $2
          AND li.actor_kind = 'user'
        LIMIT 1
        "#,
    )
    .bind(&auth_context.tenant_id)
    .bind(&token_grant.identity_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| "查询用户飞书身份失败")?
    .ok_or("当前用户没有可用的飞书身份")?;

    if open_id.trim().is_empty() {
        return Err("当前用户飞书身份为空");
    }
    Ok(open_id)
}

pub(crate) async fn revoke_logout_session_and_last_device_grant(
    pool: sqlx::PgPool,
    auth_context: &AuthenticatedContext,
    now: SystemTime,
) -> Result<(), oar_core::storage::postgres::PostgresRepositoryError> {
    let now_ms = system_time_to_ms_lossy(now);
    let generated_grant_id = grant_id_for_workspace_user_id(&auth_context.user_id);
    PostgresAuthLifecycleRepository::new(pool)
        .revoke_logout_session_and_last_device_grants(PostgresAuthLogoutRevokeRequest {
            tenant_id: &auth_context.tenant_id,
            user_id: &auth_context.user_id,
            session_id: &auth_context.session_id,
            grant_id_hint: generated_grant_id.as_deref(),
            occurred_at_ms: now_ms,
            revocation_reason: LOGOUT_GRANT_REVOCATION_REASON,
            audit_trace_id: &safe_logout_grant_revoke_trace_id(auth_context, now_ms),
            audit_action_type: LOGOUT_GRANT_REVOKE_ACTION_TYPE,
        })
        .await
        .map(|_| ())
}

fn safe_logout_grant_revoke_trace_id(
    auth_context: &AuthenticatedContext,
    occurred_at_ms: u64,
) -> String {
    let digest = stable_sha256_hex(&[
        &auth_context.tenant_id,
        &auth_context.user_id,
        &auth_context.session_id,
        &occurred_at_ms.to_string(),
        LOGOUT_GRANT_REVOKE_ACTION_TYPE,
    ]);
    format!("auth-logout-grant-revoke-{}", &digest[..24])
}

fn grant_id_for_workspace_user_id(user_id: &str) -> Option<String> {
    user_id
        .strip_prefix(WORKSPACE_USER_PREFIX)
        .map(|user_tail| format!("{GRANT_PREFIX}{user_tail}"))
}
