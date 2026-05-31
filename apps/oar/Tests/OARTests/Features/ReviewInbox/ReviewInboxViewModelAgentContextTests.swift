import XCTest
@testable import OAR

@MainActor
final class ReviewInboxViewModelAgentContextTests: XCTestCase {
    func testAgentWorkspaceContextSummarizesCountsRisksAndPendingActions() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        let context = model.agentWorkspaceContext

        XCTAssertEqual(context.title, "激活 12 个合格试点团队")
        XCTAssertEqual(context.evidenceSummaries.count, 3)
        XCTAssertEqual(context.evidenceRefs.count, 3)
        XCTAssertEqual(context.evidenceRefs[0].sourceType, "okr")
        XCTAssertEqual(context.evidenceRefs[0].sourceRef, "okr://cycle/2026q2/objective/ent-growth")
        XCTAssertEqual(context.evidenceRefs[0].summary, "上次进展停留在 19 天前，当前仍为 5/12 个试点。")
        XCTAssertEqual(context.evidenceRefs[1].sourceType, "task")
        XCTAssertEqual(context.evidenceRefs[1].sourceRef, "task://pilot-security-review")
        XCTAssertEqual(context.evidenceRefs[1].summary, "安全问卷任务卡在应用权限说明。")
        XCTAssertEqual(context.evidenceRefs[2].sourceType, "meeting")
        XCTAssertEqual(context.evidenceRefs[2].sourceRef, "minutes://enterprise-weekly-sync")
        XCTAssertEqual(context.evidenceRefs[2].summary, "会议纪要显示两个试点需要周五前决策。")
        XCTAssertTrue(context.workspaceSummary.contains("共 6 个风险"))
        XCTAssertTrue(context.workspaceSummary.contains("严重/高 2 个（严重 1 个）"))
        XCTAssertTrue(context.workspaceSummary.contains("待确认 2 个"))
        XCTAssertTrue(context.workspaceSummary.contains("已确认 1 个"))
        XCTAssertTrue(context.workspaceSummary.contains("执行中 1 个"))
        XCTAssertTrue(context.workspaceSummary.contains("失败 1 个"))
        XCTAssertTrue(context.workspaceSummary.contains("已执行 1 个"))
        XCTAssertTrue(context.workspaceSummary.contains("已取消 0 个"))
        XCTAssertTrue(context.workspaceSummary.contains("已拒绝 0 个"))
        XCTAssertTrue(context.workspaceSummary.contains("当前筛选“全部”显示 6 个"))
        XCTAssertTrue(context.workspaceSummary.contains("当前焦点 1/6"))

        XCTAssertEqual(context.workspaceSignals.count, 5)
        XCTAssertTrue(context.workspaceSignals[0].contains("高｜首轮配置失败率降至 4% 以下"))
        XCTAssertTrue(context.workspaceSignals[0].contains("owner：周然"))
        XCTAssertTrue(context.workspaceSignals[0].contains("置信 84%"))
        XCTAssertFalse(context.workspaceSignals.contains { $0.contains("严重｜激活 12 个合格试点团队") })
        XCTAssertTrue(
            context.workspaceSignals.contains {
                $0.contains("证据缺口：首轮配置失败率降至 4% 以下 仅 1 条摘要证据")
            }
        )

        XCTAssertEqual(context.pendingActionSummaries.count, 3)
        XCTAssertTrue(context.pendingActionSummaries[0].contains("激活 12 个合格试点团队｜更新进展｜gate：待处理"))
        XCTAssertTrue(context.pendingActionSummaries[0].contains("不修改 owner、target、权重"))
        XCTAssertTrue(context.pendingActionSummaries[1].contains("激活 12 个合格试点团队｜安排复盘｜gate：待处理"))
        XCTAssertTrue(context.pendingActionSummaries[2].contains("首轮配置失败率降至 4% 以下｜提醒负责人｜gate：待处理"))
    }

    func testAgentWorkspaceContextIncludesScopedBackendLedgerSummaries() async {
        let selectedAction = makeSuggestedAction(id: "act-selected", reviewItemId: "review-ledger")
        let relatedAction = makeSuggestedAction(id: "act-related", reviewItemId: "review-ledger")
        let unrelatedAction = makeSuggestedAction(id: "act-unrelated", reviewItemId: "review-other")
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [makeDisplayItem(id: "review-ledger")],
            evidence: [],
            actions: [selectedAction, relatedAction, unrelatedAction],
            ledgerEvents: [
                makeTimelineEvent(
                    id: "le-related",
                    actionId: relatedAction.id,
                    stage: .operationLedger,
                    status: .ok,
                    timestamp: "2026-05-30T10:01:00Z",
                    message: "raw_payload sk-secret token leaked",
                    idempotencyKey: "redacted"
                ),
                makeTimelineEvent(
                    id: "le-selected",
                    actionId: selectedAction.id,
                    stage: .auditEvent,
                    status: .ok,
                    timestamp: "2026-05-30T10:02:00Z",
                    message: "Audit event recorded.",
                    idempotencyKey: "redacted"
                ),
                makeTimelineEvent(
                    id: "le-unrelated",
                    actionId: unrelatedAction.id,
                    stage: .confirmedAction,
                    status: .ok,
                    timestamp: "2026-05-30T10:03:00Z",
                    message: "Other action confirmed.",
                    idempotencyKey: "redacted"
                )
            ]
        )))

        await model.load()
        model.selectAction(selectedAction)

        let summaries = model.agentWorkspaceContext.ledgerEventSummaries
        XCTAssertEqual(summaries.count, 2)
        XCTAssertTrue(summaries[0].contains("审计事件｜正常｜2026-05-30T10:02:00Z"))
        XCTAssertTrue(summaries[0].contains("ActionID act-selected｜更新进展｜gate：待处理"))
        XCTAssertTrue(summaries[0].contains("Audit event recorded."))
        XCTAssertTrue(summaries[1].contains("ActionID act-related｜更新进展｜gate：待处理"))
        XCTAssertTrue(summaries[1].contains("已隐藏敏感账本详情。"))
        XCTAssertFalse(summaries.joined(separator: "\n").contains("act-unrelated"))
        XCTAssertFalse(summaries.joined(separator: "\n").contains("raw_payload"))
        XCTAssertFalse(summaries.joined(separator: "\n").contains("sk-secret"))
    }
}
