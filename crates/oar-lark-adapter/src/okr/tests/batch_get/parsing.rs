use serde_json::json;

use super::{sample_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{FeishuOkrReadClient, OkrReadSnapshot};

#[test]
fn batch_get_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
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
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
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
fn batch_get_typed_model_tolerates_missing_optional_fields_and_normalizes() {
    let response = HttpResponse::new(
        200,
        json!({
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
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    let data = parsed.data.expect("data");
    let snapshot = OkrReadSnapshot::from_batch_get_data(&data);
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
    let response = HttpResponse::new(
        200,
        json!({
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
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    let snapshot = OkrReadSnapshot::from_batch_get_data(&parsed.data.expect("data"));

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

#[test]
fn batch_get_accepts_numeric_progress_status() {
    let response = HttpResponse::new(
        200,
        json!({
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
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
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
