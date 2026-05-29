use std::fmt;

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOAuthLoginToken {
    pub access_token: SecretString,
    pub refresh_token: Option<SecretString>,
    pub expires_in_seconds: u64,
    pub refresh_token_expires_in_seconds: Option<u64>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuOAuthLoginToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLoginToken")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field(
                "refresh_token_expires_in_seconds",
                &self.refresh_token_expires_in_seconds,
            )
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeishuOAuthLoginUser {
    pub open_id: String,
    pub union_id: Option<String>,
    pub tenant_key: Option<String>,
    pub display_name: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOAuthLogin {
    pub token: FeishuOAuthLoginToken,
    pub user: FeishuOAuthLoginUser,
}

impl fmt::Debug for FeishuOAuthLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLogin")
            .field("token", &self.token)
            .field("user", &self.user)
            .finish()
    }
}
