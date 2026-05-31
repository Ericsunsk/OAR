use super::super::super::postgres_support::{device_session, run_live_postgres_test, seed_user};
use super::{configured_env, configured_runtime};
use crate::feishu_auth::FeishuLoginRuntime;
use crate::persistence::FacadePersistenceRuntime;
use crate::{dispatch_request_with_runtime, OarHttpFacadeRuntime};
use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::SessionState;
use oar_core::domain::identity::{ActorKind, ScopeBoundary, TokenGrantState};
use oar_core::storage::postgres::{
    EncryptedTokenGrantRecord, PostgresDeviceSessionRepository, PostgresTokenGrantRepository,
};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn configured_runtime_logout_requires_oar_bearer_and_session_store() {
    let runtime = configured_runtime();

    let missing = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/logout",
        None,
        None,
        None,
    )
    .await;
    let invalid = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/logout",
        None,
        Some("Bearer feishu_token"),
        None,
    )
    .await;
    let unavailable = dispatch_request_with_runtime(
        runtime,
        &Method::POST,
        "/auth/logout",
        None,
        Some("Bearer oar_session_dev"),
        None,
    )
    .await;
    let unavailable_body: Value = serde_json::from_str(&unavailable.body).expect("json");

    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(invalid.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unavailable.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable_body["error"],
        "oar_session_verification_unavailable"
    );
    assert!(!unavailable.body.contains("super-secret"));
    assert!(!unavailable.body.contains("oar_session_dev"));
}

#[tokio::test]
async fn logout_route_revokes_active_oar_session_and_is_idempotent_when_database_is_available() {
    run_live_postgres_test(
        "logout_route_revokes_active_oar_session",
        |pool| async move {
            let tenant_id = "tenant_logout_route";
            let user_id = "user_logout_route";
            let session_id = "oar_session_logout_route";
            seed_user(&pool, tenant_id, user_id).await?;

            let repository = PostgresDeviceSessionRepository::new(pool.clone());
            let now = SystemTime::UNIX_EPOCH + Duration::from_millis(1_748_310_000_000);
            seed_oar_device_session(
                &pool,
                tenant_id,
                user_id,
                session_id,
                "sha256:logout-route",
                now,
            )
            .await?;

            let runtime = configured_runtime_with_persistence(pool);
            let first = post_logout(Arc::clone(&runtime), session_id).await;
            let first_body: Value = serde_json::from_str(&first.body).expect("first logout json");
            assert_eq!(first.status, StatusCode::OK);
            assert_eq!(first_body["status"], "signed_out");

            let revoked = repository
                .get_by_id(tenant_id, session_id)
                .await?
                .expect("session should still exist after logout");
            assert_eq!(revoked.state, SessionState::Revoked);
            let revoked_at = revoked.revoked_at.expect("revoked_at should be set");

            let second = post_logout(runtime, session_id).await;
            let second_body: Value =
                serde_json::from_str(&second.body).expect("second logout json");
            assert_eq!(second.status, StatusCode::OK);
            assert_eq!(second_body["status"], "signed_out");

            let after_second = repository
                .get_by_id(tenant_id, session_id)
                .await?
                .expect("session should still exist after second logout");
            assert_eq!(after_second.state, SessionState::Revoked);
            assert_eq!(after_second.revoked_at, Some(revoked_at));

            Ok(())
        },
    )
    .await;
}

