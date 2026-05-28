import XCTest
@testable import OARReviewInbox

final class ReviewInboxAPIContractTests: XCTestCase {
    func testSnapshotDecodesAndMapsToDisplayModels() throws {
        let json = """
        {
          "contract_version": 1,
          "generated_at": "2026-05-28T10:00:00Z",
          "items": [
            {
              "id": "ri_1",
              "tenant_id": "t_1",
              "user_id": "u_1",
              "proposed_action_id": "pa_1",
              "proposed_action_version": 2,
              "objective_title": "提升复盘节奏",
              "key_result_title": "每周风险处理完成率 90%",
              "owner_display_name": "陈敏",
              "week_label": "2026 第 22 周",
              "risk_score": 92,
              "priority": 10,
              "risk_reason": "连续两周未处理高风险项。",
              "confidence_score": 0.88,
              "status": "open",
              "sync_cursor": 42,
              "updated_at_display": "5 月 28 日",
              "ledger_status": null,
              "operation_id": null
            }
          ],
          "proposed_actions": [
            {
              "id": "pa_1",
              "review_item_id": "ri_1",
              "tenant_id": "t_1",
              "actor_user_id": "u_1",
              "target_user_id": null,
              "owner_user_id": "u_owner",
              "version": 2,
              "status": "published",
              "kind": "update_kr_progress",
              "risk_severity": "critical",
              "evidence_ids": ["ev_1"],
              "rationale": "补充本周进展和风险说明。",
              "expected_impact": "复盘前对齐 owner。",
              "dry_run_result_summary": "将更新 1 条 KR 进展。",
              "estimated_write_targets_count": 1,
              "decision": null
            }
          ],
          "evidence": [
            {
              "id": "ev_1",
              "review_item_id": "ri_1",
              "source_kind": "okr_progress",
              "source_id": "okr_progress_1",
              "locator": "okr://progress/1",
              "observed_at_display": "5 月 28 日",
              "summary": "只包含摘要，不包含原始正文。",
              "signal_type": "cadence",
              "trust_score": 0.91,
              "content_hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "visibility": "team"
            }
          ],
          "ledger_events": [
            {
              "id": "le_1",
              "action_id": "pa_1",
              "stage": "confirmed_action",
              "stage_status": "pending",
              "timestamp_display": "未执行",
              "message": "等待人工确认。",
              "idempotency_key": "tenant:t_1:pa:pa_1:v2:confirm"
            }
          ]
        }
        """

        let snapshot = try JSONDecoder().decode(ReviewInboxAPISnapshot.self, from: Data(json.utf8))
        let display = snapshot.toDisplaySnapshot()

        XCTAssertEqual(display.items.first?.riskLevel, .critical)
        XCTAssertEqual(display.items.first?.syncCursor, 42)
        XCTAssertEqual(display.actions.first?.version, 2)
        XCTAssertEqual(display.actions.first?.gateState, .pending)
        XCTAssertTrue(display.actions.first?.canEnterProductionExecution == true)
        XCTAssertEqual(display.evidence.first?.sourceType, .okr)
        XCTAssertEqual(display.ledgerEvents.first?.stage, .confirmedAction)
    }
}
