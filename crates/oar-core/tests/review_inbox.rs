use std::time::{Duration, SystemTime};

use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::review_inbox::{
    ReviewInboxError, ReviewInboxItem, ReviewInboxItemId, ReviewInboxItemStatus,
};

fn sample_item(now: SystemTime) -> ReviewInboxItem {
    ReviewInboxItem::new(
        ReviewInboxItemId("inbox_item_01".to_string()),
        TenantId("tenant_01".to_string()),
        WorkspaceUserId("user_01".to_string()),
        "proposed_action_01",
        1,
        80,
        10,
        100,
        10,
        now,
    )
}

#[test]
fn sync_cursor_rollback_is_rejected() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);
    let later = now + Duration::from_secs(30);

    item.advance_sync_cursor(11, later).unwrap();
    let err = item
        .advance_sync_cursor(10, later + Duration::from_secs(10))
        .unwrap_err();

    assert_eq!(
        err,
        ReviewInboxError::StaleSyncCursor {
            current: 11,
            proposed: 10
        }
    );
}

#[test]
fn terminal_status_cannot_roll_back() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.reject(11, now + Duration::from_secs(10)).unwrap();
    let err = item.confirm(12, now + Duration::from_secs(20)).unwrap_err();

    assert_eq!(
        err,
        ReviewInboxError::InvalidStatusTransition {
            from: ReviewInboxItemStatus::Rejected,
            to: ReviewInboxItemStatus::Confirmed
        }
    );
}

#[test]
fn confirmed_can_project_to_executing_and_succeeded() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.confirm(11, now + Duration::from_secs(10)).unwrap();
    item.apply_ledger_projection(
        ReviewInboxItemStatus::Executing,
        "executing",
        Some("op_01".to_string()),
        12,
        now + Duration::from_secs(20),
    )
    .unwrap();
    item.apply_ledger_projection(
        ReviewInboxItemStatus::Succeeded,
        "succeeded",
        Some("op_01".to_string()),
        13,
        now + Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(item.status, ReviewInboxItemStatus::Succeeded);
    assert_eq!(item.ledger_status.as_deref(), Some("succeeded"));
    assert_eq!(item.operation_id.as_deref(), Some("op_01"));
}

#[test]
fn rejected_cannot_project_to_succeeded() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.reject(11, now + Duration::from_secs(10)).unwrap();
    let err = item
        .apply_ledger_projection(
            ReviewInboxItemStatus::Succeeded,
            "succeeded",
            Some("op_02".to_string()),
            12,
            now + Duration::from_secs(20),
        )
        .unwrap_err();

    assert_eq!(
        err,
        ReviewInboxError::InvalidLedgerProjection {
            from: ReviewInboxItemStatus::Rejected,
            to: ReviewInboxItemStatus::Succeeded
        }
    );
}

#[test]
fn higher_sort_key_ranks_first_in_inbox_order() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut low = sample_item(now);
    low.id = ReviewInboxItemId("low".to_string());
    low.sort_key = 100;

    let mut high = sample_item(now + Duration::from_secs(10));
    high.id = ReviewInboxItemId("high".to_string());
    high.sort_key = 300;

    let mut mid = sample_item(now + Duration::from_secs(20));
    mid.id = ReviewInboxItemId("mid".to_string());
    mid.sort_key = 200;

    let mut items = [low, mid, high];
    items.sort_by(|a, b| a.cmp_for_inbox(b));

    assert_eq!(items[0].id.0, "high");
    assert_eq!(items[1].id.0, "mid");
    assert_eq!(items[2].id.0, "low");
}

#[test]
fn confirmed_can_project_to_failed() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.confirm(11, now + Duration::from_secs(10)).unwrap();
    item.apply_ledger_projection(
        ReviewInboxItemStatus::Failed,
        "failed",
        Some("op_03".to_string()),
        12,
        now + Duration::from_secs(20),
    )
    .unwrap();

    assert_eq!(item.status, ReviewInboxItemStatus::Failed);
    assert!(item.status.is_terminal());
}

#[test]
fn withdrawn_is_terminal_and_cannot_be_reopened() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.withdraw(11, now + Duration::from_secs(10)).unwrap();
    assert!(item.status.is_terminal());

    let err = item.confirm(12, now + Duration::from_secs(20)).unwrap_err();

    assert_eq!(
        err,
        ReviewInboxError::InvalidStatusTransition {
            from: ReviewInboxItemStatus::Withdrawn,
            to: ReviewInboxItemStatus::Confirmed
        }
    );
}

#[test]
fn confirmed_cannot_be_rejected_after_confirmation() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut item = sample_item(now);

    item.confirm(11, now + Duration::from_secs(10)).unwrap();
    let err = item.reject(12, now + Duration::from_secs(20)).unwrap_err();

    assert_eq!(
        err,
        ReviewInboxError::InvalidStatusTransition {
            from: ReviewInboxItemStatus::Confirmed,
            to: ReviewInboxItemStatus::Rejected
        }
    );
}

#[test]
fn sort_ties_use_updated_at_desc_then_id_asc() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut newer = sample_item(now + Duration::from_secs(20));
    newer.id = ReviewInboxItemId("b-item".to_string());
    newer.sort_key = 500;

    let mut older = sample_item(now + Duration::from_secs(10));
    older.id = ReviewInboxItemId("a-item".to_string());
    older.sort_key = 500;

    let mut same_time_a = sample_item(now + Duration::from_secs(20));
    same_time_a.id = ReviewInboxItemId("a-item-same-time".to_string());
    same_time_a.sort_key = 500;

    let mut same_time_b = sample_item(now + Duration::from_secs(20));
    same_time_b.id = ReviewInboxItemId("b-item-same-time".to_string());
    same_time_b.sort_key = 500;

    let mut items = [same_time_b, older, newer, same_time_a];
    items.sort_by(|a, b| a.cmp_for_inbox(b));

    assert_eq!(items[0].id.0, "a-item-same-time");
    assert_eq!(items[1].id.0, "b-item");
    assert_eq!(items[2].id.0, "b-item-same-time");
    assert_eq!(items[3].id.0, "a-item");
}
