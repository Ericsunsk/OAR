import XCTest
@testable import OAR

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
        XCTAssertEqual(display.ledgerEvents.first?.stageStatus, .pending)
        XCTAssertEqual(display.ledgerEvents.first?.timestamp, "未执行")
        XCTAssertEqual(display.ledgerEvents.first?.message, "等待人工确认。")
        XCTAssertEqual(display.ledgerEvents.first?.idempotencyKey, "tenant:t_1:pa:pa_1:v2:confirm")
    }

    func testDecisionlessDraftSupersededAndWithdrawnActionsDoNotMapToRejectedOrPending() throws {
        let json = """
        {
          "contract_version": 1,
          "generated_at": "2026-05-28T10:00:00Z",
          "items": [],
          "proposed_actions": [
            {
              "id": "pa_draft",
              "review_item_id": "ri_1",
              "tenant_id": "t_1",
              "actor_user_id": "u_1",
              "target_user_id": null,
              "owner_user_id": "u_owner",
              "version": 1,
              "status": "draft",
              "kind": "update_kr_progress",
              "risk_severity": "high",
              "evidence_ids": [],
              "rationale": "草稿动作",
              "expected_impact": "待发布",
              "dry_run_result_summary": "dry-run",
              "estimated_write_targets_count": 0,
              "decision": null
            },
            {
              "id": "pa_superseded",
              "review_item_id": "ri_2",
              "tenant_id": "t_1",
              "actor_user_id": "u_1",
              "target_user_id": null,
              "owner_user_id": "u_owner",
              "version": 1,
              "status": "superseded",
              "kind": "update_kr_progress",
              "risk_severity": "high",
              "evidence_ids": [],
              "rationale": "已替代动作",
              "expected_impact": "不再执行",
              "dry_run_result_summary": "dry-run",
              "estimated_write_targets_count": 0,
              "decision": null
            },
            {
              "id": "pa_withdrawn",
              "review_item_id": "ri_3",
              "tenant_id": "t_1",
              "actor_user_id": "u_1",
              "target_user_id": null,
              "owner_user_id": "u_owner",
              "version": 1,
              "status": "withdrawn",
              "kind": "update_kr_progress",
              "risk_severity": "high",
              "evidence_ids": [],
              "rationale": "已撤回动作",
              "expected_impact": "不再执行",
              "dry_run_result_summary": "dry-run",
              "estimated_write_targets_count": 0,
              "decision": null
            }
          ],
          "evidence": [],
          "ledger_events": []
        }
        """

        let snapshot = try JSONDecoder().decode(ReviewInboxAPISnapshot.self, from: Data(json.utf8))
        let actions = snapshot.toDisplaySnapshot().actions

        XCTAssertEqual(actions.first { $0.id == "pa_draft" }?.gateState, .draft)
        XCTAssertEqual(actions.first { $0.id == "pa_superseded" }?.gateState, .superseded)
        XCTAssertEqual(actions.first { $0.id == "pa_withdrawn" }?.gateState, .withdrawn)
        XCTAssertNotEqual(actions.first { $0.id == "pa_draft" }?.gateState, .rejected)
        XCTAssertNotEqual(actions.first { $0.id == "pa_superseded" }?.gateState, .rejected)
        XCTAssertNotEqual(actions.first { $0.id == "pa_withdrawn" }?.gateState, .pending)
    }

    func testItemExecutionStatusPreservesLedgerStateAndOperationID() throws {
        let snapshot = ReviewInboxAPISnapshot(
            contractVersion: 1,
            generatedAt: "2026-05-28T10:00:00Z",
            items: [
                Self.item(id: "ri_executing", status: .confirmed, ledgerStatus: "executing", operationID: "op_executing"),
                Self.item(id: "ri_failed", status: .confirmed, ledgerStatus: "failed", operationID: "op_failed"),
                Self.item(id: "ri_succeeded", status: .failed, ledgerStatus: "succeeded", operationID: "op_succeeded"),
                Self.item(id: "ri_cancelled", status: .succeeded, ledgerStatus: "cancelled", operationID: "op_cancelled"),
                Self.item(id: "ri_unknown", status: .executing, ledgerStatus: "paused", operationID: "op_unknown"),
                Self.item(id: "ri_withdrawn", status: .withdrawn, ledgerStatus: nil, operationID: nil)
            ],
            proposedActions: [],
            evidence: [],
            ledgerEvents: []
        )

        let items = Dictionary(uniqueKeysWithValues: snapshot.toDisplaySnapshot().items.map { ($0.id, $0) })

        XCTAssertEqual(items["ri_executing"]?.status, .executing)
        XCTAssertEqual(items["ri_executing"]?.ledgerStatus, .executing)
        XCTAssertEqual(items["ri_executing"]?.operationID, "op_executing")
        XCTAssertEqual(items["ri_failed"]?.status, .failed)
        XCTAssertEqual(items["ri_failed"]?.ledgerStatus, .failed)
        XCTAssertEqual(items["ri_succeeded"]?.status, .executed)
        XCTAssertEqual(items["ri_succeeded"]?.ledgerStatus, .succeeded)
        XCTAssertEqual(items["ri_cancelled"]?.status, .cancelled)
        XCTAssertEqual(items["ri_cancelled"]?.ledgerStatus, .cancelled)
        XCTAssertEqual(items["ri_unknown"]?.status, .executing)
        XCTAssertEqual(items["ri_unknown"]?.ledgerStatus, .unknown("paused"))
        XCTAssertEqual(items["ri_unknown"]?.operationID, "op_unknown")
        XCTAssertEqual(items["ri_withdrawn"]?.status, .cancelled)
        XCTAssertNil(items["ri_withdrawn"]?.ledgerStatus)
        XCTAssertNil(items["ri_withdrawn"]?.operationID)
    }

    private static func item(
        id: String,
        status: ReviewInboxItemStatusDTO,
        ledgerStatus: String?,
        operationID: String?
    ) -> ReviewInboxItemDTO {
        ReviewInboxItemDTO(
            id: id,
            tenantID: "t_1",
            userID: "u_1",
            proposedActionID: "pa_\(id)",
            proposedActionVersion: 1,
            objectiveTitle: "目标",
            keyResultTitle: "关键结果",
            ownerDisplayName: "陈敏",
            weekLabel: "2026 第 22 周",
            riskScore: 70,
            priority: 10,
            riskReason: "需要复核",
            confidenceScore: 0.88,
            status: status,
            syncCursor: 42,
            updatedAtDisplay: "5 月 28 日",
            ledgerStatus: ledgerStatus,
            operationID: operationID
        )
    }
}
