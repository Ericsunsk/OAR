use crate::domain::identity::{TokenGrant, TokenGrantState};

use super::sanitize::sanitize_refresh_error_for_report;
use super::types::{
    RefreshOutcome, TokenRefreshAttempt, TokenRefreshCommandKind, TokenRefreshDecision,
    TokenRefreshDecisionKind, TokenRefreshRepositoryCommand,
};

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
