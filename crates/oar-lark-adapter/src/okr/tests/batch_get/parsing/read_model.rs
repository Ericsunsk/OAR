use serde_json::json;

use super::*;

#[test]
fn batch_get_typed_model_tolerates_missing_optional_fields_and_normalizes() {
    let snapshot = snapshot_from_batch_get_body(json!({
        "code": 0,
        "data": {
            "okr_list": [
                {
                    "id":"okr_1",
                    "period_id":"period_2026_q2",
                    "name":"north star",
                    "objective_list":[
                        {
                            "id":"obj_1",
                            "content":"grow x",
                            "progress_rate":{"percent":73,"status":"normal"},
                            "progress_record_list":[{"id":"pr_1"},{"id":"pr_2"}],
                            "progress_rate_percent_last_updated_time":"1780000000000",
                            "progress_record_last_updated_time":"1781000000000",
                            "progress_report_last_updated_time":"0",
                            "deadline":"2026-06-30",
                            "kr_list":[
                                {
                                    "id":"kr_1",
                                    "content":"ship y",
                                    "progress_rate":{"percent":80,"status":"normal"},
                                    "progress_record_list":[{"id":"kpr_1"}],
                                    "progress_rate_percent_last_updated_time":"1780000000001",
                                    "progress_rate_status_last_updated_time":"1781000000001",
                                    "progress_record_last_updated_time":"1782000000001",
                                    "deadline":"2026-06-25"
                                },
                                {
                                    "id":"kr_2",
                                    "content":"no rate or record fields"
                                }
                            ]
                        }
                    ]
                },
                {
                    "id":"okr_blank_name",
                    "name":"   ",
                    "objective_list":[]
                }
            ]
        }
    }));

    assert_eq!(snapshot.okrs.len(), 2);
    assert_eq!(snapshot.okrs[0].okr_id.as_deref(), Some("okr_1"));
    assert_eq!(
        snapshot.okrs[0].period_id.as_deref(),
        Some("period_2026_q2")
    );
    assert_eq!(snapshot.okrs[0].okr_name.as_deref(), Some("north star"));
    assert!(snapshot.okrs[1].okr_name.is_none());
    assert_eq!(snapshot.okrs[0].objectives.len(), 1);
    assert_eq!(
        snapshot.okrs[0].objectives[0].progress_record_ids,
        vec!["pr_1".to_string(), "pr_2".to_string()]
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].last_updated_time.as_deref(),
        Some("1781000000000")
    );
    assert_eq!(snapshot.okrs[0].objectives[0].krs.len(), 2);
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].kr_id.as_deref(),
        Some("kr_1")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].progress_record_ids,
        vec!["kpr_1".to_string()]
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0]
            .last_updated_time
            .as_deref(),
        Some("1782000000001")
    );
    assert!(snapshot.okrs[0].objectives[0].krs[1].progress.is_none());
    assert!(snapshot.okrs[0].objectives[0].krs[1].status.is_none());
    assert!(snapshot.okrs[0].objectives[0].krs[1]
        .progress_record_ids
        .is_empty());
}

#[test]
fn batch_get_accepts_legacy_id_aliases_and_preserves_non_epoch_update_time() {
    let snapshot = snapshot_from_batch_get_body(json!({
        "code": 0,
        "data": {
            "okr_list": [{
                "okr_id":"okr_alias",
                "objective_list":[{
                    "objective_id":"obj_alias",
                    "progress_report_last_updated_time":"2026-05-21T10:00:00Z",
                    "kr_list":[{
                        "kr_id":"kr_alias",
                        "progress_record_last_updated_time":"2026-05-22T10:00:00Z"
                    }]
                }]
            }]
        }
    }));

    assert_eq!(snapshot.okrs[0].okr_id.as_deref(), Some("okr_alias"));
    assert_eq!(
        snapshot.okrs[0].objectives[0].objective_id.as_deref(),
        Some("obj_alias")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].last_updated_time.as_deref(),
        Some("2026-05-21T10:00:00Z")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].kr_id.as_deref(),
        Some("kr_alias")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0]
            .last_updated_time
            .as_deref(),
        Some("2026-05-22T10:00:00Z")
    );
}
