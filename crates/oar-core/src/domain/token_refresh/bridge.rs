use std::fmt;
use std::time::SystemTime;

use super::decision::decide_token_refresh;
use super::types::{
    RefreshOutcome, TokenRefreshAttempt, TokenRefreshCommandReport, TokenRefreshDecision,
    TokenRefreshGrantSnapshot, TokenRefreshPlannedCommand, TokenRefreshRepositoryCommand,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRefreshBridgeError {
    TimestampBeforeUnixEpoch,
}

impl fmt::Display for TokenRefreshBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenRefreshBridgeError::TimestampBeforeUnixEpoch => {
                write!(f, "token refresh timestamp is before the Unix epoch")
            }
        }
    }
}

impl std::error::Error for TokenRefreshBridgeError {}

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

pub fn plan_token_refresh_command(
    snapshot: &TokenRefreshGrantSnapshot,
    outcome: RefreshOutcome,
    now: SystemTime,
) -> Result<TokenRefreshPlannedCommand, TokenRefreshBridgeError> {
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

    Ok(TokenRefreshPlannedCommand {
        command,
        report: TokenRefreshCommandReport {
            grant_id: snapshot.grant_id.clone(),
            tenant_id: snapshot.tenant_id.clone(),
            decision_kind,
            command_kind,
            safe_error,
        },
    })
}
