use super::*;

#[test]
fn live_read_refresh_trace_id_does_not_embed_session_or_grant() {
    let auth_context = AuthenticatedContext {
        session_id: "oar_session_secret".to_string(),
        tenant_id: "tenant_x".to_string(),
        user_id: "feishu_user_secret".to_string(),
    };

    let trace_id = safe_live_read_trace_id(&auth_context, "grant_secret", 42);

    assert!(trace_id.starts_with("live-feishu-read-"));
    assert!(!trace_id.contains("oar_session_secret"));
    assert!(!trace_id.contains("grant_secret"));
    assert!(!trace_id.contains("feishu_user_secret"));
}

#[test]
fn grant_refresh_predicate_uses_expiry_skew() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);

    let inside_skew =
        sample_token_grant_record(TokenGrantState::Valid, Some(now_ms + TOKEN_REFRESH_SKEW_MS));
    assert!(grant_requires_refresh_before_read(&inside_skew, now_ms));

    let outside_skew = sample_token_grant_record(
        TokenGrantState::Valid,
        Some(now_ms + TOKEN_REFRESH_SKEW_MS + 1),
    );
    assert!(!grant_requires_refresh_before_read(&outside_skew, now_ms));
}

#[test]
fn unusable_grant_states_deny_live_read_even_without_timestamps() {
    let revoked = sample_token_grant_record(TokenGrantState::Revoked, None);
    assert_eq!(
        live_read_grant_denial_reason(&revoked),
        Some("授权已失效，需要重新登录")
    );

    let reauth = sample_token_grant_record(TokenGrantState::ReauthRequired, None);
    assert_eq!(
        live_read_grant_denial_reason(&reauth),
        Some("授权已失效，需要重新登录")
    );

    let mut bot = sample_token_grant_record(TokenGrantState::Valid, None);
    bot.actor_kind = ActorKind::Bot;
    assert_eq!(
        live_read_grant_denial_reason(&bot),
        Some("授权主体不是当前用户")
    );
}

#[test]
fn grant_debug_redacts_token_material() {
    let mut grant = sample_token_grant_record(TokenGrantState::Valid, None);
    grant.encrypted_oauth_grant =
        b"access-token-sensitive refresh-token-sensitive raw-response".to_vec();

    let debug = format!("{grant:?}");

    assert!(!debug.contains("access-token-sensitive"));
    assert!(!debug.contains("refresh-token-sensitive"));
    assert!(!debug.contains("raw-response"));
}
