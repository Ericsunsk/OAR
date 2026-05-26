use std::fmt;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OarUserId(pub String);

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
pub struct OarUser {
    pub id: OarUserId,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub status: OarUserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarUserStatus {
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

#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthTokens {
    pub access_token: SecretString,
    pub refresh_token: Option<SecretString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenGrantState {
    Valid,
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
    pub tokens: OAuthTokens,
    pub revocation_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    MacDesktop,
    IosCompanion,
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCursor {
    pub stream: String,
    pub cursor: String,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSession {
    pub id: DeviceSessionId,
    pub tenant_id: TenantId,
    pub user_id: OarUserId,
    pub device_type: DeviceType,
    pub device_label: String,
    pub session_identity: String,
    pub sync_cursor: SyncCursor,
    pub last_seen_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
}
