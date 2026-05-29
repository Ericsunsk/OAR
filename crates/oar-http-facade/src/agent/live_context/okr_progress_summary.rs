use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrProgressListRequest, FeishuOkrProgressListTarget,
    FeishuOkrReadError, OkrDepartmentIdType, OkrReadKeyResult, OkrReadObjective,
    OkrReadProgressPage, OkrReadProgressRecord, OkrUserIdType, SecretString,
};

use super::okr_topology::OkrTopologyRead;
use super::summary::{compact_text, finalize_summary, truncate_chars};

const TOOL_LABEL: &str = "工具 feishu.okr.summarize_my_progress";
const PROGRESS_PAGE_SIZE: u32 = 20;
const CYCLE_EXPAND_LIMIT: usize = 3;
const OBJECTIVE_DISCOVERY_LIMIT: usize = 10;
const KEY_RESULT_DISCOVERY_LIMIT: usize = 20;
const PROGRESS_TARGET_LIMIT: usize = 12;
const PROGRESS_EXAMPLE_LIMIT: usize = 3;
const TITLE_LIMIT: usize = 20;
const VALUE_LIMIT: usize = 24;
const STATUS_COUNT_LIMIT: usize = 4;

pub(super) async fn read_my_okr_progress_summary_from_topology<C>(
    okr_client: &mut C,
    access_token: SecretString,
    topology: &OkrTopologyRead,
) -> Result<String, FeishuOkrReadError>
where
    C: AsyncFeishuOkrRead,
{
    let OkrTopologyRead::Snapshot(snapshot) = topology else {
        return Ok(format!("{TOOL_LABEL}｜实时：Feishu 返回空数据。"));
    };
    let mut aggregation = OkrProgressAggregation {
        cycles_total: snapshot.cycles.len(),
        cycle_pages_with_more: usize::from(snapshot.has_more_cycles),
        skipped_cycles: snapshot.cycles.len().saturating_sub(CYCLE_EXPAND_LIMIT),
        ..OkrProgressAggregation::default()
    };

    if snapshot.cycles.is_empty() {
        return Ok(build_okr_progress_live_summary(&aggregation));
    }

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
                &mut aggregation,
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
                push_progress_target(
                    kr_target,
                    &mut targets,
                    &mut seen_target_keys,
                    &mut aggregation,
                );
            }
        }
    }

    for target in targets {
        let progress_response = okr_client
            .list_progress(FeishuOkrProgressListRequest {
                user_access_token: access_token.clone(),
                user_id_type: OkrUserIdType::OpenId,
                target: target.to_adapter_target(),
                page_size: Some(PROGRESS_PAGE_SIZE),
                page_token: None,
                department_id_type: OkrDepartmentIdType::OpenDepartmentId,
            })
            .await?;
        let page = progress_response
            .data
            .as_ref()
            .map(OkrReadProgressPage::from_progress_list_data)
            .unwrap_or_else(|| OkrReadProgressPage {
                progress_records: vec![],
                next_page_token: None,
                has_more: false,
            });
        aggregation.add_progress_page(&target, &page);
    }

    Ok(build_okr_progress_live_summary(&aggregation))
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

fn build_okr_progress_live_summary(aggregation: &OkrProgressAggregation) -> String {
    if aggregation.cycles_total == 0 {
        return format!("{TOOL_LABEL}｜实时：未读取到 OKR 周期。");
    }
    if aggregation.targets_read == 0 {
        return finalize_summary(format!(
            "{TOOL_LABEL}｜实时：读取到 {} 个 OKR 周期；未发现可读取进展的 Objective/KR target{}。",
            aggregation.cycles_total,
            skip_suffix(aggregation)
        ));
    }

    let status_suffix = if aggregation.status_counts.is_empty() {
        String::new()
    } else {
        format!(
            "；状态：{}",
            summarize_counts(&aggregation.status_counts, STATUS_COUNT_LIMIT)
        )
    };
    let examples_suffix = if aggregation.examples.is_empty() {
        String::new()
    } else {
        format!(
            "；示例：{}",
            summarize_progress_examples(&aggregation.examples, PROGRESS_EXAMPLE_LIMIT)
        )
    };

    finalize_summary(format!(
        "{TOOL_LABEL}｜实时：周期 {}/{}，Objective {}，KR {}，进展目标 {}，记录 {}{}{}{}。",
        aggregation.cycles_expanded,
        aggregation.cycles_total,
        aggregation.objectives_seen,
        aggregation.key_results_seen,
        aggregation.targets_read,
        aggregation.progress_records_read,
        status_suffix,
        skip_suffix(aggregation),
        examples_suffix
    ))
}

