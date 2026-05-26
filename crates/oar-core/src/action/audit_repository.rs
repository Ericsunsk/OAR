use std::sync::{Arc, Mutex};

use super::audit_event::AuditEvent;

pub trait AuditEventRepository {
    fn append(&self, event: AuditEvent) -> Result<(), AuditRepositoryError>;
    fn find_by_trace_id(&self, trace_id: &str) -> Vec<AuditEvent>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditRepositoryError {
    DuplicateEventId(String),
    StoreUnavailable,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAuditEventRepository {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl InMemoryAuditEventRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&self, event: AuditEvent) -> Result<(), AuditRepositoryError> {
        <Self as AuditEventRepository>::append(self, event)
    }

    pub fn find_by_trace_id(&self, trace_id: &str) -> Vec<AuditEvent> {
        <Self as AuditEventRepository>::find_by_trace_id(self, trace_id)
    }
}

impl AuditEventRepository for InMemoryAuditEventRepository {
    fn append(&self, event: AuditEvent) -> Result<(), AuditRepositoryError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| AuditRepositoryError::StoreUnavailable)?;

        if events
            .iter()
            .any(|existing| existing.event_id == event.event_id)
        {
            return Err(AuditRepositoryError::DuplicateEventId(event.event_id));
        }

        events.push(event);
        Ok(())
    }

    fn find_by_trace_id(&self, trace_id: &str) -> Vec<AuditEvent> {
        let Ok(events) = self.events.lock() else {
            return Vec::new();
        };

        let mut matching: Vec<_> = events
            .iter()
            .filter(|event| event.trace_id == trace_id)
            .cloned()
            .collect();
        matching.sort_by_key(|event| event.sequence);
        matching
    }
}
