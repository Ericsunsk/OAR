use std::time::SystemTime;

use super::identity::{OAuthTokens, SecretString, TokenGrant, TokenGrantState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenGrantError {
    InvalidTransition {
        from: TokenGrantState,
        event: &'static str,
    },
    MissingRefreshToken,
    RevokedGrant,
    ReauthRequired,
}

impl TokenGrant {
    pub fn mark_needs_refresh(mut self) -> Result<Self, TokenGrantError> {
        match self.state {
            TokenGrantState::Valid | TokenGrantState::Expired | TokenGrantState::NeedsRefresh => {
                self.state = TokenGrantState::NeedsRefresh;
                Ok(self)
            }
            TokenGrantState::Revoked => Err(TokenGrantError::RevokedGrant),
            TokenGrantState::ReauthRequired => Err(TokenGrantError::ReauthRequired),
        }
    }

    pub fn refresh_succeeded_with_rotation(
        mut self,
        now: SystemTime,
        new_access_token: SecretString,
        new_refresh_token: SecretString,
        new_expires_at: Option<SystemTime>,
    ) -> Result<Self, TokenGrantError> {
        self.ensure_refreshable()?;

        self.tokens = OAuthTokens {
            access_token: new_access_token,
            refresh_token: Some(new_refresh_token),
        };
        self.state = TokenGrantState::Valid;
        self.expires_at = new_expires_at;
        self.refreshed_at = Some(now);
        self.last_refresh_error = None;
        Ok(self)
    }

    pub fn refresh_failed_transient(
        mut self,
        now: SystemTime,
        reason: impl Into<String>,
    ) -> Result<Self, TokenGrantError> {
        self.ensure_refreshable()?;

        self.state = TokenGrantState::NeedsRefresh;
        self.refreshed_at = Some(now);
        self.last_refresh_error = Some(reason.into());
        Ok(self)
    }

    pub fn refresh_failed_reauth_required(
        mut self,
        now: SystemTime,
        reason: impl Into<String>,
    ) -> Result<Self, TokenGrantError> {
        match self.state {
            TokenGrantState::Revoked => return Err(TokenGrantError::RevokedGrant),
            TokenGrantState::ReauthRequired => {
                return Err(TokenGrantError::InvalidTransition {
                    from: TokenGrantState::ReauthRequired,
                    event: "refresh_failed_reauth_required",
                });
            }
            TokenGrantState::Valid | TokenGrantState::Expired | TokenGrantState::NeedsRefresh => {}
        }

        self.state = TokenGrantState::ReauthRequired;
        self.reauth_required_at = Some(now);
        self.last_refresh_error = Some(reason.into());
        Ok(self)
    }

    pub fn revoke(
        mut self,
        now: SystemTime,
        reason: impl Into<String>,
    ) -> Result<Self, TokenGrantError> {
        if self.state == TokenGrantState::Revoked {
            return Err(TokenGrantError::InvalidTransition {
                from: TokenGrantState::Revoked,
                event: "revoke",
            });
        }

        self.state = TokenGrantState::Revoked;
        self.revoked_at = Some(now);
        self.revocation_reason = Some(reason.into());
        Ok(self)
    }

    fn ensure_refreshable(&self) -> Result<(), TokenGrantError> {
        match self.state {
            TokenGrantState::Revoked => return Err(TokenGrantError::RevokedGrant),
            TokenGrantState::ReauthRequired => return Err(TokenGrantError::ReauthRequired),
            TokenGrantState::Valid | TokenGrantState::Expired | TokenGrantState::NeedsRefresh => {}
        }

        if self.tokens.refresh_token.is_none() {
            return Err(TokenGrantError::MissingRefreshToken);
        }

        Ok(())
    }
}