#[tokio::test]
async fn logout_route_revokes_user_grant_when_last_device_signs_out() {
    run_live_postgres_test(
        "logout_route_revokes_user_grant_when_last_device_signs_out",
        |pool| async move {
            let tenant_id = "tenant_logout_last_device";
            let user_tail = "logout_last_device";
            let user_id = format!("feishu_user_{user_tail}");
            let session_id = "oar_session_logout_last_device";
            seed_user(&pool, tenant_id, &user_id).await?;
            let grant = seed_feishu_user_grant(&pool, tenant_id, user_tail).await?;

            let now = SystemTime::UNIX_EPOCH + Duration::from_millis(1_748_310_100_000);
            seed_oar_device_session(
                &pool,
                tenant_id,
                &user_id,
                session_id,
                "sha256:logout-last-device",
                now,
            )
            .await?;

            let runtime = configured_runtime_with_persistence(pool.clone());
            let response = post_logout(runtime, session_id).await;
            let body: Value = serde_json::from_str(&response.body).expect("logout json");
            assert_eq!(response.status, StatusCode::OK);
            assert_eq!(body["status"], "signed_out");

            let revoked_grant = PostgresTokenGrantRepository::new(pool.clone())
                .get_by_id(tenant_id, &grant.id)
                .await?
                .expect("grant should still exist");
            assert_eq!(revoked_grant.state, TokenGrantState::Revoked);
            assert_eq!(
                revoked_grant.revocation_reason.as_deref(),
                Some("oar_session_logout_last_device")
            );
            assert!(revoked_grant.revoked_at_ms.is_some());
            assert!(!response.body.contains("refresh-token"));
            assert!(!response.body.contains("access-token"));

            let audit = sqlx::query(
                r#"
                SELECT
                    trace_id,
                    event_type,
                    actor_id,
                    target_resource_type,
                    target_action_type,
                    jsonb_build_object(
                        'before_summary', before_summary,
                        'after_summary', after_summary,
                        'execution_result', execution_result
                    ) AS payload
                FROM audit_events
                WHERE tenant_id = $1
                  AND target_resource_id = $2
                "#,
            )
            .bind(tenant_id)
            .bind(&grant.id)
            .fetch_one(&pool)
            .await?;
            let trace_id: String = audit.try_get("trace_id")?;
            assert!(trace_id.starts_with("auth-logout-grant-revoke-"));
            assert!(!trace_id.contains(session_id));
            assert!(!trace_id.contains(&user_id));
            assert!(!trace_id.contains(&grant.id));
            assert_eq!(
                audit.try_get::<String, _>("event_type")?,
                "execution_succeeded"
            );
            assert_eq!(audit.try_get::<String, _>("actor_id")?, user_id);
            assert_eq!(
                audit.try_get::<String, _>("target_resource_type")?,
                "token_grant"
            );
            assert_eq!(
                audit.try_get::<String, _>("target_action_type")?,
                "token_grant.revoke.logout_last_device"
            );
            let payload: Value = audit.try_get("payload")?;
            let payload_text = payload.to_string();
            assert!(!payload_text.contains("refresh-token"));
            assert!(!payload_text.contains("access-token"));
            assert!(!payload_text.contains("oauth_grant_fingerprint"));

            Ok(())
        },
    )
    .await;
}

#[tokio::test]
async fn logout_route_keeps_user_grant_when_another_device_session_is_active() {
    run_live_postgres_test(
        "logout_route_keeps_user_grant_when_another_device_session_is_active",
        |pool| async move {
            let tenant_id = "tenant_logout_keep_grant";
            let user_tail = "logout_keep_grant";
            let user_id = format!("feishu_user_{user_tail}");
            seed_user(&pool, tenant_id, &user_id).await?;
            let grant = seed_feishu_user_grant(&pool, tenant_id, user_tail).await?;

            let now = SystemTime::UNIX_EPOCH + Duration::from_millis(1_748_310_200_000);
            seed_oar_device_session(
                &pool,
                tenant_id,
                &user_id,
                "oar_session_logout_keep_grant_a",
                "sha256:logout-keep-a",
                now,
            )
            .await?;
            seed_oar_device_session(
                &pool,
                tenant_id,
                &user_id,
                "oar_session_logout_keep_grant_b",
                "sha256:logout-keep-b",
                now,
            )
            .await?;

            let runtime = configured_runtime_with_persistence(pool.clone());
            let response = post_logout(runtime, "oar_session_logout_keep_grant_a").await;
            let body: Value = serde_json::from_str(&response.body).expect("logout json");
            assert_eq!(response.status, StatusCode::OK);
            assert_eq!(body["status"], "signed_out");

            let still_valid_grant = PostgresTokenGrantRepository::new(pool.clone())
                .get_by_id(tenant_id, &grant.id)
                .await?
                .expect("grant should still exist");
            assert_eq!(still_valid_grant.state, TokenGrantState::Valid);
            assert_eq!(still_valid_grant.revoked_at_ms, None);
            assert_eq!(still_valid_grant.revocation_reason, None);

            let audit_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM audit_events
                WHERE tenant_id = $1
                  AND target_action_type = $2
                "#,
            )
            .bind(tenant_id)
            .bind("token_grant.revoke.logout_last_device")
            .fetch_one(&pool)
            .await?;
            assert_eq!(audit_count, 0);

            Ok(())
        },
    )
    .await;
}

