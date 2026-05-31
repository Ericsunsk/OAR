use std::time::SystemTime;

use oar_core::storage::postgres::device_session_sql::REVOKE_DEVICE_SESSION;
use oar_core::storage::postgres::EncryptedTokenGrantRecord;

use crate::AuthenticatedContext;

use super::util::system_time_to_ms_lossy;

const WORKSPACE_USER_PREFIX: &str = "feishu_user_";
const GRANT_PREFIX: &str = "feishu_grant_";
const LOGOUT_GRANT_REVOCATION_REASON: &str = "oar_session_logout_last_device";

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
) -> Result<(), sqlx::Error> {
    let now_ms = system_time_to_ms_lossy(now) as i64;
    let generated_grant_id = grant_id_for_workspace_user_id(&auth_context.user_id);
    let mut tx = pool.begin().await?;

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
    .bind(&auth_context.tenant_id)
    .bind(&auth_context.user_id)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query(REVOKE_DEVICE_SESSION)
        .bind(&auth_context.tenant_id)
        .bind(&auth_context.session_id)
        .bind(now_ms)
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
    .bind(&auth_context.tenant_id)
    .bind(&auth_context.user_id)
    .fetch_one(&mut *tx)
    .await?;

    if remaining_active_sessions == 0 {
        sqlx::query(
            r#"
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
            "#,
        )
        .bind(&auth_context.tenant_id)
        .bind(&auth_context.user_id)
        .bind(now_ms)
        .bind(LOGOUT_GRANT_REVOCATION_REASON)
        .bind(generated_grant_id.as_deref())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

fn grant_id_for_workspace_user_id(user_id: &str) -> Option<String> {
    user_id
        .strip_prefix(WORKSPACE_USER_PREFIX)
        .map(|user_tail| format!("{GRANT_PREFIX}{user_tail}"))
}
