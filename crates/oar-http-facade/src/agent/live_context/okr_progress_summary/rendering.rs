use std::collections::BTreeMap;

use super::super::summary::{finalize_summary, tool_live_label};
use super::aggregation::{
    compare_progress_examples_for_display, OkrProgressAggregation, ProgressExample,
};
use crate::agent::tools::AgentReadTool;

const PROGRESS_EXAMPLE_LIMIT: usize = 3;
const STATUS_COUNT_LIMIT: usize = 4;

pub(super) fn build_okr_progress_live_summary(aggregation: &OkrProgressAggregation) -> String {
    let tool_label = tool_live_label(AgentReadTool::OkrProgress);
    if aggregation.cycles_total == 0 {
        return format!("{tool_label}｜实时：未读取到 OKR 周期。");
    }
    if aggregation.targets_read == 0 {
        return finalize_summary(format!(
            "{tool_label}｜实时：读取到 {} 个 OKR 周期；未发现可读取进展的 Objective/KR target{}。",
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
        "{tool_label}｜实时：周期 {}/{}，Objective {}，KR {}，进展目标 {}，记录 {}{}{}{}。",
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
