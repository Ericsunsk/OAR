use oar_lark_adapter::OkrReadCycle;

use super::okr_topology::OkrTopologyRead;
use super::summary::{compact_text, finalize_summary, truncate_chars};

const MY_OKR_CYCLE_DETAIL_LIMIT: usize = 3;
const MY_OKR_OBJECTIVE_DETAIL_LIMIT: usize = 8;
const MY_OKR_TITLE_LIMIT: usize = 3;

pub(super) fn build_my_okr_summary_from_topology(topology: &OkrTopologyRead) -> String {
    let OkrTopologyRead::Snapshot(snapshot) = topology else {
        return "工具 feishu.okr.summarize_my_okr｜实时：Feishu 返回空数据。".to_string();
    };
    if snapshot.cycles.is_empty() {
        return "工具 feishu.okr.summarize_my_okr｜实时：未读取到 OKR 周期。".to_string();
    }

    let mut cycle_summaries = Vec::new();
    let mut skipped_missing_id = 0_usize;
    for topology_cycle in snapshot.cycles.iter().take(MY_OKR_CYCLE_DETAIL_LIMIT) {
        if topology_cycle.stable_cycle_id().is_none() {
            skipped_missing_id += 1;
            continue;
        }
        let Some(objectives) = topology_cycle.objectives.as_ref() else {
            cycle_summaries.push(format!("{}：详情为空", cycle_label(&topology_cycle.cycle)));
            continue;
        };

        let mut kr_count = 0_usize;
        for objective in objectives.iter().take(MY_OKR_OBJECTIVE_DETAIL_LIMIT) {
            let Some(objective_id) = objective
                .objective_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
            else {
                continue;
            };
            if let Some(krs_page) = topology_cycle.key_results_for_objective(objective_id) {
                kr_count += krs_page.krs.len();
            }
        }

        let titles = objectives
            .iter()
            .filter_map(|objective| objective.content.as_deref())
            .map(compact_text)
            .filter(|value| !value.is_empty())
            .take(MY_OKR_TITLE_LIMIT)
            .map(|title| truncate_chars(&title, 20))
            .collect::<Vec<_>>();
        let title_suffix = if titles.is_empty() {
            String::new()
        } else {
            format!("，示例：{}", titles.join(" / "))
        };
        cycle_summaries.push(format!(
            "{}：{} 个 Objective、{} 个 KR{}",
            cycle_label(&topology_cycle.cycle),
            objectives.len(),
            kr_count,
            title_suffix
        ));
    }

    if skipped_missing_id > 0 {
        cycle_summaries.push(format!("{} 个周期缺少稳定 ID，已跳过", skipped_missing_id));
    }
    let detail_suffix = if snapshot.cycles.len() > MY_OKR_CYCLE_DETAIL_LIMIT {
        format!("；仅展开前 {} 个周期详情", MY_OKR_CYCLE_DETAIL_LIMIT)
    } else {
        String::new()
    };

    finalize_summary(format!(
        "工具 feishu.okr.summarize_my_okr｜实时：读取到 {} 个 OKR 周期；{}{}。",
        snapshot.cycles.len(),
        cycle_summaries.join("；"),
        detail_suffix
    ))
}

fn cycle_label(cycle: &OkrReadCycle) -> String {
    if let Some(name) = cycle
        .name
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
    {
        return truncate_chars(&name, 24);
    }
    let start = cycle
        .start_time
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty());
    let end = cycle
        .end_time
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty());
    match (start, end) {
        (Some(start), Some(end)) => format!(
            "{} 至 {}",
            truncate_chars(&start, 10),
            truncate_chars(&end, 10)
        ),
        (Some(start), None) => truncate_chars(&start, 24),
        _ => "未命名周期".to_string(),
    }
}
