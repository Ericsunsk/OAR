use super::super::*;

pub(super) async fn load_review_inbox_snapshot_from_pool(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    after_cursor: u64,
    limit: u32,
) -> PgRepositoryResult<StoredReviewInboxSnapshot> {
    if limit == 0 {
        return Ok(StoredReviewInboxSnapshot {
            items: Vec::new(),
            actions: Vec::new(),
            evidence: Vec::new(),
            ledger_events: Vec::new(),
        });
    }

    let item_rows = sqlx::query(LIST_REVIEW_INBOX_ITEMS)
        .bind(tenant_id)
        .bind(user_id)
        .bind(after_cursor as i64)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;
    let items = item_rows
        .iter()
        .map(stored_review_inbox_item_from_row)
        .collect::<PgRepositoryResult<Vec<_>>>()?;

    if items.is_empty() {
        return Ok(StoredReviewInboxSnapshot {
            items,
            actions: Vec::new(),
            evidence: Vec::new(),
            ledger_events: Vec::new(),
        });
    }

    let action_rows = sqlx::query(LIST_REVIEW_INBOX_ACTIONS_FOR_SNAPSHOT)
        .bind(tenant_id)
        .bind(user_id)
        .bind(after_cursor as i64)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;
    let actions = action_rows
        .iter()
        .map(stored_review_inbox_action_from_row)
        .collect::<PgRepositoryResult<Vec<_>>>()?;

    let evidence_rows = sqlx::query(LIST_REVIEW_INBOX_EVIDENCE_FOR_SNAPSHOT)
        .bind(tenant_id)
        .bind(user_id)
        .bind(after_cursor as i64)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;
    let evidence = evidence_rows
        .iter()
        .map(stored_review_inbox_evidence_from_row)
        .collect::<PgRepositoryResult<Vec<_>>>()?;

    let ledger_event_rows = sqlx::query(LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT)
        .bind(tenant_id)
        .bind(user_id)
        .bind(after_cursor as i64)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;
    let ledger_events = ledger_event_rows
        .iter()
        .map(stored_review_inbox_ledger_event_from_row)
        .collect::<PgRepositoryResult<Vec<_>>>()?;

    Ok(StoredReviewInboxSnapshot {
        items,
        actions,
        evidence,
        ledger_events,
    })
}

pub(in crate::storage::postgres::repository_sqlx) async fn load_review_decision_context_from_pool(
    pool: &PgPool,
    request: PostgresReviewDecisionContextRequest<'_>,
) -> PgRepositoryResult<Option<StoredReviewDecisionContext>> {
    let item_row = sqlx::query(LOAD_REVIEW_DECISION_ITEM)
        .bind(request.tenant_id)
        .bind(request.user_id)
        .bind(request.proposed_action_id)
        .bind(request.proposed_action_version as i64)
        .bind(request.expected_sync_cursor_value as i64)
        .fetch_optional(pool)
        .await?;
    let Some(item_row) = item_row else {
        return Ok(None);
    };
    let item = stored_review_inbox_item_from_row(&item_row)?;

    let action_row = sqlx::query(LOAD_REVIEW_DECISION_ACTION)
        .bind(request.tenant_id)
        .bind(request.user_id)
        .bind(request.proposed_action_id)
        .bind(request.proposed_action_version as i64)
        .bind(request.expected_sync_cursor_value as i64)
        .fetch_optional(pool)
        .await?;
    let Some(action_row) = action_row else {
        return Ok(None);
    };
    let action = stored_review_inbox_action_from_row(&action_row)?;

    let evidence_rows = sqlx::query(LOAD_REVIEW_DECISION_EVIDENCE)
        .bind(request.tenant_id)
        .bind(request.user_id)
        .bind(request.proposed_action_id)
        .bind(request.proposed_action_version as i64)
        .bind(request.expected_sync_cursor_value as i64)
        .fetch_all(pool)
        .await?;
    let evidence = evidence_rows
        .iter()
        .map(stored_review_inbox_evidence_from_row)
        .collect::<PgRepositoryResult<Vec<_>>>()?;

    Ok(Some(StoredReviewDecisionContext {
        item,
        action,
        evidence,
    }))
}
