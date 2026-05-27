use secrecy::ExposeSecret;
use std::fmt;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceUserId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LarkIdentityId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenGrantId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceSessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    pub id: TenantId,
    pub display_name: String,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantStatus {
    Active,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUser {
    pub id: WorkspaceUserId,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub status: WorkspaceUserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceUserStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    User,
    Bot,
    App,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LarkIdentity {
    pub id: LarkIdentityId,
    pub tenant_id: TenantId,
    pub actor_kind: ActorKind,
    pub actor_external_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeBoundary {
    Tenant,
    User,
    Admin,
    Bot,
    Service,
}

#[derive(Clone)]
pub struct SecretString(secrecy::SecretString);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(secrecy::SecretString::new(value.into().into_boxed_str()))
    }

    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        self.expose_secret() == other.expose_secret()
    }
}

impl Eq for SecretString {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthTokens {
    pub access_token: SecretString,
    pub refresh_token: Option<SecretString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenGrantState {
    Valid,
    NeedsRefresh,
    Expired,
    Revoked,
    ReauthRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenGrant {
    pub id: TokenGrantId,
    pub tenant_id: TenantId,
    pub identity_id: LarkIdentityId,
    pub actor_kind: ActorKind,
    pub scope_boundary: ScopeBoundary,
    pub scopes: Vec<String>,
    pub state: TokenGrantState,
    pub issued_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub refreshed_at: Option<SystemTime>,
    pub revoked_at: Option<SystemTime>,
    pub reauth_required_at: Option<SystemTime>,
    pub last_refresh_error: Option<String>,
    pub tokens: OAuthTokens,
    pub revocation_reason: Option<String>,
}
