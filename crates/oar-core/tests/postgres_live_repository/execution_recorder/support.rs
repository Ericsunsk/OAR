use super::super::harness::*;

pub(super) fn execution_projection_proposed_action(
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        1,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![format!("evidence_{id}")],
        json!({"kind": "update_kr_progress", "delta": "weekly"}),
    )
    .expect("proposed action should be valid");
    action.publish().expect("publish should work");
    action
}

pub(super) struct ProjectionInboxSpec<'a> {
    pub(super) id: &'a str,
    pub(super) tenant_id: &'a str,
    pub(super) user_id: &'a str,
    pub(super) proposed_action_id: &'a str,
    pub(super) status: ReviewInboxItemStatus,
    pub(super) ledger_status: Option<&'a str>,
    pub(super) operation_id: Option<&'a str>,
    pub(super) sync_cursor: u64,
}

pub(super) fn execution_projection_inbox_item(spec: ProjectionInboxSpec<'_>) -> ReviewInboxItem {
    let mut item = ReviewInboxItem::new(
        ReviewInboxItemId(spec.id.to_string()),
        TenantId(spec.tenant_id.to_string()),
        WorkspaceUserId(spec.user_id.to_string()),
        spec.proposed_action_id,
        1,
        80,
        3,
        900,
        spec.sync_cursor,
        SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(spec.sync_cursor),
    );
    item.status = spec.status;
    item.ledger_status = spec.ledger_status.map(str::to_string);
    item.operation_id = spec.operation_id.map(str::to_string);
    item
}

pub(super) async fn seed_confirmed_inbox_projection(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    proposed_action_id: &str,
    inbox_id: &str,
    operation_id: &str,
    sync_cursor: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = PostgresReviewInboxRepository::new(pool.clone());
    let proposed_action =
        execution_projection_proposed_action(tenant_id, user_id, proposed_action_id);
    repository
        .insert_proposed_action(
            &proposed_action,
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(sync_cursor)),
        )
        .await?;
    repository
        .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
            id: inbox_id,
            tenant_id,
            user_id,
            proposed_action_id,
            status: ReviewInboxItemStatus::Confirmed,
            ledger_status: Some("confirmed"),
            operation_id: Some(operation_id),
            sync_cursor,
        }))
        .await?;
    Ok(())
}
