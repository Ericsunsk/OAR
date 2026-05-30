use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadSnapshot {
    #[serde(default)]
    pub okrs: Vec<OkrReadOkr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadCyclesPage {
    #[serde(default)]
    pub cycles: Vec<OkrReadCycle>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadCycle {
    pub cycle_id: Option<String>,
    pub name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadObjectivesPage {
    pub cycle_id: String,
    #[serde(default)]
    pub objectives: Vec<OkrReadObjective>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadKeyResultsPage {
    pub objective_id: String,
    #[serde(default)]
    pub krs: Vec<OkrReadKeyResult>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadProgressPage {
    #[serde(default)]
    pub progress_records: Vec<OkrReadProgressRecord>,
    pub next_page_token: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadProgressRecord {
    pub id: Option<String>,
    pub modify_time: Option<String>,
    pub percent: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadOkr {
    pub okr_id: Option<String>,
    pub period_id: Option<String>,
    pub okr_name: Option<String>,
    #[serde(default)]
    pub objectives: Vec<OkrReadObjective>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadObjective {
    pub objective_id: Option<String>,
    pub content: Option<String>,
    pub progress: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub progress_record_ids: Vec<String>,
    pub deadline: Option<String>,
    pub last_updated_time: Option<String>,
    #[serde(default)]
    pub krs: Vec<OkrReadKeyResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OkrReadKeyResult {
    pub kr_id: Option<String>,
    pub content: Option<String>,
    pub progress: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub progress_record_ids: Vec<String>,
    pub deadline: Option<String>,
    pub last_updated_time: Option<String>,
}
