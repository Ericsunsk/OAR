import XCTest
@testable import OAR

@MainActor
final class ReviewInboxViewModelLoadingSessionTests: XCTestCase {
    func testLoadSelectsFirstSortedItem() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())

        await model.load()

        XCTAssertEqual(model.loadState, .ready)
        XCTAssertEqual(model.selectedItem?.id, "review-001")
        XCTAssertEqual(model.selectedAction?.id, "act-001")
        XCTAssertEqual(model.needsConfirmationCount, 2)
        XCTAssertEqual(model.highRiskCount, 2)
        XCTAssertEqual(model.confirmedCount, 1)
        XCTAssertEqual(model.executingCount, 1)
        XCTAssertEqual(model.failedCount, 1)
        XCTAssertEqual(model.executedCount, 1)
        XCTAssertEqual(model.cancelledCount, 0)
        XCTAssertEqual(model.rejectedCount, 0)
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
