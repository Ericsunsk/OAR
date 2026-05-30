use super::harness::*;

#[path = "review_inbox/decisions.rs"]
mod decisions;
#[path = "review_inbox/items.rs"]
mod items;
#[path = "review_inbox/schema_guards.rs"]
mod schema_guards;

const VALID_HASH: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn ms(value: u64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_millis(value)
}

fn evidence_item(id: &str, summary: &str, source_id: &str) -> EvidenceItem {
    EvidenceItem::new(
        EvidenceId(id.to_string()),
        summary,
        EvidenceRef::new(EvidenceSourceKind::OkrProgress, source_id, None)
            .expect("evidence reference should be valid"),
        VALID_HASH,
        EvidenceVisibilityScope::Tenant,
        ms(1_748_250_000_000),
        ms(1_748_250_001_000),
    )
    .expect("evidence item should be valid")
}

fn proposed_action(tenant_id: &str, user_id: &str, id: &str, version: u64) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        version,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![format!("evidence_{id}")],
        json!({"kind": "update_kr_progress"}),
    )
    .expect("proposed action draft should be valid");
    action.publish().expect("publish should work");
    action
}

struct InboxItemSpec<'a> {
    id: &'a str,
    tenant_id: &'a str,
    user_id: &'a str,
    proposed_action_id: &'a str,
    proposed_action_version: u64,
    sort_key: i64,
    sync_cursor: u64,
    status: ReviewInboxItemStatus,
    ledger_status: Option<&'a str>,
    operation_id: Option<&'a str>,
}

fn inbox_item(spec: InboxItemSpec<'_>) -> ReviewInboxItem {
    let mut item = ReviewInboxItem::new(
        ReviewInboxItemId(spec.id.to_string()),
        TenantId(spec.tenant_id.to_string()),
        WorkspaceUserId(spec.user_id.to_string()),
        spec.proposed_action_id,
        spec.proposed_action_version,
        80,
        10,
        spec.sort_key,
        spec.sync_cursor,
        ms(1_748_250_005_000 + spec.sync_cursor),
    );
    item.status = spec.status;
    item.ledger_status = spec.ledger_status.map(str::to_string);
    item.operation_id = spec.operation_id.map(str::to_string);
    item
}
