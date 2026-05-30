import XCTest
@testable import OAR

@MainActor
final class ReviewInboxViewModelTests: XCTestCase {
    func testLoadSelectsFirstSortedItem() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())

        await model.load()

        XCTAssertEqual(model.loadState, .ready)
        XCTAssertEqual(model.selectedItem?.id, "review-001")
        XCTAssertEqual(model.selectedAction?.id, "act-001")
        XCTAssertEqual(model.needsConfirmationCount, 2)
        XCTAssertEqual(model.highRiskCount, 2)
    }

    func testApproveProgressActionUpdatesGateAndLedger() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        model.confirmationNote = "已核对 dry-run。"
        await model.approveSelectedAction()

        XCTAssertNil(model.lastErrorMessage)
        XCTAssertEqual(model.actions.first { $0.id == "act-001" }?.gateState, .approved)
        XCTAssertEqual(model.items.first { $0.id == "review-001" }?.status, .confirmed)
        XCTAssertEqual(model.ledgerEvents.filter { $0.actionId == "act-001" }.count, 4)
    }

    func testFilterChangeReconcilesSelectionToVisibleItem() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        guard let executedItem = model.items.first(where: { $0.id == "review-004" }) else {
            XCTFail("Expected mock executed item")
            return
        }

        model.select(executedItem)
        model.setFilter(.highRisk)

        XCTAssertEqual(model.filter, .highRisk)
        XCTAssertEqual(model.selectedItem?.id, "review-001")
        XCTAssertEqual(model.selectedItemPositionText, "1/2")
    }

    func testPreviousAndNextSelectionFollowSortedVisibleItems() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        XCTAssertFalse(model.canMoveToPreviousItem)
        XCTAssertTrue(model.canMoveToNextItem)

        model.selectNextItem()

        XCTAssertEqual(model.selectedItem?.id, "review-002")
        XCTAssertTrue(model.canMoveToPreviousItem)

        model.selectPreviousItem()

        XCTAssertEqual(model.selectedItem?.id, "review-001")
    }

    func testSelectionPrefersBackendCurrentActionForItem() async {
        let currentAction = makeSuggestedAction(id: "act-current", reviewItemId: "review-multi", version: 2)
        let olderAction = makeSuggestedAction(id: "act-older", reviewItemId: "review-multi", version: 1)
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [
                makeDisplayItem(
                    id: "review-multi",
                    proposedActionID: "act-current",
                    proposedActionVersion: 2
                )
            ],
            evidence: [],
            actions: [olderAction, currentAction],
            ledgerEvents: []
        )))

        await model.load()

        XCTAssertEqual(model.selectedAction?.id, "act-current")
    }

    func testAgentWorkspaceContextSummarizesCountsRisksAndPendingActions() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        let context = model.agentWorkspaceContext

        XCTAssertEqual(context.title, "激活 12 个合格试点团队")
        XCTAssertEqual(context.evidenceSummaries.count, 3)
        XCTAssertEqual(context.evidenceRefs.count, 3)
        XCTAssertEqual(context.evidenceRefs[0].sourceType, "OKR")
        XCTAssertEqual(context.evidenceRefs[0].sourceRef, "okr://cycle/2026q2/objective/ent-growth")
        XCTAssertEqual(context.evidenceRefs[0].summary, "上次进展停留在 19 天前，当前仍为 5/12 个试点。")
        XCTAssertEqual(context.evidenceRefs[1].sourceType, "任务")
        XCTAssertEqual(context.evidenceRefs[1].sourceRef, "task://pilot-security-review")
        XCTAssertEqual(context.evidenceRefs[1].summary, "安全问卷任务卡在应用权限说明。")
        XCTAssertEqual(context.evidenceRefs[2].sourceType, "会议")
        XCTAssertEqual(context.evidenceRefs[2].sourceRef, "minutes://enterprise-weekly-sync")
        XCTAssertEqual(context.evidenceRefs[2].summary, "会议纪要显示两个试点需要周五前决策。")
        XCTAssertTrue(context.workspaceSummary.contains("共 4 个风险"))
        XCTAssertTrue(context.workspaceSummary.contains("严重/高 2 个（严重 1 个）"))
        XCTAssertTrue(context.workspaceSummary.contains("待确认 2 个"))
        XCTAssertTrue(context.workspaceSummary.contains("已执行 1 个"))
        XCTAssertTrue(context.workspaceSummary.contains("当前筛选“全部”显示 4 个"))
        XCTAssertTrue(context.workspaceSummary.contains("当前焦点 1/4"))

        XCTAssertEqual(context.workspaceSignals.count, 5)
        XCTAssertTrue(context.workspaceSignals[0].contains("严重｜激活 12 个合格试点团队"))
        XCTAssertTrue(context.workspaceSignals[0].contains("owner：陈敏"))
        XCTAssertTrue(context.workspaceSignals[0].contains("置信 91%"))
        XCTAssertTrue(context.workspaceSignals[1].contains("高｜首轮配置失败率降至 4% 以下"))
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

    func testApproveNonExecutableActionShowsBoundaryMessage() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        guard let draftAction = model.actions.first(where: { $0.id == "act-002" }) else {
            XCTFail("Expected mock schedule action")
            return
        }

        model.selectAction(draftAction)
        await model.approveSelectedAction()

        XCTAssertEqual(model.actions.first { $0.id == "act-002" }?.gateState, .pending)
        XCTAssertEqual(
            model.lastErrorMessage,
            "当前生产入口只开放进展创建 / 更新，其它动作先保留为草稿。"
        )
    }

    func testRejectNonExecutableActionIsAllowed() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        guard let draftAction = model.actions.first(where: { $0.id == "act-003" }),
              let item = model.items.first(where: { $0.id == draftAction.reviewItemId }) else {
            XCTFail("Expected mock reminder action")
            return
        }

        model.select(item)
        model.selectAction(draftAction)
        model.confirmationNote = "证据不足。"
        await model.rejectSelectedAction()

        XCTAssertEqual(model.actions.first { $0.id == "act-003" }?.gateState, .rejected)
        XCTAssertEqual(model.items.first { $0.id == "review-002" }?.status, .rejected)
    }

    func testLedgerForSelectedActionUsesBackendEventsInOrder() async {
        let action = makeSuggestedAction(id: "act-ledger", reviewItemId: "review-ledger")
        let events = [
            makeTimelineEvent(
                id: "le-audit",
                actionId: action.id,
                stage: .auditEvent,
                status: .ok,
                timestamp: "2026-05-30T10:02:00Z",
                message: "Audit event recorded.",
                idempotencyKey: "audit:le-audit"
            ),
            makeTimelineEvent(
                id: "le-unrelated",
                actionId: "act-other",
                stage: .confirmedAction,
                status: .ok,
                timestamp: "2026-05-30T10:00:00Z",
                message: "Other action confirmed.",
                idempotencyKey: "decision:act-other:v1:confirm"
            ),
            makeTimelineEvent(
                id: "le-operation",
                actionId: action.id,
                stage: .operationLedger,
                status: .ok,
                timestamp: "2026-05-30T10:01:00Z",
                message: "Operation ledger confirmed.",
                idempotencyKey: "decision:act-ledger:v1:confirm"
            )
        ]
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [makeDisplayItem(id: "review-ledger")],
            evidence: [],
            actions: [action],
            ledgerEvents: events
        )))

        await model.load()

        XCTAssertEqual(model.ledgerForSelectedAction.map(\.id), ["le-audit", "le-operation"])
        XCTAssertEqual(model.ledgerForSelectedAction.first?.message, "Audit event recorded.")
        XCTAssertEqual(model.ledgerForSelectedAction.first?.timestamp, "2026-05-30T10:02:00Z")
        XCTAssertEqual(model.ledgerForSelectedAction.first?.idempotencyKey, "audit:le-audit")
    }

    func testApprovedActionWithoutBackendLedgerDoesNotSynthesizeAuditHistory() async {
        let action = makeSuggestedAction(
            id: "act-approved",
            reviewItemId: "review-approved",
            gateState: .approved
        )
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [makeDisplayItem(id: "review-approved", status: .confirmed)],
            evidence: [],
            actions: [action],
            ledgerEvents: []
        )))

        await model.load()

        XCTAssertTrue(model.ledgerForSelectedAction.isEmpty)
    }

    func testRejectedActionWithoutBackendLedgerDoesNotSynthesizeAuditHistory() async {
        let action = makeSuggestedAction(
            id: "act-rejected",
            reviewItemId: "review-rejected",
            gateState: .rejected
        )
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [makeDisplayItem(id: "review-rejected", status: .rejected)],
            evidence: [],
            actions: [action],
            ledgerEvents: []
        )))

        await model.load()

        XCTAssertTrue(model.ledgerForSelectedAction.isEmpty)
    }

    func testUnauthorizedLoadTriggersSessionInvalidation() async {
        let provider = LoadFailingProvider(error: ReviewInboxDataProviderError.unauthorized)
        var invalidationMessages: [String] = []
        let model = ReviewInboxViewModel(provider: provider) { message in
            invalidationMessages.append(message)
        }

        await model.load()

        XCTAssertEqual(invalidationMessages, ["登录会话已失效，请重新扫码登录。"])
        XCTAssertEqual(model.loadState, .failed("复盘收件箱加载失败：登录会话已失效，请重新扫码登录。"))
        XCTAssertEqual(model.lastErrorMessage, "复盘收件箱加载失败：登录会话已失效，请重新扫码登录。")
    }

    func testUnauthorizedSubmitTriggersSessionInvalidation() async {
        let provider = SubmitFailingProvider(error: ReviewInboxDataProviderError.unauthorized)
        var invalidationMessages: [String] = []
        let model = ReviewInboxViewModel(provider: provider) { message in
            invalidationMessages.append(message)
        }
        await model.load()

        await model.approveSelectedAction()

        XCTAssertEqual(invalidationMessages, ["登录会话已失效，请重新扫码登录。"])
        XCTAssertEqual(model.lastErrorMessage, "决策提交失败：登录会话已失效，请重新扫码登录。")
    }

    func testReloadKeepsNewestSnapshotWhenEarlierRequestFinishesLast() async {
        let firstSnapshot = ReviewInboxDisplaySnapshot(
            items: [.init(
                id: "review-old",
                proposedActionID: "act-old",
                proposedActionVersion: 1,
                objectiveTitle: "旧",
                keyResultTitle: "旧快照",
                ownerName: "A",
                weekLabel: "W1",
                riskLevel: .high,
                riskReason: "old",
                confidenceScore: 0.8,
                status: .new,
                lastUpdatedAt: "old",
                syncCursor: 1
            )],
            evidence: [],
            actions: [],
            ledgerEvents: []
        )
        let secondSnapshot = ReviewInboxDisplaySnapshot(
            items: [.init(
                id: "review-new",
                proposedActionID: "act-new",
                proposedActionVersion: 1,
                objectiveTitle: "新",
                keyResultTitle: "新快照",
                ownerName: "B",
                weekLabel: "W2",
                riskLevel: .critical,
                riskReason: "new",
                confidenceScore: 0.9,
                status: .needsConfirmation,
                lastUpdatedAt: "new",
                syncCursor: 2
            )],
            evidence: [],
            actions: [],
            ledgerEvents: []
        )
        let provider = SequencedLoadProvider(responses: [
            .init(delayNanoseconds: 250_000_000, result: .success(firstSnapshot)),
            .init(delayNanoseconds: 20_000_000, result: .success(secondSnapshot))
        ])
        let model = ReviewInboxViewModel(provider: provider)

        let first = Task { await model.reload() }
        try? await Task.sleep(nanoseconds: 15_000_000)
        let second = Task { await model.reload() }

        await first.value
        await second.value

        XCTAssertEqual(model.items.map(\.id), ["review-new"])
        XCTAssertEqual(model.selectedItem?.id, "review-new")
        XCTAssertEqual(model.loadState, .ready)
    }
}

