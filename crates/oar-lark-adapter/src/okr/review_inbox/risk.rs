use oar_core::domain::proposed_action::RiskSeverity;

use crate::okr::types::OkrReadKeyResult;

pub(super) fn risk_score_for_kr(kr: &OkrReadKeyResult) -> u32 {
    let status_score = match kr.status.as_deref() {
        Some("2") | Some("delayed") | Some("delay") => 85,
        Some("1") | Some("risk") => 70,
        Some("-1") | None => 55,
        Some("0") | Some("normal") => 20,
        Some(_) => 45,
    };
    let progress_score = match parse_progress(kr.progress.as_deref()) {
        Some(value) if value < 30.0 => 75,
        Some(value) if value < 50.0 => 60,
        Some(value) if value < 70.0 => 45,
        Some(_) => 20,
        None => 55,
    };
    status_score.max(progress_score)
}

pub(super) fn risk_severity_for_score(score: u32) -> RiskSeverity {
    match score {
        85..=u32::MAX => RiskSeverity::High,
        60..=84 => RiskSeverity::Medium,
        _ => RiskSeverity::Low,
    }
}

fn parse_progress(value: Option<&str>) -> Option<f64> {
    value?.trim().parse::<f64>().ok()
}
