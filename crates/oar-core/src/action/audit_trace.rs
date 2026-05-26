use super::audit_event::{AuditEvent, AuditEventContext, AuditStateSummary, AuditSubject};

/// Event builder for a single action execution trace.
/// Owns trace identity and allocates strictly increasing event sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTrace {
    trace_id: String,
    subject: AuditSubject,
    next_sequence: u64,
}

impl AuditTrace {
    pub fn new(trace_id: impl Into<String>, subject: AuditSubject) -> Self {
        Self {
            trace_id: trace_id.into(),
            subject,
            next_sequence: 1,
        }
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn confirmed_action(
        &mut self,
        occurred_at_ms: u64,
        after: AuditStateSummary,
    ) -> AuditEvent {
        let context = self.context(occurred_at_ms);
        AuditEvent::confirmed_action(context, after)
    }

    pub fn dry_run(
        &mut self,
        occurred_at_ms: u64,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> AuditEvent {
        let context = self.context(occurred_at_ms);
        AuditEvent::dry_run(context, before, after)
    }

    pub fn execution_succeeded(
        &mut self,
        occurred_at_ms: u64,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: impl Into<String>,
    ) -> AuditEvent {
        let context = self.context(occurred_at_ms);
        AuditEvent::execution_succeeded(context, before, after, adapter_operation_id)
    }

    pub fn execution_denied(
        &mut self,
        occurred_at_ms: u64,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> AuditEvent {
        let context = self.context(occurred_at_ms);
        AuditEvent::execution_denied(context, error_code, message)
    }

    pub fn execution_failed(
        &mut self,
        occurred_at_ms: u64,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> AuditEvent {
        let context = self.context(occurred_at_ms);
        AuditEvent::execution_failed(context, before, after, error_code, message)
    }

    fn context(&mut self, occurred_at_ms: u64) -> AuditEventContext {
        let sequence = self.allocate_sequence();
        AuditEventContext {
            event_id: self.event_id_for(sequence),
            trace_id: self.trace_id.clone(),
            sequence,
            occurred_at_ms,
            subject: self.subject.clone(),
        }
    }

    fn allocate_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }

    fn event_id_for(&self, sequence: u64) -> String {
        format!("{}-evt-{}", self.trace_id, sequence)
    }
}
