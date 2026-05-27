use std::collections::HashSet;
use std::time::SystemTime;

use serde_json::Value;

use crate::action::confirmed_action::ConfirmedAction;
use crate::domain::identity::{TenantId, WorkspaceUserId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProposedActionId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposedActionStatus {
    Draft,
    Published,
    Superseded,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposedActionKind {
    CreateKrProgress,
    UpdateKrProgress,
    DeleteKrProgressDryRun,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProposedActionDecision {
    Confirm,
    EditThenConfirm { edited_payload: Value },
    Reject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposedAction {
    pub id: ProposedActionId,
    pub tenant_id: TenantId,
    pub actor_user_id: WorkspaceUserId,
    pub target_user_id: Option<WorkspaceUserId>,
    pub owner_user_id: Option<WorkspaceUserId>,
    pub version: u64,
    pub status: ProposedActionStatus,
    pub kind: ProposedActionKind,
    pub risk_severity: RiskSeverity,
    pub evidence_ids: Vec<String>,
    pub suggested_payload: Value,
    pub decision: Option<ProposedActionDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposedActionError {
    EmptyEvidence,
    InvalidEvidenceId,
    InvalidVersion { version: u64 },
    InvalidStatusForPublish { status: ProposedActionStatus },
    InvalidStatusForWithdraw { status: ProposedActionStatus },
    InvalidStatusForSupersede { status: ProposedActionStatus },
    InvalidStatusForDecision { status: ProposedActionStatus },
    DecisionAlreadyFinalized,
}

impl ProposedAction {
    #[allow(clippy::too_many_arguments)]
    pub fn draft(
        id: ProposedActionId,
        tenant_id: TenantId,
        actor_user_id: WorkspaceUserId,
        target_user_id: Option<WorkspaceUserId>,
        owner_user_id: Option<WorkspaceUserId>,
        version: u64,
        kind: ProposedActionKind,
        risk_severity: RiskSeverity,
        evidence_ids: Vec<String>,
        suggested_payload: Value,
    ) -> Result<Self, ProposedActionError> {
        if version == 0 {
            return Err(ProposedActionError::InvalidVersion { version });
        }
        let evidence_ids = normalize_evidence_ids(evidence_ids)?;

        Ok(Self {
            id,
            tenant_id,
            actor_user_id,
            target_user_id,
            owner_user_id,
            version,
            status: ProposedActionStatus::Draft,
            kind,
            risk_severity,
            evidence_ids,
            suggested_payload,
            decision: None,
        })
    }

    pub fn publish(&mut self) -> Result<(), ProposedActionError> {
        self.ensure_publishable()?;
        self.status = ProposedActionStatus::Published;
        Ok(())
    }

    pub fn withdraw(&mut self) -> Result<(), ProposedActionError> {
        if self.decision.is_some() {
            return Err(ProposedActionError::DecisionAlreadyFinalized);
        }
        if !matches!(
            self.status,
            ProposedActionStatus::Draft | ProposedActionStatus::Published
        ) {
            return Err(ProposedActionError::InvalidStatusForWithdraw {
                status: self.status,
            });
        }

        self.status = ProposedActionStatus::Withdrawn;
        Ok(())
    }

    pub fn supersede(&mut self) -> Result<(), ProposedActionError> {
        if self.decision.is_some() {
            return Err(ProposedActionError::DecisionAlreadyFinalized);
        }
        if self.status != ProposedActionStatus::Published {
            return Err(ProposedActionError::InvalidStatusForSupersede {
                status: self.status,
            });
        }

        self.status = ProposedActionStatus::Superseded;
        Ok(())
    }

    pub fn decide(
        &mut self,
        decision: ProposedActionDecision,
        confirmed_at: SystemTime,
    ) -> Result<Option<ConfirmedAction>, ProposedActionError> {
        self.ensure_decidable()?;
        self.decision = Some(decision.clone());

        match decision {
            ProposedActionDecision::Confirm => {
                Ok(Some(self.build_confirmed_action("confirm", confirmed_at)))
            }
            ProposedActionDecision::EditThenConfirm { .. } => Ok(Some(
                self.build_confirmed_action("edit_then_confirm", confirmed_at),
            )),
            ProposedActionDecision::Reject => Ok(None),
        }
    }

    fn ensure_publishable(&self) -> Result<(), ProposedActionError> {
        if self.version == 0 {
            return Err(ProposedActionError::InvalidVersion {
                version: self.version,
            });
        }
        if self.evidence_ids.is_empty() {
            return Err(ProposedActionError::EmptyEvidence);
        }
        if !has_valid_evidence_chain(&self.evidence_ids) {
            return Err(ProposedActionError::InvalidEvidenceId);
        }
        if self.status != ProposedActionStatus::Draft {
            return Err(ProposedActionError::InvalidStatusForPublish {
                status: self.status,
            });
        }
        Ok(())
    }

    fn ensure_decidable(&self) -> Result<(), ProposedActionError> {
        if self.version == 0 {
            return Err(ProposedActionError::InvalidVersion {
                version: self.version,
            });
        }
        if self.evidence_ids.is_empty() {
            return Err(ProposedActionError::EmptyEvidence);
        }
        if !has_valid_evidence_chain(&self.evidence_ids) {
            return Err(ProposedActionError::InvalidEvidenceId);
        }
        if self.status != ProposedActionStatus::Published {
            return Err(ProposedActionError::InvalidStatusForDecision {
                status: self.status,
            });
        }
        if self.decision.is_some() {
            return Err(ProposedActionError::DecisionAlreadyFinalized);
        }
        Ok(())
    }

    fn build_confirmed_action(
        &self,
        decision_kind: &str,
        confirmed_at: SystemTime,
    ) -> ConfirmedAction {
        let version_chain = format!("{}:v{}", self.id.0, self.version);
        let action_id = format!("pa:{version_chain}:{decision_kind}");
        let idempotency_key = format!(
            "tenant:{}:pa:{version_chain}:{decision_kind}",
            self.tenant_id.0
        );

        ConfirmedAction::proposed(
            action_id,
            self.tenant_id.0.clone(),
            self.actor_user_id.0.clone(),
            idempotency_key,
        )
        .confirm(confirmed_at)
    }
}

fn normalize_evidence_ids(evidence_ids: Vec<String>) -> Result<Vec<String>, ProposedActionError> {
    if evidence_ids.is_empty() {
        return Err(ProposedActionError::EmptyEvidence);
    }

    let mut seen = HashSet::with_capacity(evidence_ids.len());
    let mut normalized = Vec::with_capacity(evidence_ids.len());

    for raw in evidence_ids {
        let evidence_id = raw.trim();
        if evidence_id.is_empty() {
            return Err(ProposedActionError::InvalidEvidenceId);
        }
        if seen.insert(evidence_id.to_string()) {
            normalized.push(evidence_id.to_string());
        }
    }

    if normalized.is_empty() {
        return Err(ProposedActionError::EmptyEvidence);
    }

    Ok(normalized)
}

fn has_valid_evidence_chain(evidence_ids: &[String]) -> bool {
    evidence_ids.iter().all(|id| !id.trim().is_empty())
}
