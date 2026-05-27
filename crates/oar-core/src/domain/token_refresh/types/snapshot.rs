use std::time::SystemTime;

use crate::domain::identity::{TenantId, TokenGrant, TokenGrantId, TokenGrantState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshGrantSnapshot {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub expected_fingerprint: String,
    pub state: TokenGrantState,
    pub has_refresh_material: bool,
    pub revoked_at: Option<SystemTime>,
    pub reauth_required_at: Option<SystemTime>,
}

impl TokenRefreshGrantSnapshot {
    pub fn from_grant(grant: &TokenGrant, expected_fingerprint: impl Into<String>) -> Self {
        Self {
            grant_id: grant.id.clone(),
            tenant_id: grant.tenant_id.clone(),
            expected_fingerprint: expected_fingerprint.into(),
            state: grant.state,
            has_refresh_material: grant.tokens.refresh_token.is_some(),
            revoked_at: grant.revoked_at,
            reauth_required_at: grant.reauth_required_at,
        }
    }

    pub fn short_circuit_reason(&self) -> Option<TokenRefreshShortCircuitReason> {
        if self.state == TokenGrantState::Revoked || self.revoked_at.is_some() {
            return Some(TokenRefreshShortCircuitReason::Revoked);
        }
        if self.state == TokenGrantState::ReauthRequired || self.reauth_required_at.is_some() {
            return Some(TokenRefreshShortCircuitReason::ReauthRequired);
        }
        if !self.has_refresh_material {
            return Some(TokenRefreshShortCircuitReason::MissingRefreshMaterial);
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshApplyResult {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub state: TokenGrantState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshShortCircuitReason {
    Revoked,
    ReauthRequired,
    MissingRefreshMaterial,
}
