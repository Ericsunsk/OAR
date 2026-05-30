use serde_json::json;

use super::*;

#[test]
fn batch_get_success_response_parses() {
    let parsed = parse_batch_get_body(json!({
        "code": 0,
        "msg": "ok",
        "data": {
            "okr_list": [
                {
                    "id":"okr_1",
                    "period_id":"period_2026_q2",
                    "name":"A",
                    "permission":1,
                    "confirm_status":2,
                    "objective_list":[
                        {
                            "id":"obj_1",
                            "content":"grow x",
                            "permission":3,
                            "score":88.5,
                            "weight":60,
                            "progress_rate":{"percent":73,"status":"normal"},
                            "progress_record_list":[{"id":"pr_1"},{"id":"pr_2"}],
                            "last_updated_time":"2026-05-20T10:00:00Z",
                            "deadline":"2026-06-30",
                            "kr_list":[
                                {
                                    "id":"kr_1",
                                    "content":"ship y",
                                    "score":95,
                                    "kr_weight":20,
                                    "weight":0.5,
                                    "progress_rate":{"percent":80,"status":"normal"},
                                    "progress_record_list":[{"id":"kpr_1"}],
                                    "last_updated_time":"2026-05-21T10:00:00Z",
                                    "deadline":"2026-06-25"
                                }
                            ]
                        }
                    ]
                },
                {"id":"okr_2","name":"B","objective_list":[]}
            ]
        }
    }));

    assert_eq!(parsed.code, 0);
    let data = parsed.data.expect("data");
    assert_eq!(data.okr_list.len(), 2);
    assert_eq!(data.okr_list[0].id.as_deref(), Some("okr_1"));
    assert_eq!(data.okr_list[0].permission.as_deref(), Some("1"));
    assert_eq!(data.okr_list[0].confirm_status.as_deref(), Some("2"));
    assert_eq!(
        data.okr_list[0].objective_list[0].score.as_deref(),
        Some("88.5")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].weight.as_deref(),
        Some("60")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0]
            .progress_rate
            .as_ref()
            .and_then(|x| x.percent.as_deref()),
        Some("73")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0].progress_record_list[0]
            .id
            .as_deref(),
        Some("kpr_1")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0]
            .kr_weight
            .as_deref(),
        Some("20")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0]
            .weight
            .as_deref(),
        Some("0.5")
    );
}

#[test]
fn batch_get_accepts_numeric_progress_status() {
    let parsed = parse_batch_get_body(json!({
        "code": 0,
        "data": {
            "okr_list": [{
                "id":"okr_1",
                "objective_list":[{
                    "id":"obj_1",
                    "permission":{"unexpected":true},
                    "progress_rate":{"percent":50.5,"status":1},
                    "kr_list":[]
                }]
            }]
        }
    }));

    let objective = &parsed.data.expect("data").okr_list[0].objective_list[0];
    assert!(objective.permission.is_none());
    assert_eq!(
        objective
            .progress_rate
            .as_ref()
            .and_then(|x| x.percent.as_deref()),
        Some("50.5")
    );
    assert_eq!(
        objective
            .progress_rate
            .as_ref()
            .and_then(|x| x.status.as_deref()),
        Some("1")
    );
}
