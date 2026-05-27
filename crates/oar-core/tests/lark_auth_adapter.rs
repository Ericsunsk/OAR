use std::time::{Duration, SystemTime};

use oar_core::domain::identity::{TenantId, TokenGrantState};
use oar_core::domain::token_refresh::service::AuthRefreshAdapter;
use oar_core::domain::token_refresh::types::{RefreshOutcome, TokenRefreshGrantSnapshot};
use oar_core::lark::auth::adapter::{LarkAuthRefreshAdapter, LarkAuthRefreshClient};
use oar_core::lark::auth::client::{
    LarkAuthRefreshClientError, LarkAuthRefreshRawEnvelope, LarkAuthRefreshSafeClient,
    LarkAuthRefreshSafeClientConfig, LarkAuthRefreshTransport,
};
use oar_core::lark::auth::parser::parse_lark_auth_refresh_response;
use oar_core::lark::auth::types::{
    LarkAuthGrantState, LarkAuthRefreshRequest, LarkAuthRefreshSuccess,
};
use oar_core::lark::fixtures::{
    AUTH_REFRESH_CONFIG_REQUIRED_JSON, AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON,
    AUTH_REFRESH_REAUTH_REQUIRED_JSON, AUTH_REFRESH_ROTATED_ENCRYPTED_JSON,
    AUTH_REFRESH_TRANSIENT_FAILURE_JSON,
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
fn safe_client_parses_encrypted_fixture_and_adapter_maps_success() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope(AUTH_REFRESH_ROTATED_ENCRYPTED_JSON),
    ));
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
fn refresh_success_debug_redacts_key_and_fingerprint_material() {
    let success = LarkAuthRefreshSuccess {
        encrypted_primary: vec![1, 2, 3],
        encrypted_renewal: vec![4, 5, 6],
        key_id: "kms-key-sensitive".to_string(),
        new_fingerprint: "fp-sensitive".to_string(),
        refreshed_at_ms: 123,
        expires_at_ms: Some(456),
    };

    let debug = format!("{success:?}");

    assert!(!debug.contains("kms-key-sensitive"));
    assert!(!debug.contains("fp-sensitive"));
    assert!(!debug.contains("[1, 2, 3]"));
    assert!(!debug.contains("[4, 5, 6]"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn plaintext_token_like_transport_output_is_rejected_and_maps_safe_parse_failure() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope(AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON),
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ConfigRequired { safe_error } => {
            assert_eq!(safe_error, "auth_refresh_parse_failed");
            assert!(!safe_error.contains("tok_access_live_should_never_parse"));
            assert!(!safe_error.contains("tok_refresh_live_should_never_parse"));
            assert!(!safe_error.contains("refresh_token="));
            assert!(!safe_error.contains("access_token="));
        }
        other => panic!("expected config required failure, got {other:?}"),
    }
}

#[test]
fn reauth_required_fixture_maps_through_safe_client() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope(AUTH_REFRESH_REAUTH_REQUIRED_JSON),
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ReauthFailure { safe_error } => {
            assert_eq!(safe_error, "invalid_grant");
        }
        other => panic!("expected reauth failure, got {other:?}"),
    }
}

#[test]
fn config_required_fixture_maps_to_config_required_outcome() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope(AUTH_REFRESH_CONFIG_REQUIRED_JSON),
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ConfigRequired { safe_error } => {
            assert_eq!(safe_error, "refresh_config_required");
        }
        other => panic!("expected config required failure, got {other:?}"),
    }
}

#[test]
fn transient_failure_fixture_maps_through_safe_client() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope(AUTH_REFRESH_TRANSIENT_FAILURE_JSON),
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::TransientFailure { safe_error } => {
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected transient failure, got {other:?}"),
    }
}

#[test]
fn safe_client_parse_error_maps_to_config_required_outcome() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::new(
        FakeTransport::from_envelope("not json"),
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ConfigRequired { safe_error } => {
            assert_eq!(safe_error, "auth_refresh_parse_failed");
        }
        other => panic!("expected config required failure, got {other:?}"),
    }
}

