use std::fmt;
use std::time::SystemTime;

use super::identity::{TenantId, TokenGrant, TokenGrantId, TokenGrantState};

const REDACTED_REFRESH_ERROR: &str = "<redacted refresh error>";

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
            safe_error: sanitize_refresh_error_for_report(&safe_error),
        },
        RefreshOutcome::ReauthFailure { safe_error } => TokenRefreshDecision::MarkReauthRequired {
            grant_id: attempt.grant_id,
            tenant_id: attempt.tenant_id,
            expected_fingerprint: attempt.expected_fingerprint,
            safe_error: sanitize_refresh_error_for_report(&safe_error),
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

pub trait AuthRefreshAdapter {
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome;
}

pub trait TokenRefreshCommandSink {
    type Error;

    fn apply_refresh_command(
        &mut self,
        command: TokenRefreshRepositoryCommand,
    ) -> Result<Option<TokenRefreshApplyResult>, Self::Error>;
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
pub enum TokenRefreshServiceError<E> {
    DecisionBridge(TokenRefreshBridgeError),
    CommandSink(E),
}

impl<E> From<TokenRefreshBridgeError> for TokenRefreshServiceError<E> {
    fn from(value: TokenRefreshBridgeError) -> Self {
        Self::DecisionBridge(value)
    }
}

pub struct TokenRefreshService<A, S>
where
    A: AuthRefreshAdapter,
    S: TokenRefreshCommandSink,
{
    adapter: A,
    sink: S,
}

impl<A, S> TokenRefreshService<A, S>
where
    A: AuthRefreshAdapter,
    S: TokenRefreshCommandSink,
{
    pub fn new(adapter: A, sink: S) -> Self {
        Self { adapter, sink }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn refresh_grant_at(
        &mut self,
        snapshot: TokenRefreshGrantSnapshot,
        now: SystemTime,
    ) -> Result<TokenRefreshServiceReport, TokenRefreshServiceError<S::Error>> {
        if let Some(reason) = snapshot.short_circuit_reason() {
            return Ok(TokenRefreshServiceReport {
                grant_id: snapshot.grant_id,
                tenant_id: snapshot.tenant_id,
                status: TokenRefreshReportStatus::ShortCircuited(reason),
                adapter_called: false,
                sink_called: false,
                decision: None,
                command: None,
                safe_error: None,
            });
        }

        let outcome = self.adapter.refresh(&snapshot);
        let attempt = TokenRefreshAttempt {
            grant_id: snapshot.grant_id.clone(),
            tenant_id: snapshot.tenant_id.clone(),
            expected_fingerprint: snapshot.expected_fingerprint.clone(),
            outcome,
        };
        let decision = decide_token_refresh(attempt);
        let decision_kind = decision.kind();
        let safe_error = decision.safe_error().map(ToOwned::to_owned);
        let command = decision.into_repository_command_at(now)?;
        let command_kind = command.kind();
        let apply_result = self
            .sink
            .apply_refresh_command(command)
            .map_err(TokenRefreshServiceError::CommandSink)?;

        let status = if apply_result.is_some() {
            TokenRefreshReportStatus::Succeeded
        } else {
            TokenRefreshReportStatus::ConflictNoop
        };

        Ok(TokenRefreshServiceReport {
            grant_id: snapshot.grant_id,
            tenant_id: snapshot.tenant_id,
            status,
            adapter_called: true,
            sink_called: true,
            decision: Some(decision_kind),
            command: Some(command_kind),
            safe_error,
        })
    }
}

impl TokenRefreshDecision {
    pub fn kind(&self) -> TokenRefreshDecisionKind {
        match self {
            TokenRefreshDecision::RotateGrantCas { .. } => TokenRefreshDecisionKind::RotateGrantCas,
            TokenRefreshDecision::MarkNeedsRefresh { .. } => {
                TokenRefreshDecisionKind::MarkNeedsRefresh
            }
            TokenRefreshDecision::MarkReauthRequired { .. } => {
                TokenRefreshDecisionKind::MarkReauthRequired
            }
        }
    }

    pub fn safe_error(&self) -> Option<&str> {
        match self {
            TokenRefreshDecision::MarkNeedsRefresh { safe_error, .. }
            | TokenRefreshDecision::MarkReauthRequired { safe_error, .. } => Some(safe_error),
            TokenRefreshDecision::RotateGrantCas { .. } => None,
        }
    }
}

fn sanitize_refresh_error_for_report(reason: &str) -> String {
    match reason.trim() {
        "invalid_grant" => "invalid_grant".to_string(),
        "temporarily unavailable" => "temporarily unavailable".to_string(),
        _ => REDACTED_REFRESH_ERROR.to_string(),
    }
}

impl TokenRefreshRepositoryCommand {
    pub fn kind(&self) -> TokenRefreshCommandKind {
        match self {
            TokenRefreshRepositoryCommand::RotateGrantCas { .. } => {
                TokenRefreshCommandKind::RotateGrantCas
            }
            TokenRefreshRepositoryCommand::MarkNeedsRefresh { .. } => {
                TokenRefreshCommandKind::MarkNeedsRefresh
            }
            TokenRefreshRepositoryCommand::MarkReauthRequired { .. } => {
                TokenRefreshCommandKind::MarkReauthRequired
            }
        }
    }
}
