use super::super::feishu_types::{
    FeishuOkr, FeishuOkrBatchGetData, FeishuOkrCycle, FeishuOkrCycleListData,
    FeishuOkrCycleObjectivesListData, FeishuOkrKeyResult, FeishuOkrObjective,
    FeishuOkrObjectiveKeyResultsListData, FeishuOkrProgressListData, FeishuOkrProgressRecord,
    FeishuOkrProgressRecordRef,
};
use super::super::parser::{content_value_to_text, latest_updated_time, non_empty};
use super::models::{
    OkrReadCycle, OkrReadCyclesPage, OkrReadKeyResult, OkrReadKeyResultsPage, OkrReadObjective,
    OkrReadObjectivesPage, OkrReadOkr, OkrReadProgressPage, OkrReadProgressRecord, OkrReadSnapshot,
};

impl OkrReadSnapshot {
    pub fn from_batch_get_data(data: &FeishuOkrBatchGetData) -> Self {
        let okrs = data.okr_list.iter().map(OkrReadOkr::from).collect();
        Self { okrs }
    }
}

impl OkrReadCyclesPage {
    pub fn from_cycle_list_data(data: &FeishuOkrCycleListData) -> Self {
        Self {
            cycles: data.items.iter().map(OkrReadCycle::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl OkrReadObjectivesPage {
    pub fn from_cycle_objectives_list_data(
        cycle_id: impl Into<String>,
        data: &FeishuOkrCycleObjectivesListData,
    ) -> Self {
        Self {
            cycle_id: cycle_id.into(),
            objectives: data.items.iter().map(OkrReadObjective::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl OkrReadKeyResultsPage {
    pub fn from_objective_key_results_list_data(
        objective_id: impl Into<String>,
        data: &FeishuOkrObjectiveKeyResultsListData,
    ) -> Self {
        Self {
            objective_id: objective_id.into(),
            krs: data.items.iter().map(OkrReadKeyResult::from).collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl OkrReadProgressPage {
    pub fn from_progress_list_data(data: &FeishuOkrProgressListData) -> Self {
        Self {
            progress_records: data
                .progress_list
                .iter()
                .map(OkrReadProgressRecord::from)
                .collect(),
            next_page_token: data.page_token.clone(),
            has_more: data.has_more.unwrap_or(false),
        }
    }
}

impl From<&FeishuOkrCycle> for OkrReadCycle {
    fn from(value: &FeishuOkrCycle) -> Self {
        Self {
            cycle_id: value.id.clone(),
            name: value.name.clone().and_then(non_empty),
            start_time: value.start_time.clone(),
            end_time: value.end_time.clone(),
            status: value.status.clone(),
        }
    }
}

impl From<&FeishuOkr> for OkrReadOkr {
    fn from(value: &FeishuOkr) -> Self {
        Self {
            okr_id: value.id.clone(),
            period_id: value.period_id.clone(),
            okr_name: value.name.clone().and_then(non_empty),
            objectives: value
                .objective_list
                .iter()
                .map(OkrReadObjective::from)
                .collect(),
        }
    }
}

impl From<&FeishuOkrObjective> for OkrReadObjective {
    fn from(value: &FeishuOkrObjective) -> Self {
        Self {
            objective_id: value.id.clone(),
            content: value.content.as_ref().and_then(content_value_to_text),
            progress: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.percent.clone()),
            status: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.status.clone()),
            progress_record_ids: collect_progress_record_ids(&value.progress_record_list),
            deadline: value.deadline.clone(),
            last_updated_time: latest_updated_time(&[
                value.last_updated_time.as_deref(),
                value.progress_rate_percent_last_updated_time.as_deref(),
                value.progress_rate_status_last_updated_time.as_deref(),
                value.progress_record_last_updated_time.as_deref(),
                value.progress_report_last_updated_time.as_deref(),
                value.score_last_updated_time.as_deref(),
            ]),
            krs: value.kr_list.iter().map(OkrReadKeyResult::from).collect(),
        }
    }
}

impl From<&FeishuOkrKeyResult> for OkrReadKeyResult {
    fn from(value: &FeishuOkrKeyResult) -> Self {
        Self {
            kr_id: value.id.clone(),
            content: value.content.as_ref().and_then(content_value_to_text),
            progress: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.percent.clone()),
            status: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.status.clone()),
            progress_record_ids: collect_progress_record_ids(&value.progress_record_list),
            deadline: value.deadline.clone(),
            last_updated_time: latest_updated_time(&[
                value.last_updated_time.as_deref(),
                value.progress_rate_percent_last_updated_time.as_deref(),
                value.progress_rate_status_last_updated_time.as_deref(),
                value.progress_record_last_updated_time.as_deref(),
                value.progress_report_last_updated_time.as_deref(),
                value.score_last_updated_time.as_deref(),
            ]),
        }
    }
}

impl From<&FeishuOkrProgressRecord> for OkrReadProgressRecord {
    fn from(value: &FeishuOkrProgressRecord) -> Self {
        Self {
            id: value.progress_id.clone(),
            modify_time: value.modify_time.clone(),
            percent: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.percent.clone()),
            status: value
                .progress_rate
                .as_ref()
                .and_then(|rate| rate.status.clone()),
        }
    }
}

fn collect_progress_record_ids(records: &[FeishuOkrProgressRecordRef]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| record.id.clone())
        .collect()
}