fn configured_runtime_with_persistence(pool: sqlx::PgPool) -> Arc<OarHttpFacadeRuntime> {
    let persistence =
        FacadePersistenceRuntime::new_for_test(pool, "key-test-v1".to_string(), [7; 32]);
    let feishu_login = FeishuLoginRuntime::from_env_map(&configured_env)
        .expect("login runtime")
        .expect("configured login runtime");
    Arc::new(OarHttpFacadeRuntime {
        persistence: Some(persistence),
        feishu_login: Some(Arc::new(feishu_login)),
        agent: None,
        agent_settings: None,
        tenant_maintenance: None,
        tenant_maintenance_daemon_status: Default::default(),
    })
}

async fn post_logout(
    runtime: Arc<OarHttpFacadeRuntime>,
    session_id: &str,
) -> crate::FacadeResponse {
    dispatch_request_with_runtime(
        runtime,
        &Method::POST,
        "/auth/logout",
        None,
        Some(&format!("Bearer {session_id}")),
        None,
    )
    .await
}

async fn seed_oar_device_session(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    user_id: &str,
    session_id: &str,
    identity_hash: &str,
    now: SystemTime,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session = device_session(tenant_id, user_id, session_id, "review_inbox", 0, now);
    PostgresDeviceSessionRepository::new(pool.clone())
        .upsert_with_identity_hash(&session, identity_hash)
        .await?;
    Ok(())
}

async fn seed_feishu_user_grant(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    user_tail: &str,
) -> Result<EncryptedTokenGrantRecord, Box<dyn std::error::Error + Send + Sync>> {
    let identity_id = format!("feishu_identity_{user_tail}");
    sqlx::query(
        r#"
        INSERT INTO lark_identities (id, tenant_id, actor_kind, actor_external_id, display_name)
        VALUES ($1, $2, 'user', $3, $4)
        "#,
    )
    .bind(&identity_id)
    .bind(tenant_id)
    .bind(format!("ou_{user_tail}"))
    .bind(format!("Feishu User {user_tail}"))
    .execute(pool)
    .await?;

    let grant = EncryptedTokenGrantRecord {
        id: format!("feishu_grant_{user_tail}"),
        tenant_id: tenant_id.to_string(),
        identity_id,
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["offline_access".to_string()],
        state: TokenGrantState::Valid,
        issued_at_ms: 1_748_300_000_000,
        expires_at_ms: Some(1_748_360_000_000),
        refreshed_at_ms: Some(1_748_300_000_000),
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: None,
        encrypted_oauth_grant: b"encrypted-refresh-token".to_vec(),
        oauth_grant_key_id: "key-test-v1".to_string(),
        oauth_grant_fingerprint: format!("fp_{user_tail}"),
        revocation_reason: None,
    };
    PostgresTokenGrantRepository::new(pool.clone())
        .upsert_encrypted_grant(&grant)
        .await?;
    Ok(grant)
}
