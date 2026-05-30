use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxMessage {
    pub id: i64,
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub next_attempt_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxEnvelope {
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub next_attempt_at_ms: u64,
}
