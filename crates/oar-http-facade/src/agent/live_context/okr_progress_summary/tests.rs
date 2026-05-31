use super::*;

#[test]
fn empty_progress_summary_reports_no_cycles() {
    let summary = build_okr_progress_live_summary(&OkrProgressAggregation::default());

    assert_eq!(
        summary,
        format!(
            "{}｜实时：未读取到 OKR 周期。",
            tool_live_label(AgentReadTool::OkrProgress)
        )
    );
}

#[test]
fn progress_summary_does_not_leak_ids_or_raw_record_content() {
    let target = ProgressTarget {
        kind: ProgressTargetKind::Objective,
        id: "obj_full_secret_identifier_1234567890".to_string(),
        title: Some(short_title("Launch integration safely with a long title").unwrap()),
        percent: Some("60".to_string()),
        status: Some("started".to_string()),
        modify_time: Some("2026-05-01T00:00:00Z".to_string()),
    };
    let page = OkrReadProgressPage {
        progress_records: vec![OkrReadProgressRecord {
            id: Some("progress_record_raw_content_private_body".to_string()),
            modify_time: Some("2026-05-20T10:00:00Z".to_string()),
            percent: Some("75.5".to_string()),
            status: Some("normal".to_string()),
        }],
        next_page_token: Some("next_raw_payload_token".to_string()),
        has_more: true,
    };
    let mut aggregation = OkrProgressAggregation {
        cycles_total: 1,
        cycles_expanded: 1,
        objectives_seen: 1,
        ..OkrProgressAggregation::default()
    };

    aggregation.add_progress_page(&target, &page);
    let summary = build_okr_progress_live_summary(&aggregation);

    assert!(summary.contains("Launch integration"));
    assert!(summary.contains("p=75.5"));
    assert!(summary.contains("s=normal"));
    assert!(summary.contains("t=2026-05-20T10:00:00Z"));
    assert!(summary.contains("进展分页 1"));
    assert!(!summary.contains("obj_full_secret_identifier"));
    assert!(!summary.contains("progress_record_raw_content"));
    assert!(!summary.contains("next_raw_payload_token"));
    assert!(!summary.contains("private_body"));
}

#[test]
fn progress_summary_counts_statuses_and_truncation() {
    let mut aggregation = OkrProgressAggregation {
        cycles_total: 5,
        cycles_expanded: 3,
        objectives_seen: 12,
        key_results_seen: 22,
        skipped_cycles: 2,
        skipped_objectives: 2,
        skipped_key_results: 2,
        skipped_progress_targets: 4,
        skipped_missing_ids: 1,
        objective_pages_with_more: 1,
        ..OkrProgressAggregation::default()
    };

    for status in ["normal", "normal", "risk"] {
        let target = ProgressTarget {
            kind: ProgressTargetKind::KeyResult,
            id: format!("kr_{status}"),
            title: Some(format!("{status} KR")),
            percent: Some("80".to_string()),
            status: Some(status.to_string()),
            modify_time: Some("1780000000000".to_string()),
        };
        aggregation.add_progress_page(
            &target,
            &OkrReadProgressPage {
                progress_records: vec![],
                next_page_token: None,
                has_more: false,
            },
        );
    }

    let summary = build_okr_progress_live_summary(&aggregation);

    assert!(summary.contains("状态：normal 2、risk 1"));
    assert!(summary.contains("跳过/截断：周期 2、Objective 2、KR 2、进展目标 4"));
    assert!(summary.contains("列表分页 1"));
    assert!(summary.contains("缺 ID 1"));
}

#[test]
fn progress_summary_prefers_latest_comparable_progress_record() {
    let target = ProgressTarget {
        kind: ProgressTargetKind::KeyResult,
        id: "kr_1".to_string(),
        title: Some("Comparable KR".to_string()),
        percent: Some("10".to_string()),
        status: Some("old-target".to_string()),
        modify_time: Some("100".to_string()),
    };
    let page = OkrReadProgressPage {
        progress_records: vec![
            OkrReadProgressRecord {
                id: Some("older_record_id".to_string()),
                modify_time: Some("1780000000000".to_string()),
                percent: Some("40".to_string()),
                status: Some("old".to_string()),
            },
            OkrReadProgressRecord {
                id: Some("newer_record_id".to_string()),
                modify_time: Some("1780000000002".to_string()),
                percent: Some("95".to_string()),
                status: Some("fresh".to_string()),
            },
        ],
        next_page_token: None,
        has_more: false,
    };
    let mut aggregation = OkrProgressAggregation {
        cycles_total: 1,
        cycles_expanded: 1,
        key_results_seen: 1,
        ..OkrProgressAggregation::default()
    };

    aggregation.add_progress_page(&target, &page);
    let summary = build_okr_progress_live_summary(&aggregation);

    assert!(summary.contains("p=95"));
    assert!(summary.contains("s=fresh"));
    assert!(summary.contains("t=1780000000002"));
    assert!(!summary.contains("p=40"));
    assert!(!summary.contains("older_record_id"));
    assert!(!summary.contains("newer_record_id"));
}

#[test]
fn progress_summary_examples_prefer_recent_targets() {
    let mut aggregation = OkrProgressAggregation {
        cycles_total: 1,
        cycles_expanded: 1,
        key_results_seen: 4,
        ..OkrProgressAggregation::default()
    };

    for (title, modify_time) in [
        ("Old KR", "100"),
        ("Newest KR", "400"),
        ("Middle KR", "300"),
        ("Hidden KR", "200"),
    ] {
        let target = ProgressTarget {
            kind: ProgressTargetKind::KeyResult,
            id: title.to_string(),
            title: Some(title.to_string()),
            percent: Some("80".to_string()),
            status: Some("normal".to_string()),
            modify_time: Some(modify_time.to_string()),
        };
        aggregation.add_progress_page(
            &target,
            &OkrReadProgressPage {
                progress_records: vec![],
                next_page_token: None,
                has_more: false,
            },
        );
    }

    let summary = build_okr_progress_live_summary(&aggregation);
    let newest = summary.find("Newest KR").expect("newest example");
    let middle = summary.find("Middle KR").expect("middle example");
    let hidden = summary.find("Hidden KR").expect("third newest example");

    assert!(newest < middle);
    assert!(middle < hidden);
    assert!(!summary.contains("Old KR"));
}

#[test]
fn no_target_summary_includes_safe_skip_counts() {
    let aggregation = OkrProgressAggregation {
        cycles_total: 4,
        cycles_expanded: 3,
        skipped_cycles: 1,
        skipped_missing_ids: 2,
        ..OkrProgressAggregation::default()
    };

    let summary = build_okr_progress_live_summary(&aggregation);

    assert!(summary.contains("未发现可读取进展的 Objective/KR target"));
    assert!(summary.contains("周期 1"));
    assert!(summary.contains("缺 ID 2"));
}
