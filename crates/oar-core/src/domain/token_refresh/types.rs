use std::fmt;
use std::time::SystemTime;

use crate::domain::identity::{TenantId, TokenGrant, TokenGrantId, TokenGrantState};

use super::sanitize::sanitize_refresh_error_for_report;

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

impl fmt::Debug for RefreshOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success {
                rotated_material,
                refreshed_at,
                expires_at,
                ..
            } => f
                .debug_struct("Success")
                .field("rotated_material", rotated_material)
                .field("key_id", &"[REDACTED]")
                .field("new_fingerprint", &"[REDACTED]")
                .field("refreshed_at", refreshed_at)
                .field("expires_at", expires_at)
                .finish(),
            Self::TransientFailure { safe_error } => f
                .debug_struct("TransientFailure")
                .field("safe_error", safe_error)
                .finish(),
            Self::ReauthFailure { safe_error } => f
                .debug_struct("ReauthFailure")
                .field("safe_error", safe_error)
                .finish(),
        }
    }
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

#[derive(Clone, PartialEq, Eq)]
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
        }
    }
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

#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for TokenRefreshRepositoryCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RotateGrantCas {
                grant_id,
                tenant_id,
                expires_at_ms,
                refreshed_at_ms,
                encrypted_grant_blob,
                ..
            } => f
                .debug_struct("RotateGrantCas")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("expires_at_ms", expires_at_ms)
                .field("refreshed_at_ms", refreshed_at_ms)
                .field("encrypted_grant_blob", encrypted_grant_blob)
                .field("grant_key_id", &"[REDACTED]")
                .field("new_fingerprint", &"[REDACTED]")
                .finish(),
            Self::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                refreshed_at_ms,
                safe_error,
                ..
            } => f
                .debug_struct("MarkNeedsRefresh")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("refreshed_at_ms", refreshed_at_ms)
                .field("safe_error", safe_error)
                .finish(),
            Self::MarkReauthRequired {
                grant_id,
                tenant_id,
                reauth_required_at_ms,
                safe_error,
                ..
            } => f
                .debug_struct("MarkReauthRequired")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("reauth_required_at_ms", reauth_required_at_ms)
                .field("safe_error", safe_error)
                .finish(),
        }
    }
}

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
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshShortCircuitReason {
    Revoked,
    ReauthRequired,
    MissingRefreshMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshDecisionKind {
    RotateGrantCas,
    MarkNeedsRefresh,
    MarkReauthRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshCommandKind {
    RotateGrantCas,
    MarkNeedsRefresh,
    MarkReauthRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRefreshReportStatus {
    Succeeded,
    ConflictNoop,
    ShortCircuited(TokenRefreshShortCircuitReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshServiceReport {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub status: TokenRefreshReportStatus,
    pub adapter_called: bool,
    pub sink_called: bool,
    pub decision: Option<TokenRefreshDecisionKind>,
    pub command: Option<TokenRefreshCommandKind>,
    pub safe_error: Option<String>,
}

impl TokenRefreshServiceReport {
    pub fn audit_summary(&self) -> TokenRefreshAuditSummary {
        TokenRefreshAuditSummary {
            grant_id: self.grant_id.clone(),
            tenant_id: self.tenant_id.clone(),
            status: self.status.clone(),
            decision: self.decision,
            command: self.command,
            safe_error: self.safe_error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshAuditSummary {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub status: TokenRefreshReportStatus,
    pub decision: Option<TokenRefreshDecisionKind>,
    pub command: Option<TokenRefreshCommandKind>,
    pub safe_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshPlannedCommand {
    pub command: TokenRefreshRepositoryCommand,
    pub report: TokenRefreshCommandReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshCommandReport {
    pub grant_id: TokenGrantId,
    pub tenant_id: TenantId,
    pub decision_kind: TokenRefreshDecisionKind,
    pub command_kind: TokenRefreshCommandKind,
    pub safe_error: Option<String>,
}

impl TokenRefreshCommandReport {
    pub fn audit_summary(&self, status: TokenRefreshReportStatus) -> TokenRefreshAuditSummary {
        TokenRefreshAuditSummary {
            grant_id: self.grant_id.clone(),
            tenant_id: self.tenant_id.clone(),
            status,
            decision: Some(self.decision_kind),
            command: Some(self.command_kind),
            safe_error: self
                .safe_error
                .as_deref()
                .map(sanitize_refresh_error_for_report),
        }
    }

    pub fn into_service_report(self, applied: bool) -> TokenRefreshServiceReport {
        TokenRefreshServiceReport {
            grant_id: self.grant_id,
            tenant_id: self.tenant_id,
            status: if applied {
                TokenRefreshReportStatus::Succeeded
            } else {
                TokenRefreshReportStatus::ConflictNoop
            },
            adapter_called: true,
            sink_called: true,
            decision: Some(self.decision_kind),
            command: Some(self.command_kind),
            safe_error: self.safe_error,
        }
    }
}

impl TokenRefreshPlannedCommand {
    pub fn grant_id(&self) -> &TokenGrantId {
        match &self.command {
            TokenRefreshRepositoryCommand::RotateGrantCas { grant_id, .. }
            | TokenRefreshRepositoryCommand::MarkNeedsRefresh { grant_id, .. }
            | TokenRefreshRepositoryCommand::MarkReauthRequired { grant_id, .. } => grant_id,
        }
    }

    pub fn tenant_id(&self) -> &TenantId {
        match &self.command {
            TokenRefreshRepositoryCommand::RotateGrantCas { tenant_id, .. }
            | TokenRefreshRepositoryCommand::MarkNeedsRefresh { tenant_id, .. }
            | TokenRefreshRepositoryCommand::MarkReauthRequired { tenant_id, .. } => tenant_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_refresh_repository_command_debug_redacts_sensitive_fields() {
        let command = TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id: TokenGrantId("grant_1".to_string()),
            tenant_id: TenantId("tenant_1".to_string()),
            expected_fingerprint: "expected_fp_sensitive".to_string(),
            expires_at_ms: Some(123),
            refreshed_at_ms: 456,
            encrypted_grant_blob: EncryptedGrantBlob(vec![1, 2, 3]),
            grant_key_id: "key_sensitive".to_string(),
            new_fingerprint: "new_fp_sensitive".to_string(),
        };

        let debug = format!("{command:?}");
        assert!(!debug.contains("expected_fp_sensitive"));
        assert!(!debug.contains("key_sensitive"));
        assert!(!debug.contains("new_fp_sensitive"));
        assert!(!debug.contains("[1, 2, 3]"));
        assert!(debug.contains("[REDACTED]"));
    }
}