private struct StaticSnapshotProvider: ReviewInboxDataProviding {
    let snapshot: ReviewInboxDisplaySnapshot

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}

private func makeDisplayItem(
    id: String,
    status: ReviewInboxDisplayStatus = .needsConfirmation,
    proposedActionID: String = "act-default",
    proposedActionVersion: UInt64 = 1
) -> ReviewInboxDisplayItem {
    ReviewInboxDisplayItem(
        id: id,
        proposedActionID: proposedActionID,
        proposedActionVersion: proposedActionVersion,
        objectiveTitle: "目标",
        keyResultTitle: "关键结果",
        ownerName: "负责人",
        weekLabel: "2026 第 22 周",
        riskLevel: .high,
        riskReason: "需要复核",
        confidenceScore: 0.88,
        status: status,
        lastUpdatedAt: "2026-05-30T10:00:00Z",
        syncCursor: 1
    )
}

private func makeSuggestedAction(
    id: String,
    reviewItemId: String,
    gateState: ReviewInboxGateState = .pending,
    version: UInt64 = 1
) -> ReviewInboxSuggestedAction {
    ReviewInboxSuggestedAction(
        id: id,
        reviewItemId: reviewItemId,
        version: version,
        actionType: .updateProgress,
        rationale: "补充进展",
        expectedImpact: "更新一条进展",
        dryRunResultSummary: "将更新 1 条进展记录。",
        estimatedWriteTargetsCount: 1,
        gateState: gateState
    )
}

