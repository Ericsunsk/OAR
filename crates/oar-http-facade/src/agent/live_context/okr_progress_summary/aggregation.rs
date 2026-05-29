use std::cmp::Ordering;
use std::collections::BTreeMap;

use oar_lark_adapter::{OkrReadProgressPage, OkrReadProgressRecord};

use super::targets::{ProgressTarget, ProgressTargetKind};
use super::text::{compact, short_value};

#[derive(Debug, Default)]
pub(super) struct OkrProgressAggregation {
    pub(super) cycles_total: usize,
    pub(super) cycles_expanded: usize,
    pub(super) objectives_seen: usize,
    pub(super) key_results_seen: usize,
    pub(super) targets_read: usize,
    pub(super) progress_records_read: usize,
    pub(super) skipped_cycles: usize,
    pub(super) skipped_objectives: usize,
    pub(super) skipped_key_results: usize,
    pub(super) skipped_progress_targets: usize,
    pub(super) skipped_missing_ids: usize,
    pub(super) cycle_pages_with_more: usize,
    pub(super) objective_pages_with_more: usize,
    pub(super) key_result_pages_with_more: usize,
    pub(super) progress_pages_with_more: usize,
    pub(super) status_counts: BTreeMap<String, usize>,
    pub(super) examples: Vec<ProgressExample>,
}

impl OkrProgressAggregation {
    pub(super) fn add_progress_page(
        &mut self,
        target: &ProgressTarget,
        page: &OkrReadProgressPage,
    ) {
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
pub(super) struct ProgressExample {
    kind: ProgressTargetKind,
    title: String,
    percent: Option<String>,
    status: String,
    modify_time: Option<String>,
}

impl ProgressExample {
    pub(super) fn summary(&self) -> String {
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

pub(super) fn compare_progress_examples_for_display(
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
    let value = compact(value);
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
