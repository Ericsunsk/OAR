use super::*;

const EXPECTED_FINGERPRINT: &str = "fp_prev_v1";

pub(crate) async fn seed_feishu_refresh_grant(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    identity_id: &str,
    grant_id: &str,
    state: TokenGrantState,
) -> Result<PostgresTokenGrantRepository, Box<dyn std::error::Error + Send + Sync>> {
    seed_user(pool, tenant_id, user_id).await?;
    seed_identity(pool, tenant_id, identity_id).await?;

    let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
    grant_repo
        .upsert_encrypted_grant(&encrypted_token_grant_record(
            tenant_id,
            grant_id,
            identity_id,
            state,
            EXPECTED_FINGERPRINT,
        ))
        .await?;

    Ok(grant_repo)
}

pub(crate) fn feishu_refresh_snapshot(
    tenant_id: &str,
    grant_id: &str,
    state: TokenGrantState,
) -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot {
        grant_id: TokenGrantId(grant_id.to_string()),
        tenant_id: TenantId(tenant_id.to_string()),
        expected_fingerprint: EXPECTED_FINGERPRINT.to_string(),
        state,
        has_refresh_material: true,
        revoked_at: None,
        reauth_required_at: None,
    }
}

pub(crate) fn feishu_refresh_audit_context(
    trace_id: &str,
    sequence: u64,
    occurred_at_ms: u64,
    actor_user_id: &str,
) -> TokenRefreshAuditContext {
    TokenRefreshAuditContext {
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms,
        actor: actor(actor_user_id),
        workspace_id: None,
    }
}

pub(crate) fn feishu_fixture_orchestrator(
    pool: PgPool,
    fixture_body: &'static str,
) -> (
    FixtureClient,
    PostgresTokenRefreshOrchestrator<FeishuAuthRefreshAdapter<FixtureClient>>,
) {
    let client = FixtureClient::new(fixture_body);
    let orchestrator =
        PostgresTokenRefreshOrchestrator::new(pool, FeishuAuthRefreshAdapter::new(client.clone()));
    (client, orchestrator)
}

pub(crate) fn system_time_ms(value: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(value)
}

pub(crate) async fn audit_refresh_payload_text(
    pool: &PgPool,
    event_id: &str,
) -> Result<String, sqlx::Error> {
    let payload: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT
        jsonb_build_object(
          'before_summary', before_summary,
          'after_summary', after_summary,
          'execution_result', execution_result
        )
        FROM audit_events
        WHERE event_id = $1
        "#,
    )
    .bind(event_id)
    .fetch_one(pool)
    .await?;

    Ok(payload.to_string())
}
