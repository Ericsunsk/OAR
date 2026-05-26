use std::fmt;
use std::time::SystemTime;

use super::identity::{TenantId, TokenGrant, TokenGrantId, TokenGrantState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshAttempt {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub expected_fingerprint: String,
    pub outcome: RefreshOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    Success {
        rotated_material: EncryptedGrantMaterial,
        key_id: String,
        new_fingerprint: String,
        refreshed_at: SystemTime,
        expires_at: Option<SystemTime>,
    },
    TransientFailure {
        safe_error: String,
    },
    ReauthFailure {
        safe_error: String,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedGrantMaterial {
    pub encrypted_primary: Vec<u8>,
    pub encrypted_renewal: Vec<u8>,
}

impl fmt::Debug for EncryptedGrantMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedGrantMaterial")
            .field("encrypted_primary", &"[REDACTED]")
            .field("encrypted_renewal", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRefreshDecision {
    RotateGrantCas {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        rotated_material: EncryptedGrantMaterial,
        key_id: String,
        new_fingerprint: String,
        refreshed_at: SystemTime,
        expires_at: Option<SystemTime>,
    },
    MarkNeedsRefresh {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        safe_error: String,
    },
    MarkReauthRequired {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        safe_error: String,
    },
}

pub fn decide_token_refresh(attempt: TokenRefreshAttempt) -> TokenRefreshDecision {
    match attempt.outcome {
        RefreshOutcome::Success {
            rotated_material,
            key_id,
            new_fingerprint,
            refreshed_at,
            expires_at,
        } => TokenRefreshDecision::RotateGrantCas {
            grant_id: attempt.grant_id,
            tenant_id: attempt.tenant_id,
            expected_fingerprint: attempt.expected_fingerprint,
            rotated_material,
            key_id,
            new_fingerprint,
            refreshed_at,
            expires_at,
        },
        RefreshOutcome::TransientFailure { safe_error } => TokenRefreshDecision::MarkNeedsRefresh {
            grant_id: attempt.grant_id,
            tenant_id: attempt.tenant_id,
            expected_fingerprint: attempt.expected_fingerprint,
            safe_error,
        },
        RefreshOutcome::ReauthFailure { safe_error } => TokenRefreshDecision::MarkReauthRequired {
            grant_id: attempt.grant_id,
            tenant_id: attempt.tenant_id,
            expected_fingerprint: attempt.expected_fingerprint,
            safe_error,
        },
    }
}

pub fn is_refreshable(grant: &TokenGrant) -> bool {
    matches!(
        grant.state,
        TokenGrantState::Valid | TokenGrantState::Expired | TokenGrantState::NeedsRefresh
    ) && grant.tokens.refresh_token.is_some()
        && grant.revoked_at.is_none()
        && grant.reauth_required_at.is_none()
}