fn summarize_counts(counts: &BTreeMap<String, usize>, limit: usize) -> String {
    let mut parts = counts
        .iter()
        .take(limit)
        .map(|(status, count)| format!("{status} {count}"))
        .collect::<Vec<_>>();
    let skipped = counts.len().saturating_sub(limit);
    if skipped > 0 {
        parts.push(format!("另 {} 类", skipped));
    }
    parts.join("、")
}

fn summarize_progress_examples(examples: &[ProgressExample], limit: usize) -> String {
    let mut ordered = examples.iter().enumerate().collect::<Vec<_>>();
    ordered.sort_by(|(left_index, left), (right_index, right)| {
        compare_progress_examples_for_display(left, right)
            .unwrap_or_else(|| left_index.cmp(right_index))
    });
    ordered
        .into_iter()
        .take(limit)
        .map(|(_, example)| example.summary())
        .collect::<Vec<_>>()
        .join(" / ")
}

fn skip_suffix(aggregation: &OkrProgressAggregation) -> String {
    let mut parts = Vec::new();
    if aggregation.skipped_cycles > 0 {
        parts.push(format!("周期 {}", aggregation.skipped_cycles));
    }
    if aggregation.skipped_objectives > 0 {
        parts.push(format!("Objective {}", aggregation.skipped_objectives));
    }
    if aggregation.skipped_key_results > 0 {
        parts.push(format!("KR {}", aggregation.skipped_key_results));
    }
    if aggregation.skipped_progress_targets > 0 {
        parts.push(format!("进展目标 {}", aggregation.skipped_progress_targets));
    }
    if aggregation.progress_pages_with_more > 0 {
        parts.push(format!("进展分页 {}", aggregation.progress_pages_with_more));
    }
    let list_pages_with_more = aggregation.cycle_pages_with_more
        + aggregation.objective_pages_with_more
        + aggregation.key_result_pages_with_more;
    if list_pages_with_more > 0 {
        parts.push(format!("列表分页 {}", list_pages_with_more));
    }
    if aggregation.skipped_missing_ids > 0 {
        parts.push(format!("缺 ID {}", aggregation.skipped_missing_ids));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("；跳过/截断：{}", parts.join("、"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressTargetKind {
    Objective,
    KeyResult,
}

impl ProgressTargetKind {
    fn label(self) -> &'static str {
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
struct ProgressTarget {
    kind: ProgressTargetKind,
    id: String,
    title: Option<String>,
    percent: Option<String>,
    status: Option<String>,
    modify_time: Option<String>,
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

    fn to_adapter_target(&self) -> FeishuOkrProgressListTarget {
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

#[derive(Debug, Default)]
struct OkrProgressAggregation {
    cycles_total: usize,
    cycles_expanded: usize,
    objectives_seen: usize,
    key_results_seen: usize,
    targets_read: usize,
    progress_records_read: usize,
    skipped_cycles: usize,
    skipped_objectives: usize,
    skipped_key_results: usize,
    skipped_progress_targets: usize,
    skipped_missing_ids: usize,
    cycle_pages_with_more: usize,
    objective_pages_with_more: usize,
    key_result_pages_with_more: usize,
    progress_pages_with_more: usize,
    status_counts: BTreeMap<String, usize>,
    examples: Vec<ProgressExample>,
}

impl OkrProgressAggregation {
    fn add_progress_page(&mut self, target: &ProgressTarget, page: &OkrReadProgressPage) {
        self.targets_read += 1;
        self.progress_records_read += page.progress_records.len();
        if page.has_more {
            self.progress_pages_with_more += 1;
        }

        let representative = newest_record(&page.progress_records);
        let percent = representative
            .and_then(|record| record.percent.as_deref().and_then(short_value))
            .or_else(|| target.percent.clone());
        let status = representative
            .and_then(|record| record.status.as_deref().and_then(short_value))
            .or_else(|| target.status.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let modify_time = representative
            .and_then(|record| record.modify_time.as_deref().and_then(short_value))
            .or_else(|| target.modify_time.clone());

        *self.status_counts.entry(status.clone()).or_insert(0) += 1;
        self.examples.push(ProgressExample {
            kind: target.kind,
            title: target
                .title
                .clone()
                .unwrap_or_else(|| "未命名目标".to_string()),
            percent,
            status,
            modify_time,
        });
    }
}

#[derive(Debug, Clone)]
struct ProgressExample {
    kind: ProgressTargetKind,
    title: String,
    percent: Option<String>,
    status: String,
    modify_time: Option<String>,
}

impl ProgressExample {
    fn summary(&self) -> String {
        let percent = self
            .percent
            .as_deref()
            .map(|value| format!(" p={value}"))
            .unwrap_or_default();
        let modify_time = self
            .modify_time
            .as_deref()
            .map(|value| format!(" t={value}"))
            .unwrap_or_default();
        format!(
            "{}「{}」{} s={}{}",
            self.kind.label(),
            self.title,
            percent,
            self.status,
            modify_time
        )
    }
}

fn newest_record(records: &[OkrReadProgressRecord]) -> Option<&OkrReadProgressRecord> {
    let mut best = None;
    for record in records {
        if !record_has_summary_value(record) {
            continue;
        }
        let Some(current) = best else {
            best = Some(record);
            continue;
        };
        if progress_record_is_newer(record, current) {
            best = Some(record);
        }
    }
    best
}

fn record_has_summary_value(record: &OkrReadProgressRecord) -> bool {
    record.percent.as_deref().and_then(short_value).is_some()
        || record.status.as_deref().and_then(short_value).is_some()
        || record
            .modify_time
            .as_deref()
            .and_then(short_value)
            .is_some()
}

fn progress_record_is_newer(
    candidate: &OkrReadProgressRecord,
    current: &OkrReadProgressRecord,
) -> bool {
    match (
        candidate.modify_time.as_deref().and_then(progress_time_key),
        current.modify_time.as_deref().and_then(progress_time_key),
    ) {
        (Some(candidate), Some(current)) => {
            compare_progress_time_key(&candidate, &current) == Some(Ordering::Greater)
        }
        (Some(_), None) => true,
        (None, Some(_)) | (None, None) => false,
    }
}

fn compare_progress_examples_for_display(
    left: &ProgressExample,
    right: &ProgressExample,
) -> Option<Ordering> {
    match (
        left.modify_time.as_deref().and_then(progress_time_key),
        right.modify_time.as_deref().and_then(progress_time_key),
    ) {
        (Some(left), Some(right)) => compare_progress_time_key(&right, &left),
        (Some(_), None) => Some(Ordering::Less),
        (None, Some(_)) => Some(Ordering::Greater),
        (None, None) => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProgressTimeKey {
    Numeric { len: usize, digits: String },
    Text(String),
}

fn progress_time_key(value: &str) -> Option<ProgressTimeKey> {
    let value = compact_text(value);
    if value.is_empty() {
        return None;
    }
    if value.chars().all(|character| character.is_ascii_digit()) {
        let digits = value.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        return Some(ProgressTimeKey::Numeric {
            len: digits.len(),
            digits: digits.to_string(),
        });
    }
    Some(ProgressTimeKey::Text(value))
}

fn compare_progress_time_key(left: &ProgressTimeKey, right: &ProgressTimeKey) -> Option<Ordering> {
    match (left, right) {
        (
            ProgressTimeKey::Numeric {
                len: left_len,
                digits: left_digits,
            },
            ProgressTimeKey::Numeric {
                len: right_len,
                digits: right_digits,
            },
        ) => Some(
            left_len
                .cmp(right_len)
                .then_with(|| left_digits.cmp(right_digits)),
        ),
        (ProgressTimeKey::Text(left), ProgressTimeKey::Text(right)) => Some(left.cmp(right)),
        (ProgressTimeKey::Numeric { .. }, ProgressTimeKey::Text(_))
        | (ProgressTimeKey::Text(_), ProgressTimeKey::Numeric { .. }) => None,
    }
}

fn non_empty_compact(value: Option<&str>) -> Option<String> {
    value
        .map(compact_text)
        .filter(|value| !value.trim().is_empty())
}

fn short_title(value: &str) -> Option<String> {
    let value = compact_text(value);
    if value.is_empty() {
        None
    } else {
        Some(truncate_chars(&value, TITLE_LIMIT))
    }
}

fn short_value(value: &str) -> Option<String> {
    let value = compact_text(value);
    if value.is_empty() {
        None
    } else {
        Some(truncate_chars(&value, VALUE_LIMIT))
    }
}

#[cfg(test)]
mod tests;
