use hyper::http::StatusCode;
use serde_json::Value;

use super::super::decision::decode_review_decision_request;

#[test]
fn decision_request_decode_rejects_invalid_json_safely() {
    let response =
        decode_review_decision_request(br#"{"action_id":"pa_1","#).expect_err("invalid json");
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "review_decision_invalid_json");
    assert!(!response.body.contains("authorization"));
    assert!(!response.body.contains("token"));
}

#[test]
fn decision_request_decode_accepts_swift_contract_body() {
    let request = decode_review_decision_request(
        br#"{
                "action_id":"pa_1",
                "action_version":2,
                "decision":"confirm",
                "note":"ok",
                "expected_sync_cursor":42
            }"#,
    )
    .expect("valid request");

    assert_eq!(request.action_id, "pa_1");
    assert_eq!(request.action_version, 2);
    assert_eq!(request.expected_sync_cursor, Some(42));
}
