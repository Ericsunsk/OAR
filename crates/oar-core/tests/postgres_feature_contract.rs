use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_RETRYABLE,
    MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_SENT,
    MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT,
};
use oar_core::storage::postgres::device_session_sql::{
    ADVANCE_DEVICE_SESSION_CURSOR_CAS, EXPIRE_DEVICE_SESSION, GET_DEVICE_SESSION_BY_ID,
    REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};
use oar_core::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_TENANT_BY_ID,
    GET_WORKSPACE_USER_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_TENANT, UPSERT_WORKSPACE_USER,
};
use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION, MARK_EXECUTING,
    MARK_FAILED, MARK_SUCCEEDED, SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use oar_core::storage::postgres::review_inbox_sql::{
    INSERT_EVIDENCE_ITEM, INSERT_PROPOSED_ACTION, INSERT_PROPOSED_ACTION_DECISION,
    INSERT_PROPOSED_ACTION_EVIDENCE_REF, LIST_REVIEW_INBOX_ITEMS,
    LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT, UPDATE_REVIEW_INBOX_LEDGER_PROJECTION,
    UPSERT_REVIEW_INBOX_ITEM,
};
use oar_core::storage::postgres::scheduler_sql::{
    CLAIM_SCHEDULER_JOB, COMPLETE_SCHEDULER_JOB_FOR_LEASE, FAIL_SCHEDULER_JOB_FOR_LEASE,
    GET_SCHEDULER_JOB, UPSERT_SCHEDULER_JOB,
};
use oar_core::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED,
    REVOKE_TOKEN_GRANT, ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};

fn compact(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn default_build_exposes_postgres_sql_contract_constants() {
    let operation_sql = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);
    let execution_queue_sql = compact(LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION);
    let transition_sql = compact(MARK_EXECUTING);
    let audit_sql = compact(APPEND_AUDIT_EVENT);
    let claim_outbox_sql = compact(CLAIM_AUDIT_OUTBOX);
    let rotate_grant_sql = compact(ROTATE_TOKEN_GRANT);
    let device_session_sql = compact(UPSERT_DEVICE_SESSION);
    let tenant_sql = compact(UPSERT_TENANT);
    let evidence_sql = compact(INSERT_EVIDENCE_ITEM);
    let review_inbox_sql = compact(UPSERT_REVIEW_INBOX_ITEM);
    let review_inbox_projection_sql = compact(UPDATE_REVIEW_INBOX_LEDGER_PROJECTION);
    let review_inbox_ledger_sql = compact(LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT);
    let scheduler_claim_sql = compact(CLAIM_SCHEDULER_JOB);

    assert!(operation_sql.contains("insert into confirmed_actions"));
    assert!(operation_sql.contains("insert into operation_ledger"));
    assert!(operation_sql.contains("true as created"));
    assert!(operation_sql.contains("false as created"));
    assert!(execution_queue_sql.contains("operation_ledger.status = 'confirmed'"));
    assert!(execution_queue_sql.contains("join confirmed_actions"));
    assert!(transition_sql.contains("update operation_ledger"));
    assert!(audit_sql.contains("insert into audit_events"));
    assert!(claim_outbox_sql.contains("for update skip locked"));
    assert!(rotate_grant_sql.contains("update token_grants"));
    assert!(rotate_grant_sql.contains("oauth_grant_fingerprint = $3"));
    assert!(rotate_grant_sql.contains("revoked_at is null"));
    assert!(rotate_grant_sql.contains("reauth_required_at is null"));
    assert!(device_session_sql.contains("insert into device_sessions"));
    assert!(device_session_sql.contains("session_identity_hash"));
    assert!(tenant_sql.contains("insert into tenants"));
    assert!(tenant_sql.contains("on conflict (id) do update"));
    assert!(evidence_sql.contains("insert into evidence_items"));
    assert!(review_inbox_sql.contains("insert into review_inbox_items"));
    assert!(review_inbox_projection_sql.contains("update review_inbox_items"));
    assert!(review_inbox_ledger_sql.contains("from unioned_events"));
    assert!(scheduler_claim_sql.contains("for update skip locked"));

    // Touch all constants to lock import visibility for default builds.
    let _ = MARK_SUCCEEDED;
    let _ = MARK_FAILED;
    let _ = GET_BY_IDEMPOTENCY_KEY;
    let _ = FIND_AUDIT_EVENTS_BY_TRACE_ID;
    let _ = ENQUEUE_AUDIT_OUTBOX;
    let _ = MARK_AUDIT_OUTBOX_SENT;
    let _ = MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT;
    let _ = MARK_AUDIT_OUTBOX_RETRYABLE;
    let _ = MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT;
    let _ = MARK_AUDIT_OUTBOX_FAILED;
    let _ = MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT;
    let _ = UPSERT_TOKEN_GRANT;
    let _ = GET_TOKEN_GRANT_BY_ID;
    let _ = MARK_TOKEN_GRANT_REFRESH_FAILED;
    let _ = MARK_TOKEN_GRANT_REAUTH_REQUIRED;
    let _ = REVOKE_TOKEN_GRANT;
    let _ = ADVANCE_DEVICE_SESSION_CURSOR_CAS;
    let _ = GET_DEVICE_SESSION_BY_ID;
    let _ = REVOKE_DEVICE_SESSION;
    let _ = EXPIRE_DEVICE_SESSION;
    let _ = UPSERT_TENANT;
    let _ = GET_TENANT_BY_ID;
    let _ = UPSERT_WORKSPACE_USER;
    let _ = GET_WORKSPACE_USER_BY_ID;
    let _ = UPSERT_LARK_IDENTITY;
    let _ = GET_LARK_IDENTITY_BY_ID;
    let _ = GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL;
    let _ = INSERT_PROPOSED_ACTION;
    let _ = INSERT_PROPOSED_ACTION_EVIDENCE_REF;
    let _ = INSERT_PROPOSED_ACTION_DECISION;
    let _ = LIST_REVIEW_INBOX_ITEMS;
    let _ = LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT;
    let _ = UPDATE_REVIEW_INBOX_LEDGER_PROJECTION;
    let _ = UPSERT_SCHEDULER_JOB;
    let _ = GET_SCHEDULER_JOB;
    let _ = COMPLETE_SCHEDULER_JOB_FOR_LEASE;
    let _ = FAIL_SCHEDULER_JOB_FOR_LEASE;
}

#[cfg(feature = "postgres")]
#[path = "postgres_feature_contract/postgres_feature_api_contract.rs"]
mod postgres_feature_api_contract;
