use crate::action::audit_event::AuditStateSummary;
use crate::action::confirmed_action::ConfirmedAction;
use crate::action::executor::{ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMutationKind {
    Create,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressMutation {
    pub kind: ProgressMutationKind,
    pub objective_id: String,
    pub key_result_id: String,
    pub progress_delta: i32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LarkExecutionRequest {
    pub confirmed_action: ConfirmedAction,
    pub mutation: ProgressMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LarkExecutionMode {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LarkExecutionSummary {
    pub action_id: String,
    pub idempotency_key: String,
    pub mode: LarkExecutionMode,
    pub kind: ProgressMutationKind,
    pub resource_hint: String,
    pub progress_delta: i32,
    pub accepted: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LarkAdapterError {
    UnsupportedAction { reason: String },
    ExecutionFailed { code: String, safe_message: String },
}

pub trait LarkAdapter {
    fn dry_run(
        &self,
        request: &LarkExecutionRequest,
    ) -> Result<LarkExecutionSummary, LarkAdapterError>;

    fn execute(
        &self,
        request: &LarkExecutionRequest,
    ) -> Result<LarkExecutionSummary, LarkAdapterError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockOutcome {
    Succeed,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockLarkAdapter {
    outcome: MockOutcome,
}

impl MockLarkAdapter {
    pub fn succeeding() -> Self {
        Self {
            outcome: MockOutcome::Succeed,
        }
    }

    pub fn failing() -> Self {
        Self {
            outcome: MockOutcome::Fail,
        }
    }

    fn safe_resource_hint(mutation: &ProgressMutation) -> String {
        format!(
            "{}:{}",
            mutation.objective_id.chars().take(8).collect::<String>(),
            mutation.key_result_id.chars().take(8).collect::<String>()
        )
    }

    fn success_summary(
        mode: LarkExecutionMode,
        request: &LarkExecutionRequest,
    ) -> LarkExecutionSummary {
        let mutation = &request.mutation;
        LarkExecutionSummary {
            action_id: request.confirmed_action.action_id.clone(),
            idempotency_key: request.confirmed_action.idempotency_key.clone(),
            mode,
            kind: mutation.kind,
            resource_hint: Self::safe_resource_hint(mutation),
            progress_delta: mutation.progress_delta,
            accepted: true,
            message: match mode {
                LarkExecutionMode::DryRun => "dry-run accepted".to_string(),
                LarkExecutionMode::Execute => "executed via mock adapter".to_string(),
            },
        }
    }
}

impl LarkAdapter for MockLarkAdapter {
    fn dry_run(
        &self,
        request: &LarkExecutionRequest,
    ) -> Result<LarkExecutionSummary, LarkAdapterError> {
        Ok(Self::success_summary(LarkExecutionMode::DryRun, request))
    }

    fn execute(
        &self,
        request: &LarkExecutionRequest,
    ) -> Result<LarkExecutionSummary, LarkAdapterError> {
        match self.outcome {
            MockOutcome::Succeed => Ok(Self::success_summary(LarkExecutionMode::Execute, request)),
            MockOutcome::Fail => Err(LarkAdapterError::ExecutionFailed {
                code: "MOCK_EXECUTION_FAILURE".to_string(),
                safe_message: "mock adapter configured to fail".to_string(),
            }),
        }
    }
}

impl ActionAdapter for MockLarkAdapter {
    fn dry_run(&mut self, action: &ConfirmedAction) -> Result<AdapterDryRun, AdapterError> {
        let request = LarkExecutionRequest {
            confirmed_action: action.clone(),
            mutation: ProgressMutation {
                kind: ProgressMutationKind::Update,
                objective_id: "objective_mock_alpha".to_string(),
                key_result_id: "kr_mock_beta".to_string(),
                progress_delta: 5,
                note: Some("weekly check-in".to_string()),
            },
        };
        let summary = LarkAdapter::dry_run(self, &request).map_err(AdapterError::from)?;
        Ok(AdapterDryRun {
            before: None,
            after: Some(AuditStateSummary {
                summary: summary.message,
                reference_ids: vec![summary.resource_hint],
                content_hash: None,
            }),
        })
    }

    fn execute(&mut self, action: &ConfirmedAction) -> Result<AdapterExecution, AdapterError> {
        let request = LarkExecutionRequest {
            confirmed_action: action.clone(),
            mutation: ProgressMutation {
                kind: ProgressMutationKind::Update,
                objective_id: "objective_mock_alpha".to_string(),
                key_result_id: "kr_mock_beta".to_string(),
                progress_delta: 5,
                note: Some("weekly check-in".to_string()),
            },
        };
        let summary = LarkAdapter::execute(self, &request).map_err(AdapterError::from)?;
        Ok(AdapterExecution {
            adapter_operation_id: format!("mock-lark-{}", summary.idempotency_key),
            before: None,
            after: Some(AuditStateSummary {
                summary: summary.message,
                reference_ids: vec![summary.resource_hint],
                content_hash: None,
            }),
        })
    }
}

impl From<LarkAdapterError> for AdapterError {
    fn from(value: LarkAdapterError) -> Self {
        match value {
            LarkAdapterError::UnsupportedAction { reason } => {
                AdapterError::new("unsupported_action", reason)
            }
            LarkAdapterError::ExecutionFailed { code, safe_message } => {
                AdapterError::new(code, safe_message)
            }
        }
    }
}
