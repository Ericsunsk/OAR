import XCTest
@testable import OAR

final class AgentContextCardContentTests: XCTestCase {
    func testContentUsesConversationContextInsteadOfStaleItemAndAction() {
        let staleItem = ReviewInboxDisplayItem(
            id: "review-stale",
            proposedActionID: "action-stale",
            proposedActionVersion: 1,
            objectiveTitle: "旧目标",
            keyResultTitle: "旧的 KR 标题",
            ownerName: "旧负责人",
            weekLabel: "W1",
            riskLevel: .low,
            riskReason: "旧风险原因",
            confidenceScore: 0.2,
            status: .new,
            lastUpdatedAt: "2026-05-01",
            syncCursor: 1
        )
        let staleAction = ReviewInboxSuggestedAction(
            id: "action-stale",
            reviewItemId: "review-stale",
            version: 1,
            actionType: .pingOwner,
            rationale: "旧动作理由",
            expectedImpact: "旧影响",
            dryRunResultSummary: "旧 dry-run",
            estimatedWriteTargetsCount: 1,
            gateState: .pending
        )
        let context = AgentConversationContext(
            title: "真实发送的 KR 标题",
            riskReason: "真实发送的风险原因",
            actionSummary: "更新进展：真实发送的动作摘要 dry-run：只写入进展，不改 owner。",
            evidenceSummaries: ["证据摘要 A", "证据摘要 B"],
            evidenceRefs: [
                AgentEvidenceRef(sourceType: "okr", sourceRef: "okr://real", summary: "证据摘要 A"),
                AgentEvidenceRef(sourceType: "task", sourceRef: "task://real", summary: "证据摘要 B")
            ],
            workspaceSummary: "工作区摘要：共 2 个风险，当前焦点 1/2。",
            workspaceSignals: ["严重｜真实发送的 KR 标题｜owner：陈敏｜置信 91%"],
            pendingActionSummaries: ["真实发送的 KR 标题｜更新进展｜gate：待处理"],
            ledgerEventSummaries: ["审计事件｜正常｜ActionID act-real｜AuditEvent 已记录"]
        )

        let content = AgentContextCardContent(context: context, item: staleItem, action: staleAction)

        XCTAssertEqual(content.title, "真实发送的 KR 标题")
        XCTAssertEqual(
            content.focusText,
            "当前焦点：更新进展：真实发送的动作摘要 dry-run：只写入进展，不改 owner。"
        )
        XCTAssertEqual(content.summaryText, "工作区摘要：共 2 个风险，当前焦点 1/2。")
        XCTAssertEqual(content.statisticsText, "证据 2｜信号 1｜待处理 1｜账本 1")
        XCTAssertEqual(content.primarySignalText, "信号：严重｜真实发送的 KR 标题｜owner：陈敏｜置信 91%")
    }

    func testContentFallsBackToPayloadRiskReasonAndEvidenceSummaries() {
        let context = AgentConversationContext(
            title: "  ",
            riskReason: "缺少最近一周负责人确认。",
            actionSummary: AgentConversationContext.empty.actionSummary,
            evidenceSummaries: ["会议纪要显示需要补证。"],
            evidenceRefs: [],
            workspaceSummary: "  ",
            workspaceSignals: [],
            pendingActionSummaries: []
        )

        let content = AgentContextCardContent(context: context)

        XCTAssertEqual(content.title, AgentConversationContext.empty.title)
        XCTAssertEqual(content.focusText, "当前焦点：缺少最近一周负责人确认。")
        XCTAssertEqual(content.summaryText, "缺少最近一周负责人确认。")
        XCTAssertEqual(content.statisticsText, "证据 1｜信号 0｜待处理 0｜账本 0")
        XCTAssertEqual(content.primarySignalText, "证据：会议纪要显示需要补证。")
    }

    func testContentCompactsWhitespaceFallbacks() {
        let context = AgentConversationContext(
            title: "  ",
            riskReason: "  需要   补充  平台事实  ",
            actionSummary: "  ",
            evidenceSummaries: [],
            evidenceRefs: [],
            workspaceSummary: "  ",
            workspaceSignals: [],
            pendingActionSummaries: []
        )

        let content = AgentContextCardContent(context: context)

        XCTAssertEqual(content.focusText, "当前焦点：需要 补充 平台事实")
        XCTAssertEqual(content.summaryText, "需要 补充 平台事实")
    }

    func testContextStatusContentPrioritizesLiveReadSummary() {
        let content = AgentContextStatusContent(
            status: AgentContextStatus(
                activatedSkillSummaries: ["feishu.okr｜Feishu OKR｜用途：读取 OKR"],
                liveReadSummaries: ["工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。"]
            )
        )

        XCTAssertEqual(content.title, "实时读取已接入")
        XCTAssertEqual(content.statisticsText, "读取 1｜技能 1")
        XCTAssertEqual(
            content.detailText,
            "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。\nfeishu.okr｜Feishu OKR｜用途：读取 OKR"
        )
        XCTAssertEqual(content.symbolName, "antenna.radiowaves.left.and.right")
    }

    func testContextStatusContentHighlightsDegradedRead() {
        let content = AgentContextStatusContent(
            status: AgentContextStatus(
                activatedSkillSummaries: ["feishu.okr｜Feishu OKR"],
                liveReadSummaries: ["工具 feishu.okr.summarize_my_okr｜实时读取降级：缺少权限。"]
            )
        )

        XCTAssertEqual(content.title, "实时读取受限")
        XCTAssertEqual(content.symbolName, "exclamationmark.triangle")
    }
}
