import XCTest
@testable import OAR

@MainActor
final class ReviewInboxViewModelDecisionsLedgerTests: XCTestCase {
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
        XCTAssertEqual(model.rejectedCount, 1)
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
}
