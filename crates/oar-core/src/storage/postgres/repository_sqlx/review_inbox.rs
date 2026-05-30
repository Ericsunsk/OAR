use super::*;

impl PostgresReviewInboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn insert_evidence_item(
        &self,
        tenant_id: &str,
        item: &EvidenceItem,
    ) -> PgRepositoryResult<Option<StoredEvidenceItem>> {
        let row = sqlx::query(INSERT_EVIDENCE_ITEM)
            .bind(&item.id.0)
            .bind(tenant_id)
            .bind(&item.summary)
            .bind(evidence_source_kind_to_db(&item.reference.source_kind))
            .bind(&item.reference.source_id)
            .bind(&item.reference.locator)
            .bind(&item.content_hash)
            .bind(evidence_visibility_scope_to_db(&item.visibility))
            .bind(system_time_to_ms(item.observed_at)? as i64)
            .bind(system_time_to_ms(item.recorded_at)? as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_evidence_item_from_row).transpose()
    }

    pub async fn insert_proposed_action(
        &self,
        action: &ProposedAction,
        published_at: Option<SystemTime>,
    ) -> PgRepositoryResult<bool> {
        let (kind, custom_kind) = proposed_action_kind_to_db(&action.kind);
        let row = sqlx::query(INSERT_PROPOSED_ACTION)
            .bind(&action.id.0)
            .bind(&action.tenant_id.0)
            .bind(&action.actor_user_id.0)
            .bind(action.target_user_id.as_ref().map(|id| id.0.as_str()))
            .bind(action.owner_user_id.as_ref().map(|id| id.0.as_str()))
            .bind(action.version as i64)
            .bind(proposed_action_status_to_db(&action.status))
            .bind(kind)
            .bind(custom_kind)
            .bind(risk_severity_to_db(&action.risk_severity))
            .bind(&action.suggested_payload)
            .bind(option_system_time_to_i64_ms(published_at)?)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn insert_proposed_action_evidence_ref(
        &self,
        tenant_id: &str,
        proposed_action_id: &str,
        version: u64,
        evidence_id: &str,
    ) -> PgRepositoryResult<()> {
        sqlx::query(INSERT_PROPOSED_ACTION_EVIDENCE_REF)
            .bind(proposed_action_id)
            .bind(evidence_id)
            .bind(tenant_id)
            .bind(version as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_proposed_action_decision(
        &self,
        request: InsertProposedActionDecisionRequest<'_>,
    ) -> PgRepositoryResult<bool> {
        let row = insert_proposed_action_decision_query(request)?
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn upsert_review_inbox_item(
        &self,
        item: &ReviewInboxItem,
    ) -> PgRepositoryResult<Option<String>> {
        let row = upsert_review_inbox_item_query(item)?
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.as_ref().map(|row| row.try_get("id")).transpose()?)
    }

    pub async fn list_review_inbox_items(
        &self,
        tenant_id: &str,
        user_id: &str,
        after_cursor: u64,
        limit: u32,
    ) -> PgRepositoryResult<Vec<StoredReviewInboxItem>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(LIST_REVIEW_INBOX_ITEMS)
            .bind(tenant_id)
            .bind(user_id)
            .bind(after_cursor as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(stored_review_inbox_item_from_row).collect()
    }

    pub async fn load_review_inbox_snapshot(
        &self,
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
            });
        }

        let item_rows = sqlx::query(LIST_REVIEW_INBOX_ITEMS)
            .bind(tenant_id)
            .bind(user_id)
            .bind(after_cursor as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
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
            });
        }

        let action_rows = sqlx::query(LIST_REVIEW_INBOX_ACTIONS_FOR_SNAPSHOT)
            .bind(tenant_id)
            .bind(user_id)
            .bind(after_cursor as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
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
            .fetch_all(&self.pool)
            .await?;
        let evidence = evidence_rows
            .iter()
            .map(stored_review_inbox_evidence_from_row)
            .collect::<PgRepositoryResult<Vec<_>>>()?;

        Ok(StoredReviewInboxSnapshot {
            items,
            actions,
            evidence,
        })
    }

    pub async fn load_review_decision_context(
        &self,
        request: PostgresReviewDecisionContextRequest<'_>,
    ) -> PgRepositoryResult<Option<StoredReviewDecisionContext>> {
        load_review_decision_context_from_pool(&self.pool, request).await
    }
}