private func makeTimelineEvent(
    id: String,
    actionId: String,
    stage: ReviewInboxTimelineStage,
    status: ReviewInboxTimelineStatus,
    timestamp: String,
    message: String,
    idempotencyKey: String
) -> ReviewInboxTimelineEvent {
    ReviewInboxTimelineEvent(
        id: id,
        actionId: actionId,
        stage: stage,
        stageStatus: status,
        timestamp: timestamp,
        message: message,
        idempotencyKey: idempotencyKey
    )
}

private struct LoadFailingProvider: ReviewInboxDataProviding {
    let error: Error

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        throw error
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}

private struct SubmitFailingProvider: ReviewInboxDataProviding {
    let error: Error

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        ReviewInboxDisplaySnapshot(
            items: ReviewInboxMockData.reviewItems,
            evidence: ReviewInboxMockData.evidence,
            actions: ReviewInboxMockData.actions,
            ledgerEvents: ReviewInboxMockData.ledgerEvents
        )
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        throw error
    }
}

private actor SequencedLoadProvider: ReviewInboxDataProviding {
    struct Response {
        let delayNanoseconds: UInt64
        let result: Result<ReviewInboxDisplaySnapshot, Error>
    }

    private var responses: [Response]

    init(responses: [Response]) {
        self.responses = responses
    }

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        guard !responses.isEmpty else {
            return ReviewInboxDisplaySnapshot(items: [], evidence: [], actions: [], ledgerEvents: [])
        }
        let response = responses.removeFirst()
        try await Task.sleep(nanoseconds: response.delayNanoseconds)
        return try response.result.get()
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}
