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
        XCTAssertEqual(model.pendingGateCount, 3)
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
}
