use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use oar_core::action::execution_request::ConfirmedExecutionRequest;
use oar_core::action::executor::{ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution};
use oar_core::domain::token_refresh::service::{AsyncAuthRefreshAdapter, AuthRefreshAdapter};
use oar_core::domain::token_refresh::types::{RefreshOutcome, TokenRefreshGrantSnapshot};
use oar_core::lark::auth::adapter::{AsyncFeishuAuthRefreshClient, FeishuAuthRefreshClient};
use oar_core::lark::auth::parser::parse_feishu_auth_refresh_response;
use oar_core::lark::auth::types::{FeishuAuthRefreshRequest, FeishuAuthRefreshResponse};
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher,
};
use oar_core::storage::postgres::AuditOutboxMessage;

use super::summary;

#[derive(Clone, Default)]
pub(crate) struct LiveMockAdapter {
    state: Arc<Mutex<LiveMockAdapterState>>,
}

#[derive(Default)]
struct LiveMockAdapterState {
    dry_run_calls: usize,
    execute_calls: usize,
    execute_error: Option<AdapterError>,
}

impl LiveMockAdapter {
    pub(crate) fn succeeding() -> Self {
        Self::default()
    }

    pub(crate) fn failing(code: &str, message: &str) -> Self {
        let adapter = Self::default();
        adapter.state.lock().expect("adapter mutex").execute_error =
            Some(AdapterError::from_safe_message(code, message));
        adapter
    }

    pub(crate) fn dry_run_calls(&self) -> usize {
        self.state.lock().expect("adapter mutex").dry_run_calls
    }

    pub(crate) fn execute_calls(&self) -> usize {
        self.state.lock().expect("adapter mutex").execute_calls
    }
}

impl ActionAdapter for LiveMockAdapter {
    fn dry_run(
        &mut self,
        _request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterDryRun, AdapterError> {
        self.state.lock().expect("adapter mutex").dry_run_calls += 1;
        Ok(AdapterDryRun {
            before: Some(summary("before")),
            after: Some(summary("dry-run projected")),
        })
    }

    fn execute(
        &mut self,
        _request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterExecution, AdapterError> {
        let mut state = self.state.lock().expect("adapter mutex");
        state.execute_calls += 1;
        if let Some(error) = state.execute_error.clone() {
            return Err(error);
        }

        Ok(AdapterExecution {
            adapter_operation_id: "lark-op-live".to_string(),
            before: Some(summary("before")),
            after: Some(summary("applied")),
        })
    }
}

#[derive(Clone)]
pub(crate) struct LiveOutboxDispatcher {
    outcomes: Arc<Mutex<Vec<AuditOutboxDelivery>>>,
}

impl LiveOutboxDispatcher {
    pub(crate) fn new(outcomes: impl IntoIterator<Item = AuditOutboxDelivery>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
        }
    }
}

impl AuditOutboxDispatcher for LiveOutboxDispatcher {
    type Error = ();

    async fn deliver(
        &mut self,
        _message: &AuditOutboxMessage,
    ) -> Result<AuditOutboxDelivery, Self::Error> {
        let mut outcomes = self.outcomes.lock().expect("outbox dispatcher mutex");
        if outcomes.is_empty() {
            return Ok(AuditOutboxDelivery::Sent);
        }

        Ok(outcomes.remove(0))
    }
}

#[derive(Clone)]
pub(crate) struct LiveRefreshAdapter {
    outcome: RefreshOutcome,
    calls: Arc<Mutex<usize>>,
}

impl LiveRefreshAdapter {
    pub(crate) fn new(outcome: RefreshOutcome) -> Self {
        Self {
            outcome,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn calls(&self) -> usize {
        *self.calls.lock().expect("refresh adapter mutex")
    }
}

impl AuthRefreshAdapter for LiveRefreshAdapter {
    fn refresh(&mut self, _snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let mut calls = self.calls.lock().expect("refresh adapter mutex");
        *calls += 1;
        self.outcome.clone()
    }
}

#[async_trait::async_trait]
impl AsyncAuthRefreshAdapter for LiveRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        AuthRefreshAdapter::refresh(self, snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct SequenceRefreshAdapter {
    outcomes: Arc<Mutex<VecDeque<RefreshOutcome>>>,
    called_grant_ids: Arc<Mutex<Vec<String>>>,
}

impl SequenceRefreshAdapter {
    pub(crate) fn new(outcomes: impl IntoIterator<Item = RefreshOutcome>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
            called_grant_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn called_grant_ids(&self) -> Vec<String> {
        self.called_grant_ids
            .lock()
            .expect("sequence refresh adapter mutex")
            .clone()
    }
}

impl AuthRefreshAdapter for SequenceRefreshAdapter {
    fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        self.called_grant_ids
            .lock()
            .expect("sequence refresh adapter calls mutex")
            .push(snapshot.grant_id.0.clone());
        self.outcomes
            .lock()
            .expect("sequence refresh adapter outcomes mutex")
            .pop_front()
            .expect("sequence refresh outcome")
    }
}

#[async_trait::async_trait]
impl AsyncAuthRefreshAdapter for SequenceRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        AuthRefreshAdapter::refresh(self, snapshot)
    }
}

#[derive(Clone)]
pub(crate) struct FixtureClient {
    fixture: &'static str,
    calls: Arc<Mutex<usize>>,
}

impl FixtureClient {
    pub(crate) fn new(fixture: &'static str) -> Self {
        Self {
            fixture,
            calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn calls(&self) -> usize {
        *self.calls.lock().expect("fixture client mutex")
    }
}

impl FeishuAuthRefreshClient for FixtureClient {
    type Error = &'static str;

    fn refresh(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, Self::Error> {
        let mut calls = self.calls.lock().expect("fixture client mutex");
        *calls += 1;
        parse_feishu_auth_refresh_response(self.fixture).map_err(|_| "fixture_parse_failed")
    }
}

#[async_trait::async_trait]
impl AsyncFeishuAuthRefreshClient for FixtureClient {
    type Error = &'static str;

    async fn refresh(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, Self::Error> {
        FeishuAuthRefreshClient::refresh(self, request)
    }
}
