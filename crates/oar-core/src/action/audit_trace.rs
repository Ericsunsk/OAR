use super::audit_event::{AuditActor, AuditEvent, AuditScope, AuditStateSummary, AuditTarget};

/// Event builder for a single action execution trace.
/// Owns trace identity and allocates strictly increasing event sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTrace {
    trace_id: String,
    next_sequence: u64,
}

impl AuditTrace {
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            next_sequence: 1,
        }
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn confirmed_action(
        &mut self,
        occurred_at_ms: u64,
        actor: AuditActor,
        scope: AuditScope,
        target: AuditTarget,
        after: AuditStateSummary,
    ) -> AuditEvent {
        let sequence = self.allocate_sequence();
        AuditEvent::confirmed_action(
            self.event_id_for(sequence),
            self.trace_id.clone(),
            sequence,
            occurred_at_ms,
            actor,
            scope,
            target,
            after,
        )
    }

    pub fn dry_run(
        &mut self,
        occurred_at_ms: u64,
        actor: AuditActor,
        scope: AuditScope,
        target: AuditTarget,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> AuditEvent {
        let sequence = self.allocate_sequence();
        AuditEvent::dry_run(
            self.event_id_for(sequence),
            self.trace_id.clone(),
            sequence,
            occurred_at_ms,
            actor,
            scope,
            target,
            before,
            after,
        )
    }

    pub fn execution_succeeded(
        &mut self,
        occurred_at_ms: u64,
        actor: AuditActor,
        scope: AuditScope,
        target: AuditTarget,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: impl Into<String>,
    ) -> AuditEvent {
        let sequence = self.allocate_sequence();
        AuditEvent::execution_succeeded(
            self.event_id_for(sequence),
            self.trace_id.clone(),
            sequence,
            occurred_at_ms,
            actor,
            scope,
            target,
            before,
            after,
            adapter_operation_id,
        )
    }

    pub fn execution_failed(
        &mut self,
        occurred_at_ms: u64,
        actor: AuditActor,
        scope: AuditScope,
        target: AuditTarget,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> AuditEvent {
        let sequence = self.allocate_sequence();
        AuditEvent::execution_failed(
            self.event_id_for(sequence),
            self.trace_id.clone(),
            sequence,
            occurred_at_ms,
            actor,
            scope,
            target,
            before,
            after,
            error_code,
            message,
        )
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
