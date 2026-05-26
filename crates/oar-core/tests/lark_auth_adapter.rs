use std::time::{Duration, SystemTime};

use oar_core::domain::identity::{TenantId, TokenGrantState};
use oar_core::domain::token_refresh::{
    AuthRefreshAdapter, RefreshOutcome, TokenRefreshGrantSnapshot,
};
use oar_core::lark::auth::{
    parse_lark_auth_refresh_response, LarkAuthGrantState, LarkAuthRefreshAdapter,
    LarkAuthRefreshClient, LarkAuthRefreshRequest, LarkAuthRefreshResponse,
};
use oar_core::lark::fixtures::{
    AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON, AUTH_REFRESH_REAUTH_REQUIRED_JSON,
    AUTH_REFRESH_ROTATED_ENCRYPTED_JSON, AUTH_REFRESH_TRANSIENT_FAILURE_JSON,
};

fn sample_snapshot() -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot {
        grant_id: oar_core::domain::identity::TokenGrantId("grant_auth_refresh_1".to_string()),
        tenant_id: TenantId("tenant_auth_refresh_1".to_string()),
        expected_fingerprint: "fp_prev_v1".to_string(),
        state: TokenGrantState::Valid,
        has_refresh_material: true,
        revoked_at: None,
        reauth_required_at: None,
    }
}

#[test]
fn request_from_snapshot_maps_safe_metadata_and_redacts_debug() {
    let request = LarkAuthRefreshRequest::from_snapshot(&sample_snapshot());

    assert_eq!(request.grant_id, "grant_auth_refresh_1");
    assert_eq!(request.tenant_id, "tenant_auth_refresh_1");
    assert_eq!(request.grant_state, LarkAuthGrantState::Valid);
    assert!(request.has_refresh_material);
    assert!(!request.is_revoked);
    assert!(!request.reauth_marked);

    let debug = format!("{request:?}");
    assert!(debug.contains("grant_auth_refresh_1"));
    assert!(!debug.contains("fp_prev_v1"));
}

#[test]
fn rotated_encrypted_fixture_parses_and_maps_success() {
    let parsed = parse_lark_auth_refresh_response(AUTH_REFRESH_ROTATED_ENCRYPTED_JSON)
        .expect("rotated encrypted fixture should parse");
    let mut adapter = LarkAuthRefreshAdapter::new(FakeClient::from_response(parsed));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::Success {
            rotated_material,
            key_id,
            new_fingerprint,
            refreshed_at,
            expires_at,
        } => {
            assert_eq!(rotated_material.encrypted_primary, vec![1, 2, 3, 4, 5]);
            assert_eq!(rotated_material.encrypted_renewal, vec![6, 7, 8, 9, 10]);
            assert_eq!(key_id, "kms-key-2026-05");
            assert_eq!(new_fingerprint, "fp_rotated_v2");
            assert_eq!(
                refreshed_at,
                SystemTime::UNIX_EPOCH + Duration::from_millis(1_779_465_600_000)
            );
            assert_eq!(
                expires_at,
                Some(SystemTime::UNIX_EPOCH + Duration::from_millis(1_779_472_800_000))
            );
        }
        other => panic!("expected success outcome, got {other:?}"),
    }
}

#[test]
fn reauth_required_fixture_maps_to_reauth_failure() {
    let parsed = parse_lark_auth_refresh_response(AUTH_REFRESH_REAUTH_REQUIRED_JSON)
        .expect("reauth-required fixture should parse");
    let mut adapter = LarkAuthRefreshAdapter::new(FakeClient::from_response(parsed));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ReauthFailure { safe_error } => {
            assert_eq!(safe_error, "invalid_grant");
        }
        other => panic!("expected reauth failure, got {other:?}"),
    }
}

#[test]
fn transient_failure_fixture_maps_to_transient_failure() {
    let parsed = parse_lark_auth_refresh_response(AUTH_REFRESH_TRANSIENT_FAILURE_JSON)
        .expect("transient failure fixture should parse");
    let mut adapter = LarkAuthRefreshAdapter::new(FakeClient::from_response(parsed));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::TransientFailure { safe_error } => {
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected transient failure, got {other:?}"),
    }
}

#[test]
fn plaintext_token_fixture_is_rejected_without_leaking_token_text() {
    let err = parse_lark_auth_refresh_response(AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON)
        .expect_err("plaintext token fixture must be rejected");
    let rendered = err.to_string();
    let debug = format!("{err:?}");

    assert!(!rendered.contains("tok_access_live_should_never_parse"));
    assert!(!rendered.contains("tok_refresh_live_should_never_parse"));
    assert!(!debug.contains("tok_access_live_should_never_parse"));
    assert!(!debug.contains("tok_refresh_live_should_never_parse"));
    assert!(!rendered.contains("refresh_token="));
    assert!(!debug.contains("refresh_token="));
}

#[test]
fn adapter_debug_redacts_client_internals() {
    let adapter = LarkAuthRefreshAdapter::new(FakeClient::from_error(
        "access_token=tok_debug_sensitive refresh_token=rt_debug_sensitive",
    ));
    let debug = format!("{adapter:?}");

    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("tok_debug_sensitive"));
    assert!(!debug.contains("rt_debug_sensitive"));
    assert!(!debug.contains("access_token"));
    assert!(!debug.contains("refresh_token"));
}

#[test]
fn adapter_client_failure_is_redacted_to_safe_transient_error() {
    let mut adapter = LarkAuthRefreshAdapter::new(FakeClient::from_error(
        "backend returned access_token=tok_access_sensitive refresh_token=tok_refresh_sensitive",
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::TransientFailure { safe_error } => {
            assert!(!safe_error.contains("tok_access_sensitive"));
            assert!(!safe_error.contains("tok_refresh_sensitive"));
            assert!(!safe_error.contains("refresh_token="));
            assert!(!safe_error.contains("access_token="));
        }
        other => panic!("expected transient failure, got {other:?}"),
    }
}

#[derive(Debug, Clone)]
struct FakeClient {
    response: Option<LarkAuthRefreshResponse>,
    error: Option<String>,
}

impl FakeClient {
    fn from_response(response: LarkAuthRefreshResponse) -> Self {
        Self {
            response: Some(response),
            error: None,
        }
    }

    fn from_error(error: impl Into<String>) -> Self {
        Self {
            response: None,
            error: Some(error.into()),
        }
    }
}

impl LarkAuthRefreshClient for FakeClient {
    type Error = String;

    fn refresh(
        &mut self,
        _request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.response
            .clone()
            .ok_or_else(|| "missing fake response".to_string())
    }
}
