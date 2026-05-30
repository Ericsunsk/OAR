use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedTokenGrantRecord {
    pub id: String,
    pub tenant_id: String,
    pub identity_id: String,
    pub actor_kind: ActorKind,
    pub scope_boundary: ScopeBoundary,
    pub scopes: Vec<String>,
    pub state: TokenGrantState,
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
    pub reauth_required_at_ms: Option<u64>,
    pub last_refresh_error: Option<String>,
    pub encrypted_oauth_grant: Vec<u8>,
    pub oauth_grant_key_id: String,
    pub oauth_grant_fingerprint: String,
    pub revocation_reason: Option<String>,
}

impl fmt::Debug for EncryptedTokenGrantRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedTokenGrantRecord")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("identity_id", &self.identity_id)
            .field("actor_kind", &self.actor_kind)
            .field("scope_boundary", &self.scope_boundary)
            .field("scopes", &self.scopes)
            .field("state", &self.state)
            .field("issued_at_ms", &self.issued_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field("revoked_at_ms", &self.revoked_at_ms)
            .field("reauth_required_at_ms", &self.reauth_required_at_ms)
            .field("last_refresh_error", &self.last_refresh_error)
            .field(
                "encrypted_oauth_grant",
                &format_args!("[REDACTED; bytes={}]", self.encrypted_oauth_grant.len()),
            )
            .field("oauth_grant_key_id", &"[REDACTED]")
            .field("oauth_grant_fingerprint", &"[REDACTED]")
            .field("revocation_reason", &self.revocation_reason)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RotateEncryptedGrantRequest<'a> {
    pub tenant_id: &'a str,
    pub id: &'a str,
    pub expected_fingerprint: &'a str,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: u64,
    pub encrypted_oauth_grant: &'a [u8],
    pub oauth_grant_key_id: &'a str,
    pub oauth_grant_fingerprint: &'a str,
}

impl fmt::Debug for RotateEncryptedGrantRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RotateEncryptedGrantRequest")
            .field("tenant_id", &self.tenant_id)
            .field("id", &self.id)
            .field("expected_fingerprint", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field(
                "encrypted_oauth_grant",
                &format_args!("[REDACTED; bytes={}]", self.encrypted_oauth_grant.len()),
            )
            .field("oauth_grant_key_id", &"[REDACTED]")
            .field("oauth_grant_fingerprint", &"[REDACTED]")
            .finish()
    }
}
