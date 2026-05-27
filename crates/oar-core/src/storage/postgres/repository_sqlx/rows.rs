use super::codec::{
    action_status_from_db, audit_actor_kind_from_db, audit_event_type_from_db,
    device_entry_point_from_db, device_session_state_from_db, evidence_source_kind_from_db,
    evidence_visibility_scope_from_db, identity_actor_kind_from_db, oar_user_status_from_db,
    proposed_action_decision_from_db, proposed_action_kind_from_db, proposed_action_status_from_db,
    review_inbox_item_status_from_db, risk_severity_from_db, scheduler_job_kind_from_db,
    scheduler_job_status_from_db, scope_boundary_from_db, tenant_status_from_db,
    token_grant_state_from_db,
};
use super::util::{
    json_value_option, ms_to_system_time, non_negative_i64_to_u64, optional_non_negative_i64_to_u64,
};
use super::{
    AuditActor, AuditEvent, AuditOutboxMessage, AuditScope, AuditTarget, EncryptedTokenGrantRecord,
    OperationRecord, PgRepositoryResult, StoredDeviceSession, StoredEvidenceItem,
    StoredLarkIdentity, StoredOarUser, StoredProposedAction, StoredProposedActionDecision,
    StoredReviewInboxItem, StoredSchedulerJob, StoredTenant,
};
use crate::domain::identity::{TenantId, TokenGrantId};
use crate::domain::token_refresh::types::TokenRefreshGrantSnapshot;
use sqlx::postgres::PgRow;
use sqlx::Row;

pub(super) fn operation_record_from_row(row: &PgRow) -> PgRepositoryResult<OperationRecord> {
    let status: String = row.try_get("status")?;
    Ok(OperationRecord {
        operation_id: row.try_get("operation_id")?,
        tenant_id: row.try_get("tenant_id")?,
        action_id: row.try_get("action_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        status: action_status_from_db(&status)?,
        last_error: row.try_get("last_error")?,
    })
}

pub(super) fn audit_event_from_row(row: &PgRow) -> PgRepositoryResult<AuditEvent> {
    let sequence = non_negative_i64_to_u64(row.try_get("sequence")?, "sequence")?;
    let occurred_at_ms = non_negative_i64_to_u64(row.try_get("occurred_at_ms")?, "occurred_at_ms")?;
    let actor_kind: String = row.try_get("actor_kind")?;
    let event_type: String = row.try_get("event_type")?;

    Ok(AuditEvent {
        event_id: row.try_get("event_id")?,
        trace_id: row.try_get("trace_id")?,
        sequence,
        occurred_at_ms,
        event_type: audit_event_type_from_db(&event_type)?,
        actor: AuditActor {
            kind: audit_actor_kind_from_db(&actor_kind)?,
            actor_id: row.try_get("actor_id")?,
            display_name: row.try_get("actor_display_name")?,
        },
        scope: AuditScope {
            tenant_id: row.try_get("tenant_id")?,
            workspace_id: None,
        },
        target: AuditTarget {
            resource_type: row.try_get("target_resource_type")?,
            resource_id: row.try_get("target_resource_id")?,
            action_type: row.try_get("target_action_type")?,
        },
        before: json_value_option(row.try_get("before_summary")?)?,
        after: json_value_option(row.try_get("after_summary")?)?,
        execution: json_value_option(row.try_get("execution_result")?)?,
    })
}

pub(super) fn audit_outbox_message_from_row(row: &PgRow) -> PgRepositoryResult<AuditOutboxMessage> {
    Ok(AuditOutboxMessage {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        stream: row.try_get("stream")?,
        aggregate_id: row.try_get("aggregate_id")?,
        payload: row.try_get("payload")?,
        attempt_count: row.try_get("attempt_count")?,
        next_attempt_at_ms: row.try_get("next_attempt_at_ms")?,
    })
}

pub(super) fn encrypted_token_grant_from_row(
    row: &PgRow,
) -> PgRepositoryResult<EncryptedTokenGrantRecord> {
    let actor_kind: String = row.try_get("actor_kind")?;
    let scope_boundary: String = row.try_get("scope_boundary")?;
    let state: String = row.try_get("state")?;

    Ok(EncryptedTokenGrantRecord {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        identity_id: row.try_get("identity_id")?,
        actor_kind: identity_actor_kind_from_db(&actor_kind)?,
        scope_boundary: scope_boundary_from_db(&scope_boundary)?,
        scopes: row.try_get("scopes")?,
        state: token_grant_state_from_db(&state)?,
        issued_at_ms: non_negative_i64_to_u64(row.try_get("issued_at_ms")?, "issued_at_ms")?,
        expires_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("expires_at_ms")?,
            "expires_at_ms",
        )?,
        refreshed_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("refreshed_at_ms")?,
            "refreshed_at_ms",
        )?,
        revoked_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?,
        reauth_required_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?,
        last_refresh_error: row.try_get("last_refresh_error")?,
        encrypted_oauth_grant: row.try_get("encrypted_oauth_grant")?,
        oauth_grant_key_id: row.try_get("oauth_grant_key_id")?,
        oauth_grant_fingerprint: row.try_get("oauth_grant_fingerprint")?,
        revocation_reason: row.try_get("revocation_reason")?,
    })
}

pub(super) fn token_refresh_snapshot_from_row(
    row: &PgRow,
) -> PgRepositoryResult<TokenRefreshGrantSnapshot> {
    let state: String = row.try_get("state")?;
    Ok(TokenRefreshGrantSnapshot {
        grant_id: TokenGrantId(row.try_get("id")?),
        tenant_id: TenantId(row.try_get("tenant_id")?),
        expected_fingerprint: row.try_get("oauth_grant_fingerprint")?,
        state: token_grant_state_from_db(&state)?,
        has_refresh_material: row.try_get("has_refresh_material")?,
        revoked_at: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?
        .map(ms_to_system_time),
        reauth_required_at: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?
        .map(ms_to_system_time),
    })
}

