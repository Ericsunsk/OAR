import XCTest
@testable import OAR

@MainActor
final class ReviewInboxViewModelSelectionFilteringTests: XCTestCase {
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

    func testExecutionStatusFiltersUseSeparateBucketsAndReconcileSelection() async {
        let model = ReviewInboxViewModel(provider: MockReviewInboxDataProvider())
        await model.load()

        guard let executedItem = model.items.first(where: { $0.id == "review-004" }) else {
            XCTFail("Expected mock executed item")
            return
        }

        model.select(executedItem)
        model.setFilter(.confirmed)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-003"])
        XCTAssertEqual(model.selectedItem?.id, "review-003")
        XCTAssertEqual(model.selectedItemPositionText, "1/1")

        model.setFilter(.executing)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-005"])
        XCTAssertEqual(model.selectedItem?.id, "review-005")
        XCTAssertEqual(model.executingCount, 1)

        model.setFilter(.failed)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-006"])
        XCTAssertEqual(model.selectedItem?.id, "review-006")
        XCTAssertEqual(model.failedCount, 1)

        model.setFilter(.executed)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-004"])
        XCTAssertEqual(model.selectedItem?.id, "review-004")
        XCTAssertEqual(model.executedCount, 1)
    }

    func testTerminalStatusFiltersIncludeCancelledAndRejected() async {
        let model = ReviewInboxViewModel(provider: StaticSnapshotProvider(snapshot: ReviewInboxDisplaySnapshot(
            items: [
                makeDisplayItem(id: "review-cancelled", status: .cancelled),
                makeDisplayItem(id: "review-rejected", status: .rejected),
                makeDisplayItem(id: "review-open", status: .needsConfirmation)
            ],
            evidence: [],
            actions: [],
            ledgerEvents: []
        )))

        await model.load()

        XCTAssertEqual(model.cancelledCount, 1)
        XCTAssertEqual(model.rejectedCount, 1)

        model.setFilter(.cancelled)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-cancelled"])
        XCTAssertEqual(model.selectedItem?.id, "review-cancelled")

        model.setFilter(.rejected)

        XCTAssertEqual(model.sortedItems.map(\.id), ["review-rejected"])
        XCTAssertEqual(model.selectedItem?.id, "review-rejected")
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
}
