use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::SessionState;
use oar_core::domain::evidence::{
    EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind, EvidenceVisibilityScope,
};
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionId, ProposedActionKind, RiskSeverity,
};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId};
use oar_core::storage::postgres::{PostgresDeviceSessionRepository, PostgresReviewInboxRepository};
use serde_json::{json, Value};

use super::postgres_support::{device_session, run_live_postgres_test, seed_user, TestResult};
use crate::persistence::FacadePersistenceRuntime;
use crate::{dispatch_request_with_runtime, OarHttpFacadeRuntime};

const VALID_HASH: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn snapshot_route_loads_live_postgres_review_inbox_data() {
    run_live_postgres_test("facade_review_inbox_snapshot_live", |pool| async move {
        let tenant_id = "tenant_facade_review";
        let user_id = "user_facade_review";
        let session_id = "oar_session_facade_review";
        seed_user(&pool, tenant_id, user_id).await?;

        let now = UNIX_EPOCH + Duration::from_millis(1_748_310_100_000);
        let session = device_session(tenant_id, user_id, session_id, "review_inbox", 0, now);
        PostgresDeviceSessionRepository::new(pool.clone())
            .upsert_with_identity_hash(&session, "sha256:facade-review-session")
            .await?;

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

fn runtime_with_persistence(pool: sqlx::PgPool) -> Arc<OarHttpFacadeRuntime> {
    Arc::new(OarHttpFacadeRuntime {
        persistence: Some(FacadePersistenceRuntime::new_for_test(
            pool,
            "key-test-v1".to_string(),
            [7; 32],
        )),
        feishu_login: None,
        agent: None,
        agent_settings: None,
    })
}

async fn seed_review_inbox_snapshot(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    user_id: &str,
) -> TestResult {
    let repository = PostgresReviewInboxRepository::new(pool.clone());
    repository
        .insert_evidence_item(
            tenant_id,
            &evidence_item(
                "evidence_facade_1",
                "Stale KR progress evidence",
                "kr_facade_1",
            ),
        )
        .await?;
    let action = proposed_action(tenant_id, user_id, "action_facade_1", 1);
    repository
        .insert_proposed_action(&action, Some(ms(1_748_310_101_000)))
        .await?;
    repository
        .insert_proposed_action_evidence_ref(tenant_id, "action_facade_1", 1, "evidence_facade_1")
        .await?;
    repository
        .upsert_review_inbox_item(&review_inbox_item(
            tenant_id,
            user_id,
            "review_facade_1",
            "action_facade_1",
            1,
        ))
        .await?;
    Ok(())
}

fn evidence_item(id: &str, summary: &str, source_id: &str) -> EvidenceItem {
    EvidenceItem::new(
        EvidenceId(id.to_string()),
        summary,
        EvidenceRef::new(EvidenceSourceKind::OkrProgress, source_id, None)
            .expect("evidence reference should be valid"),
        VALID_HASH,
        EvidenceVisibilityScope::Tenant,
        ms(1_748_310_100_000),
        ms(1_748_310_101_000),
    )
    .expect("evidence item should be valid")
}

fn proposed_action(tenant_id: &str, user_id: &str, id: &str, version: u64) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        Some(WorkspaceUserId(user_id.to_string())),
        version,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec!["evidence_facade_1".to_string()],
        json!({
            "objective_title": "Ship OAR",
            "key_result_title": "KR progress freshness",
            "owner_display_name": "OAR User",
            "rationale": "KR progress is stale.",
            "expected_impact": "Refresh weekly progress signal.",
            "dry_run_result_summary": "Would update one KR progress record.",
            "estimated_write_targets_count": 1,
            "raw_sensitive_payload": "should_not_leave_backend"
        }),
    )
    .expect("proposed action draft should be valid");
    action.publish().expect("publish should work");
    action
}

fn review_inbox_item(
    tenant_id: &str,
    user_id: &str,
    id: &str,
    proposed_action_id: &str,
    proposed_action_version: u64,
) -> ReviewInboxItem {
    ReviewInboxItem::new(
        ReviewInboxItemId(id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        proposed_action_id,
        proposed_action_version,
        82,
        8,
        700,
        77,
        ms(1_748_310_102_000),
    )
}

fn ms(value: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(value)
}
