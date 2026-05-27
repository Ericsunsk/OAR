use std::fmt;

use crate::domain::token_refresh::types::TokenRefreshGrantSnapshot;

use super::safety::{
    sanitize_safe_error, SAFE_CONFIG_ERROR, SAFE_PARSE_ERROR, SAFE_REAUTH_ERROR,
    SAFE_TRANSIENT_ERROR,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LarkAuthGrantState {
    Valid,
    Expired,
    NeedsRefresh,
    Revoked,
    ReauthRequired,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuAuthRefreshRequest {
    pub grant_id: String,
    pub tenant_id: String,
    pub expected_fingerprint: String,
    pub grant_state: LarkAuthGrantState,
    pub has_refresh_material: bool,
    pub is_revoked: bool,
    pub reauth_marked: bool,
}

impl fmt::Debug for FeishuAuthRefreshRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAuthRefreshRequest")
            .field("grant_id", &self.grant_id)
            .field("tenant_id", &self.tenant_id)
            .field("expected_fingerprint", &"[REDACTED]")
            .field("grant_state", &self.grant_state)
            .field("has_refresh_material", &self.has_refresh_material)
            .field("is_revoked", &self.is_revoked)
            .field("reauth_marked", &self.reauth_marked)
            .finish()
    }
}

impl FeishuAuthRefreshRequest {
    pub fn from_snapshot(snapshot: &TokenRefreshGrantSnapshot) -> Self {
        Self {
            grant_id: snapshot.grant_id.0.clone(),
            tenant_id: snapshot.tenant_id.0.clone(),
            expected_fingerprint: snapshot.expected_fingerprint.clone(),
            grant_state: map_grant_state(snapshot.state),
            has_refresh_material: snapshot.has_refresh_material,
            is_revoked: snapshot.revoked_at.is_some(),
            reauth_marked: snapshot.reauth_required_at.is_some(),
        }
    }
}

impl From<&TokenRefreshGrantSnapshot> for FeishuAuthRefreshRequest {
    fn from(value: &TokenRefreshGrantSnapshot) -> Self {
        Self::from_snapshot(value)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuAuthRefreshSuccess {
    pub encrypted_primary: Vec<u8>,
    pub encrypted_renewal: Vec<u8>,
    pub key_id: String,
    pub new_fingerprint: String,
    pub refreshed_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

impl fmt::Debug for FeishuAuthRefreshSuccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAuthRefreshSuccess")
            .field("encrypted_primary", &"[REDACTED]")
            .field("encrypted_renewal", &"[REDACTED]")
            .field("key_id", &"[REDACTED]")
            .field("new_fingerprint", &"[REDACTED]")
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuAuthRefreshFailure {
    Transient { safe_error: String },
    ReauthRequired { safe_error: String },
    ConfigRequired { safe_error: String },
}

impl fmt::Debug for FeishuAuthRefreshFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshFailure::Transient { safe_error } => f
                .debug_struct("Transient")
                .field(
                    "safe_error",
                    &sanitize_safe_error(safe_error, SAFE_TRANSIENT_ERROR),
                )
                .finish(),
            FeishuAuthRefreshFailure::ReauthRequired { safe_error } => f
                .debug_struct("ReauthRequired")
                .field(
                    "safe_error",
                    &sanitize_safe_error(safe_error, SAFE_REAUTH_ERROR),
                )
                .finish(),
            FeishuAuthRefreshFailure::ConfigRequired { safe_error } => f
                .debug_struct("ConfigRequired")
                .field(
                    "safe_error",
                    &sanitize_safe_error(safe_error, SAFE_CONFIG_ERROR),
                )
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuAuthRefreshResponse {
    Success(FeishuAuthRefreshSuccess),
    Failure(FeishuAuthRefreshFailure),
}

impl fmt::Debug for FeishuAuthRefreshResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshResponse::Success(success) => {
                f.debug_tuple("Success").field(success).finish()
            }
            FeishuAuthRefreshResponse::Failure(failure) => {
                f.debug_tuple("Failure").field(failure).finish()
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuAuthRefreshParseError {
    InvalidEnvelope,
    SensitiveContentDetected,
}

impl fmt::Debug for FeishuAuthRefreshParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshParseError::InvalidEnvelope => {
                write!(f, "FeishuAuthRefreshParseError(InvalidEnvelope)")
            }
            FeishuAuthRefreshParseError::SensitiveContentDetected => {
                write!(f, "FeishuAuthRefreshParseError(SensitiveContentDetected)")
            }
        }
    }
}

impl fmt::Display for FeishuAuthRefreshParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshParseError::InvalidEnvelope => write!(f, "{}", SAFE_PARSE_ERROR),
            FeishuAuthRefreshParseError::SensitiveContentDetected => {
                write!(f, "{}", SAFE_PARSE_ERROR)
            }
        }
    }
}

impl std::error::Error for FeishuAuthRefreshParseError {}

fn map_grant_state(state: crate::domain::identity::TokenGrantState) -> LarkAuthGrantState {
    match state {
        crate::domain::identity::TokenGrantState::Valid => LarkAuthGrantState::Valid,
        crate::domain::identity::TokenGrantState::Expired => LarkAuthGrantState::Expired,
        crate::domain::identity::TokenGrantState::NeedsRefresh => LarkAuthGrantState::NeedsRefresh,
        crate::domain::identity::TokenGrantState::Revoked => LarkAuthGrantState::Revoked,
        crate::domain::identity::TokenGrantState::ReauthRequired => {
            LarkAuthGrantState::ReauthRequired
        }
    }
}
