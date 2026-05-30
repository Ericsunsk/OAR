use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn stored_evidence_item_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredEvidenceItem> {
    let source_kind: String = row.try_get("source_kind")?;
    let visibility_scope: String = row.try_get("visibility_scope")?;

    Ok(StoredEvidenceItem {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        summary: row.try_get("summary")?,
        source_kind: evidence_source_kind_from_db(&source_kind)?,
        source_id: row.try_get("source_id")?,
        locator: row.try_get("locator")?,
        content_hash: row.try_get("content_hash")?,
        visibility_scope: evidence_visibility_scope_from_db(&visibility_scope)?,
        observed_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("observed_at_ms")?,
            "observed_at_ms",
        )?),
        recorded_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("recorded_at_ms")?,
            "recorded_at_ms",
        )?),
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_review_inbox_item_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredReviewInboxItem> {
    let status: String = row.try_get("status")?;
    let ledger_status: Option<String> = row.try_get("ledger_status")?;
    Ok(StoredReviewInboxItem {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        user_id: row.try_get("user_id")?,
        proposed_action_id: row.try_get("proposed_action_id")?,
        proposed_action_version: non_negative_i64_to_u64(
            row.try_get("proposed_action_version")?,
            "proposed_action_version",
        )?,
        risk_score: non_negative_i64_to_u64(
            row.try_get::<i32, _>("risk_score")? as i64,
            "risk_score",
        )? as u32,
        priority: non_negative_i64_to_u64(row.try_get::<i32, _>("priority")? as i64, "priority")?
            as u32,
        status: review_inbox_item_status_from_db(&status)?,
        sort_key: row.try_get("sort_key")?,
        sync_cursor_value: non_negative_i64_to_u64(
            row.try_get("sync_cursor_value")?,
            "sync_cursor_value",
        )?,
        updated_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("updated_at_ms")?,
            "updated_at_ms",
        )?),
        ledger_status: ledger_status
            .as_deref()
            .map(action_status_from_db)
            .transpose()?,
        operation_id: row.try_get("operation_id")?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_review_inbox_action_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredReviewInboxAction> {
    let status: String = row.try_get("status")?;
    let kind: String = row.try_get("kind")?;
    let custom_kind: Option<String> = row.try_get("custom_kind")?;
    let risk_severity: String = row.try_get("risk_severity")?;
    Ok(StoredReviewInboxAction {
        review_item_id: row.try_get("review_item_id")?,
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        actor_user_id: row.try_get("actor_user_id")?,
        target_user_id: row.try_get("target_user_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        version: non_negative_i64_to_u64(row.try_get("version")?, "version")?,
        status: proposed_action_status_from_db(&status)?,
        kind: proposed_action_kind_from_db(&kind, custom_kind)?,
        risk_severity: risk_severity_from_db(&risk_severity)?,
        evidence_ids: row.try_get("evidence_ids")?,
        suggested_payload: row.try_get("suggested_payload")?,
        decision: stored_review_inbox_action_decision_from_row(row)?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_review_inbox_evidence_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredReviewInboxEvidence> {
    Ok(StoredReviewInboxEvidence {
        review_item_id: row.try_get("review_item_id")?,
        item: stored_evidence_item_from_row(row)?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_review_inbox_ledger_event_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredReviewInboxLedgerEvent> {
    let stage: String = row.try_get("stage")?;
    let stage_status: String = row.try_get("stage_status")?;
    Ok(StoredReviewInboxLedgerEvent {
        id: row.try_get("id")?,
        action_id: row.try_get("action_id")?,
        stage: review_inbox_ledger_stage_from_db(&stage)?,
        stage_status: review_inbox_ledger_status_from_db(&stage_status)?,
        timestamp: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("timestamp_ms")?,
            "timestamp_ms",
        )?),
        message: row.try_get("message")?,
        idempotency_key: row.try_get("idempotency_key")?,
    })
}

fn stored_review_inbox_action_decision_from_row(
    row: &PgRow,
) -> PgRepositoryResult<Option<StoredReviewInboxActionDecision>> {
    let Some(id) = row.try_get("decision_id")? else {
        return Ok(None);
    };
    let decision: String = row.try_get("decision")?;
    Ok(Some(StoredReviewInboxActionDecision {
        id,
        actor_user_id: row.try_get("decision_actor_user_id")?,
        decision: proposed_action_decision_kind_from_db(&decision)?,
        confirmed_action_id: row.try_get("confirmed_action_id")?,
        decided_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("decided_at_ms")?,
            "decided_at_ms",
        )?),
    }))
}
