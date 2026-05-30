use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct StoredPendingConfirmedAction {
    pub request: crate::action::execution_request::ConfirmedExecutionRequest,
    pub operation: OperationRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresExecutionRecorderReport {
    pub operation: OperationRecord,
    pub outbox_id: Option<i64>,
    pub inbox_item_id: Option<String>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresReviewDecisionRecorderReport {
    pub operation: Option<OperationRecord>,
    pub inbox_item_id: Option<String>,
    pub outbox_id: Option<i64>,
    pub duplicate: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresReviewDecisionRecorderRequest<'a> {
    pub expected_sync_cursor_value: u64,
    pub decision: InsertProposedActionDecisionRequest<'a>,
    pub confirmed_action: Option<&'a ConfirmedAction>,
    pub confirmed_at_ms: Option<u64>,
    pub operation_id: Option<&'a str>,
    pub inbox_item: &'a ReviewInboxItem,
    pub event: &'a AuditEvent,
    pub outbox: &'a AuditOutboxEnvelope,
}
