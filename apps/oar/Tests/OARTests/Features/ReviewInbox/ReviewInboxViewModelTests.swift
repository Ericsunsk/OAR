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
