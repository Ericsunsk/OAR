use std::collections::BTreeSet;

use oar_lark_adapter::{FeishuOkrProgressListTarget, OkrReadKeyResult, OkrReadObjective};

use super::super::okr_topology::OkrTopologySnapshot;
use super::aggregation::OkrProgressAggregation;
use super::text::{non_empty_compact, short_title, short_value};

pub(super) const CYCLE_EXPAND_LIMIT: usize = 3;
const OBJECTIVE_DISCOVERY_LIMIT: usize = 10;
const KEY_RESULT_DISCOVERY_LIMIT: usize = 20;
const PROGRESS_TARGET_LIMIT: usize = 12;

pub(super) fn discover_progress_targets(
    snapshot: &OkrTopologySnapshot,
    aggregation: &mut OkrProgressAggregation,
) -> Vec<ProgressTarget> {
    let mut targets = Vec::new();
    let mut seen_target_keys = BTreeSet::new();
    let mut objective_discovery_count = 0_usize;
    let mut key_result_discovery_count = 0_usize;

    for topology_cycle in snapshot.cycles.iter().take(CYCLE_EXPAND_LIMIT) {
        if topology_cycle.stable_cycle_id().is_none() {
            aggregation.skipped_missing_ids += 1;
            continue;
        }
        aggregation.cycles_expanded += 1;
        let Some(objectives) = topology_cycle.objectives.as_ref() else {
            continue;
        };
        aggregation.objectives_seen += objectives.len();
        aggregation.objective_pages_with_more += usize::from(topology_cycle.objectives_has_more);

        for objective in objectives {
            if objective_discovery_count >= OBJECTIVE_DISCOVERY_LIMIT {
                aggregation.skipped_objectives += 1;
                continue;
            }
            objective_discovery_count += 1;

            let objective_target = match ProgressTarget::from_objective(objective) {
                Some(target) => target,
                None => {
                    aggregation.skipped_missing_ids += 1;
                    continue;
                }
            };
            push_progress_target(
                objective_target.clone(),
                &mut targets,
                &mut seen_target_keys,
                aggregation,
            );

            let Some(krs_page) = topology_cycle.key_results_for_objective(&objective_target.id)
            else {
                continue;
            };
            aggregation.key_results_seen += krs_page.krs.len();
            aggregation.key_result_pages_with_more += usize::from(krs_page.has_more);

            for kr in &krs_page.krs {
                if key_result_discovery_count >= KEY_RESULT_DISCOVERY_LIMIT {
                    aggregation.skipped_key_results += 1;
                    continue;
                }
                key_result_discovery_count += 1;
                let Some(kr_target) = ProgressTarget::from_key_result(kr) else {
                    aggregation.skipped_missing_ids += 1;
                    continue;
                };
                push_progress_target(kr_target, &mut targets, &mut seen_target_keys, aggregation);
            }
        }
    }

    targets
}

fn push_progress_target(
    target: ProgressTarget,
    targets: &mut Vec<ProgressTarget>,
    seen_target_keys: &mut BTreeSet<String>,
    aggregation: &mut OkrProgressAggregation,
) {
    let key = target.dedupe_key();
    if !seen_target_keys.insert(key) {
        return;
    }
    if targets.len() >= PROGRESS_TARGET_LIMIT {
        aggregation.skipped_progress_targets += 1;
        return;
    }
    targets.push(target);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProgressTargetKind {
    Objective,
    KeyResult,
}

impl ProgressTargetKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Objective => "O",
            Self::KeyResult => "KR",
        }
    }

    fn dedupe_prefix(self) -> &'static str {
        match self {
            Self::Objective => "o",
            Self::KeyResult => "kr",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ProgressTarget {
    pub(super) kind: ProgressTargetKind,
    pub(super) id: String,
    pub(super) title: Option<String>,
    pub(super) percent: Option<String>,
    pub(super) status: Option<String>,
    pub(super) modify_time: Option<String>,
}

impl ProgressTarget {
    fn from_objective(objective: &OkrReadObjective) -> Option<Self> {
        Some(Self {
            kind: ProgressTargetKind::Objective,
            id: non_empty_compact(objective.objective_id.as_deref())?,
            title: objective.content.as_deref().and_then(short_title),
            percent: objective.progress.as_deref().and_then(short_value),
            status: objective.status.as_deref().and_then(short_value),
            modify_time: objective.last_updated_time.as_deref().and_then(short_value),
        })
    }

    fn from_key_result(kr: &OkrReadKeyResult) -> Option<Self> {
        Some(Self {
            kind: ProgressTargetKind::KeyResult,
            id: non_empty_compact(kr.kr_id.as_deref())?,
            title: kr.content.as_deref().and_then(short_title),
            percent: kr.progress.as_deref().and_then(short_value),
            status: kr.status.as_deref().and_then(short_value),
            modify_time: kr.last_updated_time.as_deref().and_then(short_value),
        })
    }

    fn dedupe_key(&self) -> String {
        format!("{}:{}", self.kind.dedupe_prefix(), self.id)
    }

    pub(super) fn to_adapter_target(&self) -> FeishuOkrProgressListTarget {
        match self.kind {
            ProgressTargetKind::Objective => {
                FeishuOkrProgressListTarget::Objective(self.id.clone())
            }
            ProgressTargetKind::KeyResult => {
                FeishuOkrProgressListTarget::KeyResult(self.id.clone())
            }
        }
    }
}
