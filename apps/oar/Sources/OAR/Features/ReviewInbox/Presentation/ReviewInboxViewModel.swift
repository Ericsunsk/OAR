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
    private let onSessionInvalidated: @MainActor (String) -> Void
    private var latestLoadRequestID: UInt64 = 0

    init(
        provider: ReviewInboxDataProviding,
        onSessionInvalidated: @escaping @MainActor (String) -> Void = { _ in }
    ) {
        self.provider = provider
        self.onSessionInvalidated = onSessionInvalidated
    }

    var visibleItemCount: Int {
        sortedItems.count
    }

    var selectedItemPositionText: String {
        guard let selectedItem,
              let index = sortedItems.firstIndex(where: { $0.id == selectedItem.id }) else {
            return "0/0"
        }
        return "\(index + 1)/\(sortedItems.count)"
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
        if let item = sortedItems.first(where: { $0.id == selectedItemID }) {
            return item
        }
        return sortedItems.first
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

    var highRiskCount: Int {
        items.filter { $0.riskLevel == .critical || $0.riskLevel == .high }.count
    }

    var criticalCount: Int {
        items.filter { $0.riskLevel == .critical }.count
    }

    var needsConfirmationCount: Int {
        items.filter { $0.status == .needsConfirmation || $0.status == .new }.count
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

    var canMoveToPreviousItem: Bool {
        selectedSortedItemIndex.map { $0 > sortedItems.startIndex } ?? false
    }

    var canMoveToNextItem: Bool {
        guard !sortedItems.isEmpty else { return false }
        return selectedSortedItemIndex.map { $0 < sortedItems.index(before: sortedItems.endIndex) } ?? false
    }

    func load(force: Bool = false) async {
        guard force || loadState != .loading else { return }
        latestLoadRequestID += 1
        let requestID = latestLoadRequestID
        loadState = .loading
        lastErrorMessage = nil

        do {
            let snapshot = try await provider.loadSnapshot()
            guard requestID == latestLoadRequestID else { return }
            applySnapshot(snapshot)
            loadState = .ready
        } catch {
            guard requestID == latestLoadRequestID else { return }
            if let providerError = error as? ReviewInboxDataProviderError,
               case .unauthorized = providerError {
                onSessionInvalidated(providerError.errorDescription ?? "登录会话已失效，请重新扫码登录。")
            }
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

    func setFilter(_ nextFilter: ReviewInboxFilter) {
        filter = nextFilter
        reconcileSelectionWithCurrentFilter()
    }

    func reload() async {
        await load(force: true)
    }

    func selectPreviousItem() {
        guard canMoveToPreviousItem,
              let index = selectedSortedItemIndex else { return }
        select(sortedItems[sortedItems.index(before: index)])
    }

    func selectNextItem() {
        guard canMoveToNextItem,
              let index = selectedSortedItemIndex else { return }
        select(sortedItems[sortedItems.index(after: index)])
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
            if let providerError = error as? ReviewInboxDataProviderError,
               case .unauthorized = providerError {
                onSessionInvalidated(providerError.errorDescription ?? "登录会话已失效，请重新扫码登录。")
            }
            lastErrorMessage = "决策提交失败：\(error.localizedDescription)"
        }

        isSubmittingDecision = false
    }

    private func applySnapshot(_ snapshot: ReviewInboxDisplaySnapshot) {
        items = snapshot.items
        evidence = snapshot.evidence
        actions = snapshot.actions
        ledgerEvents = snapshot.ledgerEvents

        reconcileSelectionWithCurrentFilter()
    }

    private var selectedSortedItemIndex: [ReviewInboxDisplayItem].Index? {
        guard let selectedItem else { return nil }
        return sortedItems.firstIndex(where: { $0.id == selectedItem.id })
    }

    private func reconcileSelectionWithCurrentFilter() {
        if selectedItemID == nil || !sortedItems.contains(where: { $0.id == selectedItemID }) {
            selectedItemID = sortedItems.first?.id
        }

        let selectedItemActionIDs = Set(actionsForSelectedItem.map(\.id))
        if selectedActionID == nil || !selectedItemActionIDs.contains(selectedActionID ?? "") {
            selectedActionID = actionsForSelectedItem.first?.id
        }

        confirmationNote = ""
    }
}
