use oar_lark_adapter::{OkrReadCycle, OkrReadKeyResult, OkrReadObjective};

#[derive(Clone)]
pub(in crate::agent::live_context) enum OkrTopologyRead {
    EmptyData,
    Snapshot(OkrTopologySnapshot),
}

#[derive(Clone, Default)]
pub(in crate::agent::live_context) struct OkrTopologySnapshot {
    pub(in crate::agent::live_context) cycles: Vec<OkrTopologyCycle>,
    pub(in crate::agent::live_context) has_more_cycles: bool,
}

#[derive(Clone)]
pub(in crate::agent::live_context) struct OkrTopologyCycle {
    pub(in crate::agent::live_context) cycle: OkrReadCycle,
    pub(in crate::agent::live_context) objectives: Option<Vec<OkrReadObjective>>,
    pub(in crate::agent::live_context) objectives_has_more: bool,
    pub(in crate::agent::live_context) key_results: Vec<OkrTopologyKeyResults>,
}

impl OkrTopologyCycle {
    pub(in crate::agent::live_context) fn stable_cycle_id(&self) -> Option<&str> {
        self.cycle
            .cycle_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
    }

    pub(in crate::agent::live_context) fn key_results_for_objective(
        &self,
        objective_id: &str,
    ) -> Option<&OkrTopologyKeyResults> {
        self.key_results
            .iter()
            .find(|entry| entry.objective_id == objective_id)
    }
}

#[derive(Clone)]
pub(in crate::agent::live_context) struct OkrTopologyKeyResults {
    pub(in crate::agent::live_context) objective_id: String,
    pub(in crate::agent::live_context) krs: Vec<OkrReadKeyResult>,
    pub(in crate::agent::live_context) has_more: bool,
}
