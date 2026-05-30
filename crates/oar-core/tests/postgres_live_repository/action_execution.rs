use super::harness::*;
use serde_json::Value;

#[path = "action_execution/basic_flow.rs"]
mod basic_flow;

#[path = "action_execution/failures.rs"]
mod failures;

#[path = "action_execution/queue.rs"]
mod queue;

#[path = "action_execution/resume.rs"]
mod resume;

#[path = "action_execution/resume_support.rs"]
mod resume_support;

const EXECUTION_QUEUE_HASH: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn execution_payload(objective_id: &str, kr_id: &str, progress_delta: i64) -> Value {
    json!({
        "target": {
            "objective_id": objective_id,
            "kr_id": kr_id
        },
        "mutation": {
            "progress_delta": progress_delta,
            "note": "weekly check-in"
        }
    })
}

fn execution_evidence_item(id: &str, source_id: &str) -> EvidenceItem {
    EvidenceItem::new(
        EvidenceId(id.to_string()),
        "execution evidence",
        EvidenceRef::new(EvidenceSourceKind::OkrProgress, source_id, None)
            .expect("evidence reference should be valid"),
        EXECUTION_QUEUE_HASH,
        EvidenceVisibilityScope::Tenant,
        UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_000_000),
        UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_001_000),
    )
    .expect("evidence item should be valid")
}

fn proposed_execution_action(
    tenant_id: &str,
    user_id: &str,
    proposed_action_id: &str,
    evidence_id: &str,
    suggested_payload: Value,
) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(proposed_action_id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        1,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![evidence_id.to_string()],
        suggested_payload,
    )
    .expect("proposed action should be valid");
    action.publish().expect("proposed action should publish");
    action
}

struct ExecutionQueueSeed<'a> {
    tenant_id: &'a str,
    user_id: &'a str,
    proposed_action_id: &'a str,
    confirmed_action_id: &'a str,
    idempotency_key: &'a str,
    operation_id: &'a str,
    decision: ProposedActionDecision,
    suggested_payload: Value,
}

async fn seed_confirmed_execution_queue_item(
    pool: &PgPool,
    seed: ExecutionQueueSeed<'_>,
) -> Result<ConfirmedAction, Box<dyn std::error::Error + Send + Sync>> {
    let review_repository = PostgresReviewInboxRepository::new(pool.clone());
    let evidence_id = format!("evidence_{}", seed.proposed_action_id);
    review_repository
        .insert_evidence_item(
            seed.tenant_id,
            &execution_evidence_item(&evidence_id, seed.proposed_action_id),
        )
        .await?;
    let proposed_action = proposed_execution_action(
        seed.tenant_id,
        seed.user_id,
        seed.proposed_action_id,
        &evidence_id,
        seed.suggested_payload,
    );
    review_repository
        .insert_proposed_action(
            &proposed_action,
            Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_002_000)),
        )
        .await?;
    review_repository
        .insert_proposed_action_evidence_ref(
            seed.tenant_id,
            seed.proposed_action_id,
            1,
            &evidence_id,
        )
        .await?;

    let confirmed = confirmed_action(
        seed.confirmed_action_id,
        seed.tenant_id,
        seed.user_id,
        seed.idempotency_key,
    );
    PostgresOperationLedgerRepository::new(pool.clone())
        .submit_confirmed_action(&confirmed, 1_748_260_003_000, seed.operation_id)
        .await?;

    let decision_id = format!("decision_{}", seed.proposed_action_id);
    review_repository
        .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
            id: &decision_id,
            tenant_id: seed.tenant_id,
            proposed_action_id: seed.proposed_action_id,
            proposed_action_version: 1,
            actor_user_id: seed.user_id,
            decision: &seed.decision,
            confirmed_action_id: Some(&confirmed.action_id),
            decided_at: UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_004_000),
        })
        .await?;

    Ok(confirmed)
}
