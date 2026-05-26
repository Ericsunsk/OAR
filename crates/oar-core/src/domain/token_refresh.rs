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

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedGrantBlob(pub Vec<u8>);

impl fmt::Debug for EncryptedGrantBlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EncryptedGrantBlob")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl EncryptedGrantMaterial {
    pub fn into_blob(self) -> EncryptedGrantBlob {
        let primary_len = self.encrypted_primary.len() as u32;
        let renewal_len = self.encrypted_renewal.len() as u32;
        let mut out = Vec::with_capacity(8 + primary_len as usize + renewal_len as usize);
        out.extend_from_slice(&primary_len.to_be_bytes());
        out.extend_from_slice(&self.encrypted_primary);
        out.extend_from_slice(&renewal_len.to_be_bytes());
        out.extend_from_slice(&self.encrypted_renewal);
        EncryptedGrantBlob(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRefreshRepositoryCommand {
    RotateGrantCas {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        expires_at_ms: Option<u64>,
        refreshed_at_ms: u64,
        encrypted_grant_blob: EncryptedGrantBlob,
        grant_key_id: String,
        new_fingerprint: String,
    },
    MarkNeedsRefresh {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        refreshed_at_ms: u64,
        safe_error: String,
    },
    MarkReauthRequired {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        reauth_required_at_ms: u64,
        safe_error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRefreshBridgeError {
    TimestampBeforeUnixEpoch,
}

fn system_time_to_ms(value: SystemTime) -> Result<u64, TokenRefreshBridgeError> {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| TokenRefreshBridgeError::TimestampBeforeUnixEpoch)
}

impl TokenRefreshDecision {
    pub fn into_repository_command_at(
        self,
        now: SystemTime,
    ) -> Result<TokenRefreshRepositoryCommand, TokenRefreshBridgeError> {
        match self {
            TokenRefreshDecision::RotateGrantCas {
                grant_id,
                tenant_id,
                expected_fingerprint,
                rotated_material,
                key_id,
                new_fingerprint,
                refreshed_at,
                expires_at,
            } => Ok(TokenRefreshRepositoryCommand::RotateGrantCas {
                grant_id,
                tenant_id,
                expected_fingerprint,
                expires_at_ms: expires_at.map(system_time_to_ms).transpose()?,
                refreshed_at_ms: system_time_to_ms(refreshed_at)?,
                encrypted_grant_blob: rotated_material.into_blob(),
                grant_key_id: key_id,
                new_fingerprint,
            }),
            TokenRefreshDecision::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                expected_fingerprint,
                safe_error,
            } => Ok(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                expected_fingerprint,
                refreshed_at_ms: system_time_to_ms(now)?,
                safe_error,
            }),
            TokenRefreshDecision::MarkReauthRequired {
                grant_id,
                tenant_id,
                expected_fingerprint,
                safe_error,
            } => Ok(TokenRefreshRepositoryCommand::MarkReauthRequired {
                grant_id,
                tenant_id,
                expected_fingerprint,
                reauth_required_at_ms: system_time_to_ms(now)?,
                safe_error,
            }),
        }
    }
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