#[test]
fn safe_client_oversized_response_maps_to_config_required_outcome() {
    let mut adapter = LarkAuthRefreshAdapter::new(LarkAuthRefreshSafeClient::with_config(
        FakeTransport::from_envelope("x".repeat(257)),
        LarkAuthRefreshSafeClientConfig {
            max_response_bytes: 256,
        },
    ));
    let outcome = adapter.refresh(&sample_snapshot());

    match outcome {
        RefreshOutcome::ConfigRequired { safe_error } => {
            assert_eq!(safe_error, "auth_refresh_oversized_response");
        }
        other => panic!("expected config required failure, got {other:?}"),
    }
}

#[test]
fn parser_rejects_invalid_envelopes_without_leaking_payloads() {
    for raw in [
        r#"{"outcome":"unknown"}"#,
        r#"{"outcome":"success","encrypted_primary":[999]}"#,
        r#"{"outcome":"success","encrypted_primary":[1],"encrypted_renewal":[2]}"#,
        r#"{"outcome":"transient_failure","safe_error":"refresh_token=rt_live_should_not_pass"}"#,
        "not json",
    ] {
        let err = parse_lark_auth_refresh_response(raw)
            .expect_err("invalid auth refresh envelope should fail closed");
        let rendered = err.to_string();
        let debug = format!("{err:?}");

        assert_eq!(rendered, "invalid lark auth refresh envelope");
        assert!(!debug.contains(raw));
        assert!(!rendered.contains(raw));
    }
}

#[test]
fn oversized_output_is_rejected_safely() {
    let big_raw = "x".repeat(257);
    let mut safe_client = LarkAuthRefreshSafeClient::with_config(
        FakeTransport::from_envelope(big_raw),
        LarkAuthRefreshSafeClientConfig {
            max_response_bytes: 256,
        },
    );
    let err = safe_client
        .refresh(&LarkAuthRefreshRequest::from_snapshot(&sample_snapshot()))
        .expect_err("oversized payload should fail");
    assert_eq!(
        err,
        LarkAuthRefreshClientError::OversizedResponse {
            max_response_bytes: 256
        }
    );
    assert_eq!(err.classify(), "oversized_response");
    let debug = format!("{err:?}");
    let display = err.to_string();
    assert!(debug.contains("oversized_response"));
    assert!(!debug.contains(&"x".repeat(16)));
    assert!(!display.contains(&"x".repeat(16)));
}

#[test]
fn transport_error_debug_and_display_do_not_leak_raw_token_or_streams() {
    let mut safe_client = LarkAuthRefreshSafeClient::new(FakeTransport::from_error(
        "stdout=access_token=tok_live_sensitive stderr=refresh_token=rt_live_sensitive",
    ));
    let request = LarkAuthRefreshRequest::from_snapshot(&sample_snapshot());
    let err = safe_client
        .refresh(&request)
        .expect_err("transport error should map to safe error");
    assert_eq!(err, LarkAuthRefreshClientError::Transport);
    assert_eq!(err.classify(), "transport");
    let debug = format!("{err:?}");
    let display = err.to_string();
    assert!(!debug.contains("tok_live_sensitive"));
    assert!(!debug.contains("rt_live_sensitive"));
    assert!(!display.contains("tok_live_sensitive"));
    assert!(!display.contains("rt_live_sensitive"));
    assert!(!debug.contains("stdout="));
    assert!(!debug.contains("stderr="));
    assert!(!display.contains("stdout="));
    assert!(!display.contains("stderr="));
}

#[derive(Debug, Clone)]
struct FakeTransport {
    envelope: Option<LarkAuthRefreshRawEnvelope>,
    error: Option<FakeTransportError>,
}

impl FakeTransport {
    fn from_envelope(payload: impl Into<String>) -> Self {
        Self {
            envelope: Some(LarkAuthRefreshRawEnvelope::new(payload)),
            error: None,
        }
    }

    fn from_error(error: impl Into<String>) -> Self {
        Self {
            envelope: None,
            error: Some(FakeTransportError { raw: error.into() }),
        }
    }
}

impl LarkAuthRefreshTransport for FakeTransport {
    type Error = FakeTransportError;

    fn execute(
        &mut self,
        _request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshRawEnvelope, Self::Error> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.envelope.clone().ok_or_else(|| FakeTransportError {
            raw: "missing fake envelope".to_string(),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct FakeTransportError {
    raw: String,
}

impl std::fmt::Debug for FakeTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeTransportError")
            .field("raw", &self.raw)
            .finish()
    }
}

impl std::fmt::Display for FakeTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
}
