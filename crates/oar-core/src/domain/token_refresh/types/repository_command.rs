use std::fmt;

use crate::domain::identity::{TenantId, TokenGrantId};

use super::material::EncryptedGrantBlob;

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
    MarkConfigRequired {
        grant_id: TokenGrantId,
        tenant_id: TenantId,
        expected_fingerprint: String,
        refreshed_at_ms: u64,
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
            Self::MarkConfigRequired {
                grant_id,
                tenant_id,
                refreshed_at_ms,
                safe_error,
                ..
            } => f
                .debug_struct("MarkConfigRequired")
                .field("grant_id", grant_id)
                .field("tenant_id", tenant_id)
                .field("expected_fingerprint", &"[REDACTED]")
                .field("refreshed_at_ms", refreshed_at_ms)
                .field("safe_error", safe_error)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::token_refresh::types::EncryptedGrantBlob;

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
