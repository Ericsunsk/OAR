use crate::domain::identity::{TenantId, TokenGrantId};

use super::repository_command::TokenRefreshRepositoryCommand;
use super::snapshot::TokenRefreshShortCircuitReason;
use crate::domain::token_refresh::sanitize::sanitize_refresh_error_for_report;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshDecisionKind {
    RotateGrantCas,
    MarkNeedsRefresh,
    MarkReauthRequired,
    MarkConfigRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshCommandKind {
    RotateGrantCas,
    MarkNeedsRefresh,
    MarkReauthRequired,
    MarkConfigRequired,
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
            safe_error: self
                .safe_error
                .map(|value| sanitize_refresh_error_for_report(&value)),
        }
    }
}

impl TokenRefreshPlannedCommand {
    pub fn grant_id(&self) -> &TokenGrantId {
        match &self.command {
            TokenRefreshRepositoryCommand::RotateGrantCas { grant_id, .. }
            | TokenRefreshRepositoryCommand::MarkNeedsRefresh { grant_id, .. }
            | TokenRefreshRepositoryCommand::MarkReauthRequired { grant_id, .. }
            | TokenRefreshRepositoryCommand::MarkConfigRequired { grant_id, .. } => grant_id,
        }
    }

    pub fn tenant_id(&self) -> &TenantId {
        match &self.command {
            TokenRefreshRepositoryCommand::RotateGrantCas { tenant_id, .. }
            | TokenRefreshRepositoryCommand::MarkNeedsRefresh { tenant_id, .. }
            | TokenRefreshRepositoryCommand::MarkReauthRequired { tenant_id, .. }
            | TokenRefreshRepositoryCommand::MarkConfigRequired { tenant_id, .. } => tenant_id,
        }
    }
}