pub(super) fn stored_tenant_from_row(row: &PgRow) -> PgRepositoryResult<StoredTenant> {
    let status: String = row.try_get("status")?;
    Ok(StoredTenant {
        id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        status: tenant_status_from_db(&status)?,
    })
}

pub(super) fn stored_oar_user_from_row(row: &PgRow) -> PgRepositoryResult<StoredOarUser> {
    let status: String = row.try_get("status")?;
    Ok(StoredOarUser {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        display_name: row.try_get("display_name")?,
        status: oar_user_status_from_db(&status)?,
    })
}

pub(super) fn stored_lark_identity_from_row(row: &PgRow) -> PgRepositoryResult<StoredLarkIdentity> {
    let actor_kind: String = row.try_get("actor_kind")?;
    Ok(StoredLarkIdentity {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        actor_kind: identity_actor_kind_from_db(&actor_kind)?,
        actor_external_id: row.try_get("actor_external_id")?,
        display_name: row.try_get("display_name")?,
    })
}

pub(super) fn stored_device_session_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredDeviceSession> {
    let entry_point: String = row.try_get("entry_point")?;
    let state: String = row.try_get("state")?;

    Ok(StoredDeviceSession {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        user_id: row.try_get("user_id")?,
        entry_point: device_entry_point_from_db(&entry_point)?,
        state: device_session_state_from_db(&state)?,
        sync_stream: row.try_get("sync_stream")?,
        sync_cursor_value: non_negative_i64_to_u64(
            row.try_get("sync_cursor_value")?,
            "sync_cursor_value",
        )?,
        sync_cursor_updated_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("sync_cursor_updated_at_ms")?,
            "sync_cursor_updated_at_ms",
        )?),
        session_identity_hash: row.try_get("session_identity_hash")?,
        last_seen_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("last_seen_at_ms")?,
            "last_seen_at_ms",
        )?),
        revoked_at: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?
        .map(ms_to_system_time),
        expired_at: optional_non_negative_i64_to_u64(
            row.try_get("expired_at_ms")?,
            "expired_at_ms",
        )?
        .map(ms_to_system_time),
    })
}

pub(super) fn stored_evidence_item_from_row(row: &PgRow) -> PgRepositoryResult<StoredEvidenceItem> {
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

#[allow(dead_code)]
pub(super) fn stored_proposed_action_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredProposedAction> {
    let status: String = row.try_get("status")?;
    let kind: String = row.try_get("kind")?;
    let risk_severity: String = row.try_get("risk_severity")?;
    let published_at_ms: Option<i64> = row.try_get("published_at_ms")?;

    Ok(StoredProposedAction {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        actor_user_id: row.try_get("actor_user_id")?,
        target_user_id: row.try_get("target_user_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        version: non_negative_i64_to_u64(row.try_get("version")?, "version")?,
        status: proposed_action_status_from_db(&status)?,
        kind: proposed_action_kind_from_db(&kind, row.try_get("custom_kind")?)?,
        risk_severity: risk_severity_from_db(&risk_severity)?,
        suggested_payload: row.try_get("suggested_payload")?,
        published_at: optional_non_negative_i64_to_u64(published_at_ms, "published_at_ms")?
            .map(ms_to_system_time),
    })
}

#[allow(dead_code)]
pub(super) fn stored_proposed_action_decision_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredProposedActionDecision> {
    let decision: String = row.try_get("decision")?;
    let edited_payload: Option<serde_json::Value> = row.try_get("edited_payload")?;
    Ok(StoredProposedActionDecision {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        proposed_action_id: row.try_get("proposed_action_id")?,
        proposed_action_version: non_negative_i64_to_u64(
            row.try_get("proposed_action_version")?,
            "proposed_action_version",
        )?,
        actor_user_id: row.try_get("actor_user_id")?,
        decision: proposed_action_decision_from_db(&decision, edited_payload)?,
        confirmed_action_id: row.try_get("confirmed_action_id")?,
        decided_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("decided_at_ms")?,
            "decided_at_ms",
        )?),
    })
}

pub(super) fn stored_review_inbox_item_from_row(
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

pub(super) fn stored_scheduler_job_from_row(row: &PgRow) -> PgRepositoryResult<StoredSchedulerJob> {
    let job_kind: String = row.try_get("job_kind")?;
    let status: String = row.try_get("status")?;
    Ok(StoredSchedulerJob {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        job_kind: scheduler_job_kind_from_db(&job_kind)?,
        status: scheduler_job_status_from_db(&status)?,
        next_run_at_ms: non_negative_i64_to_u64(row.try_get("next_run_at_ms")?, "next_run_at_ms")?,
        lease_id: row.try_get("lease_id")?,
        lease_until_ms: optional_non_negative_i64_to_u64(
            row.try_get("lease_until_ms")?,
            "lease_until_ms",
        )?,
        attempt_count: non_negative_i64_to_u64(
            row.try_get::<i32, _>("attempt_count")? as i64,
            "attempt_count",
        )? as u32,
        last_started_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("last_started_at_ms")?,
            "last_started_at_ms",
        )?,
        last_finished_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("last_finished_at_ms")?,
            "last_finished_at_ms",
        )?,
        last_safe_error_code: row.try_get("last_safe_error_code")?,
    })
}
