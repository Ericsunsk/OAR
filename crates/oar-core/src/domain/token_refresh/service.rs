use std::fmt;
use std::time::SystemTime;

use async_trait::async_trait;

use super::bridge::{plan_token_refresh_command, TokenRefreshBridgeError};
use super::types::{
    RefreshOutcome, TokenRefreshApplyResult, TokenRefreshGrantSnapshot, TokenRefreshReportStatus,
    TokenRefreshRepositoryCommand, TokenRefreshServiceReport,
};

pub trait AuthRefreshAdapter {
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome;
}

#[async_trait(?Send)]
pub trait AsyncAuthRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome;
}

pub trait TokenRefreshCommandSink {
    type Error;

    fn apply_refresh_command(
        &mut self,
        command: TokenRefreshRepositoryCommand,
    ) -> Result<Option<TokenRefreshApplyResult>, Self::Error>;
}

#[derive(Clone, PartialEq, Eq)]
pub enum TokenRefreshServiceError<E> {
    DecisionBridge(TokenRefreshBridgeError),
    CommandSink(E),
}

impl<E> fmt::Debug for TokenRefreshServiceError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenRefreshServiceError::DecisionBridge(error) => {
                f.debug_tuple("DecisionBridge").field(error).finish()
            }
            TokenRefreshServiceError::CommandSink(_) => {
                f.debug_tuple("CommandSink").field(&"[REDACTED]").finish()
            }
        }
    }
}

impl<E> fmt::Display for TokenRefreshServiceError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenRefreshServiceError::DecisionBridge(error) => write!(f, "{error}"),
            TokenRefreshServiceError::CommandSink(_) => {
                write!(f, "token refresh command sink failed")
            }
        }
    }
}

impl<E> std::error::Error for TokenRefreshServiceError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TokenRefreshServiceError::DecisionBridge(error) => Some(error),
            TokenRefreshServiceError::CommandSink(_) => None,
        }
    }
}

impl<E> From<TokenRefreshBridgeError> for TokenRefreshServiceError<E> {
    fn from(value: TokenRefreshBridgeError) -> Self {
        Self::DecisionBridge(value)
    }
}

pub struct TokenRefreshService<A, S>
where
    A: AuthRefreshAdapter,
    S: TokenRefreshCommandSink,
{
    adapter: A,
    sink: S,
}

impl<A, S> TokenRefreshService<A, S>
where
    A: AuthRefreshAdapter,
    S: TokenRefreshCommandSink,
{
    pub fn new(adapter: A, sink: S) -> Self {
        Self { adapter, sink }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn refresh_grant_at(
        &mut self,
        snapshot: TokenRefreshGrantSnapshot,
        now: SystemTime,
    ) -> Result<TokenRefreshServiceReport, TokenRefreshServiceError<S::Error>> {
        if let Some(report) = token_refresh_short_circuit_report(&snapshot) {
            return Ok(report);
        }

        let outcome = self.adapter.refresh(&snapshot);
        let planned = plan_token_refresh_command(&snapshot, outcome, now)?;
        let apply_result = self
            .sink
            .apply_refresh_command(planned.command)
            .map_err(TokenRefreshServiceError::CommandSink)?;

        Ok(planned.report.into_service_report(apply_result.is_some()))
    }
}

pub fn token_refresh_short_circuit_report(
    snapshot: &TokenRefreshGrantSnapshot,
) -> Option<TokenRefreshServiceReport> {
    snapshot
        .short_circuit_reason()
        .map(|reason| TokenRefreshServiceReport {
            grant_id: snapshot.grant_id.clone(),
            tenant_id: snapshot.tenant_id.clone(),
            status: TokenRefreshReportStatus::ShortCircuited(reason),
            adapter_called: false,
            sink_called: false,
            decision: None,
            command: None,
            safe_error: None,
        })
}
