use std::fmt;

use crate::domain::identity::{TenantId, TokenGrantId};

use super::material::EncryptedGrantMaterial;
use super::outcome::RefreshOutcome;

#[derive(Clone, PartialEq, Eq)]
pub struct TokenRefreshAttempt {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub expected_fingerprint: String,
    pub outcome: RefreshOutcome,
}

impl fmt::Debug for TokenRefreshAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenRefreshAttempt")
            .field("grant_id", &self.grant_id)
            .field("tenant_id", &self.tenant_id)
            .field("expected_fingerprint", &"[REDACTED]")
            .field("outcome", &self.outcome)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum TokenRefreshDecision {
    RotateGrantCas {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        rotated_material: EncryptedGrantMaterial,
        key_id: String,
        new_fingerprint: String,
        refreshed_at: std::time::SystemTime,
        expires_at: Option<std::time::SystemTime>,
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
    MarkConfigRequired {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        safe_error: String,
    },
}

impl fmt::Debug for TokenRefreshDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RotateGrantCas {
                grant_id,
                tenant_id,
                rotated_material,
                refreshed_at,
                expires_at,
                ..
            } => f
                .debug_struct("RotateGrantCas")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("rotated_material", rotated_material)
                .field("key_id", &"[REDACTED]")
                .field("new_fingerprint", &"[REDACTED]")
                .field("refreshed_at", refreshed_at)
                .field("expires_at", expires_at)
                .finish(),
            Self::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                safe_error,
                ..
            } => f
                .debug_struct("MarkNeedsRefresh")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("safe_error", safe_error)
                .finish(),
            Self::MarkReauthRequired {
                grant_id,
                tenant_id,
                safe_error,
                ..
            } => f
                .debug_struct("MarkReauthRequired")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("safe_error", safe_error)
                .finish(),
            Self::MarkConfigRequired {
                grant_id,
                tenant_id,
                safe_error,
                ..
            } => f
                .debug_struct("MarkConfigRequired")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("safe_error", safe_error)
                .finish(),
        }
    }
}