pub(super) async fn load_review_decision_context_from_pool(
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

pub(super) async fn insert_proposed_action_decision_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    request: InsertProposedActionDecisionRequest<'_>,
) -> PgRepositoryResult<bool> {
    let row = insert_proposed_action_decision_query(request)?
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.is_some())
}

pub(super) async fn update_review_inbox_decision_state_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    item: &ReviewInboxItem,
    expected_sync_cursor_value: u64,
) -> PgRepositoryResult<Option<String>> {
    let updated_at_ms = system_time_to_ms(item.updated_at)? as i64;
    let row = sqlx::query(UPDATE_REVIEW_INBOX_DECISION_STATE)
        .bind(&item.tenant_id.0)
        .bind(&item.user_id.0)
        .bind(&item.proposed_action_id)
        .bind(item.proposed_action_version as i64)
        .bind(expected_sync_cursor_value as i64)
        .bind(review_inbox_item_status_to_db(&item.status))
        .bind(updated_at_ms)
        .bind(item.ledger_status.as_deref())
        .bind(item.operation_id.as_deref())
        .fetch_optional(&mut **tx)
        .await?;

    Ok(row.as_ref().map(|row| row.try_get("id")).transpose()?)
}

pub(super) async fn update_review_inbox_ledger_projection_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    operation: &OperationRecord,
    ledger_status: ActionStatus,
    now_ms: u64,
) -> PgRepositoryResult<Option<String>> {
    let Some(inbox_status) = review_inbox_status_for_ledger_status(ledger_status) else {
        return Ok(None);
    };

    let row = sqlx::query(UPDATE_REVIEW_INBOX_LEDGER_PROJECTION)
        .bind(&operation.tenant_id)
        .bind(&operation.operation_id)
        .bind(review_inbox_item_status_to_db(&inbox_status))
        .bind(action_status_to_db(&ledger_status))
        .bind(now_ms as i64)
        .fetch_optional(&mut **tx)
        .await?;

    Ok(row.as_ref().map(|row| row.try_get("id")).transpose()?)
}

fn review_inbox_status_for_ledger_status(status: ActionStatus) -> Option<ReviewInboxItemStatus> {
    match status {
        ActionStatus::Confirmed | ActionStatus::Proposed => None,
        ActionStatus::Executing => Some(ReviewInboxItemStatus::Executing),
        ActionStatus::Succeeded => Some(ReviewInboxItemStatus::Succeeded),
        ActionStatus::Failed | ActionStatus::Cancelled => Some(ReviewInboxItemStatus::Failed),
    }
}

fn insert_proposed_action_decision_query(
    request: InsertProposedActionDecisionRequest<'_>,
) -> PgRepositoryResult<sqlx::query::Query<'_, Postgres, sqlx::postgres::PgArguments>> {
    let (decision, edited_payload) = proposed_action_decision_to_db(request.decision);
    let decided_at_ms = system_time_to_ms(request.decided_at)? as i64;
    Ok(sqlx::query(INSERT_PROPOSED_ACTION_DECISION)
        .bind(request.id)
        .bind(request.tenant_id)
        .bind(request.proposed_action_id)
        .bind(request.proposed_action_version as i64)
        .bind(request.actor_user_id)
        .bind(decision)
        .bind(edited_payload)
        .bind(request.confirmed_action_id)
        .bind(decided_at_ms))
}

fn upsert_review_inbox_item_query(
    item: &ReviewInboxItem,
) -> PgRepositoryResult<sqlx::query::Query<'_, Postgres, sqlx::postgres::PgArguments>> {
    let updated_at_ms = system_time_to_ms(item.updated_at)? as i64;
    Ok(sqlx::query(UPSERT_REVIEW_INBOX_ITEM)
        .bind(&item.id.0)
        .bind(&item.tenant_id.0)
        .bind(&item.user_id.0)
        .bind(&item.proposed_action_id)
        .bind(item.proposed_action_version as i64)
        .bind(item.risk_score as i32)
        .bind(item.priority as i32)
        .bind(review_inbox_item_status_to_db(&item.status))
        .bind(item.sort_key)
        .bind(item.sync_cursor as i64)
        .bind(updated_at_ms)
        .bind(item.ledger_status.as_deref())
        .bind(item.operation_id.as_deref()))
}
