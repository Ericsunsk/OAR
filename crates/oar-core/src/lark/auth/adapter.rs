use std::fmt;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;

use crate::domain::token_refresh::service::{AsyncAuthRefreshAdapter, AuthRefreshAdapter};
use crate::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshGrantSnapshot,
};

use super::safety::{
    sanitize_safe_error, SAFE_AUTH_REFRESH_OVERSIZED_RESPONSE, SAFE_AUTH_REFRESH_PARSE_FAILED,
    SAFE_CONFIG_ERROR, SAFE_REAUTH_ERROR, SAFE_TRANSIENT_ERROR,
};
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
    C::Error: 'static,
{
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        self.refresh_sync(snapshot)
    }
}

#[async_trait(?Send)]
pub trait AsyncLarkAuthRefreshClient {
    type Error;

    async fn refresh(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error>;
}

#[async_trait(?Send)]
impl<T> AsyncLarkAuthRefreshClient for super::client::LarkAuthRefreshSafeClient<T>
where
    T: super::client::AsyncLarkAuthRefreshTransport,
{
    type Error = super::client::LarkAuthRefreshClientError;

    async fn refresh(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error> {
        self.refresh_async(request).await
    }
}

#[async_trait(?Send)]
impl<C> AsyncAuthRefreshAdapter for LarkAuthRefreshAdapter<C>
where
    C: AsyncLarkAuthRefreshClient,
    C::Error: 'static,
{
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let request = LarkAuthRefreshRequest::from_snapshot(snapshot);
        response_to_outcome(self.client.refresh(&request).await)
    }
}

impl<C> LarkAuthRefreshAdapter<C> {
    fn refresh_sync(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome
    where
        C: LarkAuthRefreshClient,
        C::Error: 'static,
    {
        let request = LarkAuthRefreshRequest::from_snapshot(snapshot);
        response_to_outcome(self.client.refresh(&request))
    }
}

fn response_to_outcome<E>(response: Result<LarkAuthRefreshResponse, E>) -> RefreshOutcome
where
    E: 'static,
{
    match response {
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
        Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::Transient { safe_error })) => {
            RefreshOutcome::TransientFailure {
                safe_error: sanitize_safe_error(&safe_error, SAFE_TRANSIENT_ERROR),
            }
        }
        Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ReauthRequired {
            safe_error,
        })) => RefreshOutcome::ReauthFailure {
            safe_error: sanitize_safe_error(&safe_error, SAFE_REAUTH_ERROR),
        },
        Ok(LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ConfigRequired {
            safe_error,
        })) => RefreshOutcome::ConfigRequired {
            safe_error: sanitize_safe_error(&safe_error, SAFE_CONFIG_ERROR),
        },
        Err(error) => map_error_to_outcome(&error),
    }
}

fn map_error_to_outcome<E>(error: &E) -> RefreshOutcome
where
    E: 'static,
{
    let any = error as &dyn std::any::Any;
    if let Some(client_error) = any.downcast_ref::<super::client::LarkAuthRefreshClientError>() {
        return match client_error {
            super::client::LarkAuthRefreshClientError::Transport => {
                RefreshOutcome::TransientFailure {
                    safe_error: SAFE_TRANSIENT_ERROR.to_string(),
                }
            }
            super::client::LarkAuthRefreshClientError::OversizedResponse { .. } => {
                RefreshOutcome::ConfigRequired {
                    safe_error: SAFE_AUTH_REFRESH_OVERSIZED_RESPONSE.to_string(),
                }
            }
            super::client::LarkAuthRefreshClientError::Parse => RefreshOutcome::ConfigRequired {
                safe_error: SAFE_AUTH_REFRESH_PARSE_FAILED.to_string(),
            },
        };
    }

    RefreshOutcome::TransientFailure {
        safe_error: SAFE_TRANSIENT_ERROR.to_string(),
    }
}

fn ms_to_system_time(ms: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
}
