use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::SessionState;
use oar_core::storage::postgres::PostgresDeviceSessionRepository;
use serde_json::{json, Value};

use support::{
    json_body, runtime_with_persistence, seed_active_session, seed_review_inbox_snapshot,
};

use super::postgres_support::{run_live_postgres_test, seed_user};
use crate::dispatch_request_with_runtime;
use crate::review_inbox_routes as review_inbox_route_handlers;

mod support;

#[tokio::test]
async fn snapshot_route_loads_live_postgres_review_inbox_data() {
    run_live_postgres_test("facade_review_inbox_snapshot_live", |pool| async move {
        let tenant_id = "tenant_facade_review";
        let user_id = "user_facade_review";
        let session_id = "oar_session_facade_review";
        seed_user(&pool, tenant_id, user_id).await?;
        seed_active_session(&pool, tenant_id, user_id, session_id).await?;

        seed_review_inbox_snapshot(&pool, tenant_id, user_id).await?;
        let response = dispatch_request_with_runtime(
            runtime_with_persistence(pool.clone()),
            &Method::GET,
            "/review-inbox/snapshot",
            None,
            Some("Bearer oar_session_facade_review"),
            Some("application/json"),
        )
        .await;
        let body: Value = serde_json::from_str(&response.body).expect("snapshot json");

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(body["contract_version"], 1);
        assert!(body["generated_at"].as_str().is_some());
        assert_eq!(body["items"].as_array().expect("items").len(), 1);
        assert_eq!(
            body["ledger_events"]
                .as_array()
                .expect("ledger_events")
                .len(),
            0
        );
        assert_eq!(body["items"][0]["id"], "review_facade_1");
        assert_eq!(body["items"][0]["status"], "open");
        assert_eq!(body["items"][0]["sync_cursor"], 77);
        assert_eq!(body["proposed_actions"][0]["id"], "action_facade_1");
        assert_eq!(
            body["proposed_actions"][0]["rationale"],
            "KR progress is stale."
        );
        assert_eq!(body["evidence"][0]["summary"], "Stale KR progress evidence");
        assert!(!response.body.contains("raw_sensitive_payload"));

        let stored_session = PostgresDeviceSessionRepository::new(pool)
            .get_by_id(tenant_id, session_id)
            .await?;
        assert_eq!(
            stored_session.map(|session| session.state),
            Some(SessionState::Active)
        );

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn decision_body_route_records_confirm_and_rejects_stale_replay() {
    run_live_postgres_test("facade_review_decision_confirm_live", |pool| async move {
        let tenant_id = "tenant_facade_decision";
        let user_id = "user_facade_decision";
        let session_id = "oar_session_facade_decision";
        seed_user(&pool, tenant_id, user_id).await?;
        seed_active_session(&pool, tenant_id, user_id, session_id).await?;
        seed_review_inbox_snapshot(&pool, tenant_id, user_id).await?;

        let body = json!({
            "action_id": "action_facade_1",
            "action_version": 1,
            "decision": "confirm",
            "note": "ok to proceed",
            "expected_sync_cursor": 77
        });
        let first = review_inbox_route_handlers::body_route_response(
            runtime_with_persistence(pool.clone()),
            &Method::POST,
            "/review-inbox/decisions",
            Some("Bearer oar_session_facade_decision"),
            json_body(&body),
        )
        .await;
        let first_body: Value = serde_json::from_str(&first.body).expect("first decision json");

        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(first_body["items"][0]["status"], "confirmed");
        assert_eq!(first_body["items"][0]["sync_cursor"], 78);
        assert_eq!(first_body["items"][0]["ledger_status"], "confirmed");
        assert!(first_body["items"][0]["operation_id"]
            .as_str()
            .expect("operation id")
            .starts_with("op-"));
        assert_eq!(first_body["proposed_actions"][0]["decision"], "confirm");
        assert!(!first.body.contains("raw_sensitive_payload"));

        let replay = review_inbox_route_handlers::body_route_response(
            runtime_with_persistence(pool.clone()),
            &Method::POST,
            "/review-inbox/decisions",
            Some("Bearer oar_session_facade_decision"),
            json_body(&body),
        )
        .await;
        let replay_body: Value = serde_json::from_str(&replay.body).expect("replay json");

        assert_eq!(replay.status, StatusCode::CONFLICT);
        assert_eq!(replay_body["error"], "review_decision_conflict");

        let missing_cursor = review_inbox_route_handlers::body_route_response(
            runtime_with_persistence(pool),
            &Method::POST,
            "/review-inbox/decisions",
            Some("Bearer oar_session_facade_decision"),
            json_body(&json!({
                "action_id": "action_facade_1",
                "action_version": 1,
                "decision": "confirm",
                "note": "missing cursor"
            })),
        )
        .await;
        let missing_body: Value =
            serde_json::from_str(&missing_cursor.body).expect("missing cursor json");

        assert_eq!(missing_cursor.status, StatusCode::BAD_REQUEST);
        assert_eq!(missing_body["error"], "review_decision_missing_sync_cursor");

        Ok(())
    })
    .await;
}
