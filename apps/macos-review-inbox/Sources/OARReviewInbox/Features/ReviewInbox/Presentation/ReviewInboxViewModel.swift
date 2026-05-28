import Foundation

@Observable
@MainActor
final class ReviewInboxViewModel {
    var items: [ReviewInboxDisplayItem] = []
    var evidence: [ReviewInboxDisplayEvidence] = []
    var actions: [ReviewInboxSuggestedAction] = []
    var ledgerEvents: [ReviewInboxTimelineEvent] = []
    var filter: ReviewInboxFilter = .all
    var selectedItemID: ReviewInboxDisplayItem.ID?
    var selectedActionID: ReviewInboxSuggestedAction.ID?
    var confirmationNote: String = ""
    var loadState: ReviewInboxLoadState = .idle
    var isSubmittingDecision = false
    var lastErrorMessage: String?

    private let provider: ReviewInboxDataProviding

    init(provider: ReviewInboxDataProviding = MockReviewInboxDataProvider()) {
        self.provider = provider
    }

    var sortedItems: [ReviewInboxDisplayItem] {
        items
            .filter { item in
                switch filter {
                case .all:
                    return true
                case .highRisk:
                    return item.riskLevel == .critical || item.riskLevel == .high
                case .needsConfirmation:
                    return item.status == .needsConfirmation || item.status == .new
                case .executed:
                    return item.status == .executed
                }
            }
            .sorted {
                if $0.riskLevel.rank == $1.riskLevel.rank {
                    return $0.confidenceScore > $1.confidenceScore
                }
                return $0.riskLevel.rank > $1.riskLevel.rank
            }
    }

    var selectedItem: ReviewInboxDisplayItem? {
        guard let selectedItemID else { return sortedItems.first }
        return items.first { $0.id == selectedItemID }
    }

    var evidenceForSelectedItem: [ReviewInboxDisplayEvidence] {
        guard let selectedItem else { return [] }
        return evidence
            .filter { $0.reviewItemId == selectedItem.id }
            .sorted { $0.trustScore > $1.trustScore }
    }

    var actionsForSelectedItem: [ReviewInboxSuggestedAction] {
        guard let selectedItem else { return [] }
        return actions.filter { $0.reviewItemId == selectedItem.id }
    }

    var selectedAction: ReviewInboxSuggestedAction? {
        if let selectedActionID,
           let action = actions.first(where: { $0.id == selectedActionID }) {
            return action
        }
        return actionsForSelectedItem.first
    }

    var ledgerForSelectedAction: [ReviewInboxTimelineEvent] {
        guard let selectedAction else { return [] }
        let events = ledgerEvents.filter { $0.actionId == selectedAction.id }
        if !events.isEmpty { return events }
        return ReviewInboxTimelineStage.allCases.enumerated().map { index, stage in
            ReviewInboxTimelineEvent(
                id: "pending-\(selectedAction.id)-\(index)",
                actionId: selectedAction.id,
                stage: stage,
                stageStatus: index == 0 && selectedAction.gateState == .approved ? .ok : .pending,
                timestamp: "未执行",
                message: stage == .confirmedAction ? "等待人工确认。" : "等待上一阶段完成。",
                idempotencyKey: "tenant:t_demo:pa:\(selectedAction.reviewItemId):v1:confirm"
            )
        }
    }

    var criticalCount: Int {
        items.filter { $0.riskLevel == .critical }.count
    }

    var pendingGateCount: Int {
        actions.filter { $0.gateState == .pending }.count
    }

    var executedCount: Int {
        items.filter { $0.status == .executed }.count
    }

    var currentSnapshot: ReviewInboxDisplaySnapshot {
        ReviewInboxDisplaySnapshot(items: items, evidence: evidence, actions: actions, ledgerEvents: ledgerEvents)
    }

    var canSubmitSelectedAction: Bool {
        guard let selectedAction else { return false }
        return selectedAction.gateState == .pending && selectedAction.canEnterProductionExecution && !isSubmittingDecision
    }

    func load() async {
        guard loadState != .loading else { return }
        loadState = .loading
        lastErrorMessage = nil

        do {
            applySnapshot(try await provider.loadSnapshot())
            loadState = .ready
        } catch {
            let message = "复盘收件箱加载失败：\(error.localizedDescription)"
            lastErrorMessage = message
            loadState = .failed(message)
        }
    }

    func select(_ item: ReviewInboxDisplayItem) {
        selectedItemID = item.id
        selectedActionID = actions.first { $0.reviewItemId == item.id }?.id
        confirmationNote = ""
    }

    func selectAction(_ action: ReviewInboxSuggestedAction) {
        selectedActionID = action.id
        confirmationNote = ""
    }

    func approveSelectedAction() async {
        guard let action = selectedAction else { return }
        guard action.canEnterProductionExecution else {
            lastErrorMessage = "当前生产入口只开放进展创建 / 更新，其它动作先保留为草稿。"
            return
        }
        await submit(
            .approve(
                actionID: action.id,
                version: action.version,
                expectedSyncCursor: selectedItem?.syncCursor,
                note: confirmationNote
            )
        )
    }

    func rejectSelectedAction() async {
        guard let action = selectedAction else { return }
        await submit(
            .reject(
                actionID: action.id,
                version: action.version,
                expectedSyncCursor: selectedItem?.syncCursor,
                note: confirmationNote
            )
        )
    }

    private func submit(_ decision: ReviewInboxDecisionCommand) async {
        guard !isSubmittingDecision else { return }
        isSubmittingDecision = true
        lastErrorMessage = nil

        do {
            let updated = try await provider.submitDecision(decision, snapshot: currentSnapshot)
            applySnapshot(updated)
            confirmationNote = ""
        } catch {
            lastErrorMessage = "决策提交失败：\(error.localizedDescription)"
        }

        isSubmittingDecision = false
    }

    private func applySnapshot(_ snapshot: ReviewInboxDisplaySnapshot) {
        items = snapshot.items
        evidence = snapshot.evidence
        actions = snapshot.actions
        ledgerEvents = snapshot.ledgerEvents

        if selectedItemID == nil || !items.contains(where: { $0.id == selectedItemID }) {
            selectedItemID = sortedItems.first?.id
        }

        if selectedActionID == nil || !actions.contains(where: { $0.id == selectedActionID }) {
            selectedActionID = actionsForSelectedItem.first?.id
        }
    }
}
