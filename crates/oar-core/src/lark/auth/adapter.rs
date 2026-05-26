use std::fmt;
use std::time::{Duration, SystemTime};

use crate::domain::token_refresh::service::AuthRefreshAdapter;
use crate::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshGrantSnapshot,
};

use super::safety::{sanitize_safe_error, SAFE_REAUTH_ERROR, SAFE_TRANSIENT_ERROR};
use super::types::{LarkAuthRefreshFailure, LarkAuthRefreshRequest, LarkAuthRefreshResponse};

pub trait LarkAuthRefreshClient {
    type Error;

    fn refresh(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshAdapter<C> {
    client: C,
}

impl<C> fmt::Debug for LarkAuthRefreshAdapter<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshAdapter")
            .field("client", &"[REDACTED]")
            .finish()
    }
}

impl<C> LarkAuthRefreshAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn client(&self) -> &C {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut C {
        &mut self.client
    }
}

impl<C> AuthRefreshAdapter for LarkAuthRefreshAdapter<C>
where
    C: LarkAuthRefreshClient,
{
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let request = LarkAuthRefreshRequest::from_snapshot(snapshot);
        match self.client.refresh(&request) {
            Ok(LarkAuthRefreshResponse::Success(success)) => RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: success.encrypted_primary,
                    encrypted_renewal: success.encrypted_renewal,
                },
                key_id: success.key_id,
                new_fingerprint: success.new_fingerprint,
                refreshed_at: ms_to_system_time(success.refreshed_at_ms),
                expires_at: success.expires_at_ms.map(ms_to_system_time),
            },
            Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::Transient {
                safe_error,
            })) => RefreshOutcome::TransientFailure {
                safe_error: sanitize_safe_error(&safe_error, SAFE_TRANSIENT_ERROR),
            },
            Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ReauthRequired {
                safe_error,
            })) => RefreshOutcome::ReauthFailure {
                safe_error: sanitize_safe_error(&safe_error, SAFE_REAUTH_ERROR),
            },
            Err(_) => RefreshOutcome::TransientFailure {
                safe_error: SAFE_TRANSIENT_ERROR.to_string(),
            },
        }
    }
}

fn ms_to_system_time(ms: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
}
