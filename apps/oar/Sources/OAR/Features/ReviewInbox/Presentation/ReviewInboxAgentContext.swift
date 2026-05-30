import Foundation

extension ReviewInboxViewModel {
    var agentWorkspaceContext: AgentConversationContext {
        guard let selectedItem else {
            return AgentConversationContext(
                title: AgentConversationContext.empty.title,
                riskReason: AgentConversationContext.empty.riskReason,
                actionSummary: AgentConversationContext.empty.actionSummary,
                evidenceSummaries: [],
                evidenceRefs: [],
                workspaceSummary: agentWorkspaceSummary,
                workspaceSignals: agentWorkspaceSignals,
                pendingActionSummaries: agentPendingActionSummaries,
                ledgerEventSummaries: agentLedgerEventSummaries
            )
        }

        let selectedEvidence = evidenceForSelectedItem
        let selectedEvidenceSummaries = selectedEvidence.map { safeAgentSummary($0.summary) }
        return AgentConversationContext(
            title: selectedItem.keyResultTitle,
            riskReason: selectedItem.riskReason,
            actionSummary: agentSelectedActionSummary,
            evidenceSummaries: selectedEvidenceSummaries,
            evidenceRefs: zip(selectedEvidence, selectedEvidenceSummaries).map { evidence, summary in
                AgentEvidenceRef(
                    sourceType: evidence.sourceType.rawValue,
                    sourceRef: evidence.sourceRef,
                    summary: summary
                )
            },
            workspaceSummary: agentWorkspaceSummary,
            workspaceSignals: agentWorkspaceSignals,
            pendingActionSummaries: agentPendingActionSummaries,
            ledgerEventSummaries: agentLedgerEventSummaries
        )
    }

    private var agentSelectedActionSummary: String {
        guard let selectedAction else { return AgentConversationContext.empty.actionSummary }
        let dryRunSummary = safeAgentSummary(selectedAction.dryRunResultSummary)
        let dryRunText = dryRunSummary.isEmpty ? "暂无 dry-run 摘要。" : "dry-run：\(dryRunSummary)"
        return "\(selectedAction.actionType.rawValue)：\(safeAgentSummary(selectedAction.rationale)) \(dryRunText)"
    }

    private var agentWorkspaceSummary: String {
        guard !items.isEmpty else {
            return "工作区摘要：当前没有风险项；筛选“\(filter.rawValue)”显示 0 个，当前焦点 0/0。"
        }

        return "工作区摘要：共 \(items.count) 个风险，严重/高 \(highRiskCount) 个（严重 \(criticalCount) 个），待确认 \(needsConfirmationCount) 个，已执行 \(executedCount) 个；当前筛选“\(filter.rawValue)”显示 \(visibleItemCount) 个，当前焦点 \(selectedItemPositionText)。"
    }

    private var agentWorkspaceSignals: [String] {
        let riskSignals = sortedItems.prefix(4).map { item in
            "\(item.riskLevel.rawValue)｜\(safeAgentSummary(item.keyResultTitle, maxCharacters: 80))｜owner：\(safeAgentSummary(item.ownerName, maxCharacters: 40))｜置信 \(agentConfidenceText(item.confidenceScore))｜状态：\(item.status.rawValue)｜原因：\(safeAgentSummary(item.riskReason))"
        }
        let evidenceGaps = agentEvidenceGapSummaries.prefix(2)
        return Array((riskSignals + evidenceGaps).prefix(5))
    }

    private var agentPendingActionSummaries: [String] {
        let actionPairs = sortedItems.flatMap { item in
            actions
                .filter { action in
                    action.reviewItemId == item.id && action.isPendingOrDraftForAgent
                }
                .map { action in (item, action) }
        }

        return actionPairs.prefix(5).map { item, action in
            let dryRunSummary = safeAgentSummary(action.dryRunResultSummary)
            let dryRunText = dryRunSummary.isEmpty ? "暂无 dry-run 摘要。" : "dry-run：\(dryRunSummary)"
            return "\(safeAgentSummary(item.keyResultTitle, maxCharacters: 80))｜\(action.actionType.rawValue)｜gate：\(action.gateState.rawValue)｜\(dryRunText)"
        }
    }

    private var agentLedgerEventSummaries: [String] {
        guard selectedItem != nil else { return [] }

        let selectedActionID = selectedAction?.id
        let selectedItemActionIDs = Set(actionsForSelectedItem.map(\.id))

        let scopedEvents: [ReviewInboxTimelineEvent]
        if let selectedActionID {
            let selectedEvents = ledgerEvents.filter { $0.actionId == selectedActionID }
            let relatedEvents = ledgerEvents.filter {
                $0.actionId != selectedActionID && selectedItemActionIDs.contains($0.actionId)
            }
            scopedEvents = selectedEvents + relatedEvents
        } else {
            scopedEvents = ledgerEvents.filter { selectedItemActionIDs.contains($0.actionId) }
        }

        return scopedEvents.prefix(5).map(agentLedgerEventSummary)
    }

    private func agentLedgerEventSummary(_ event: ReviewInboxTimelineEvent) -> String {
        let actionText = actions.first { $0.id == event.actionId }.map { action in
            "ActionID \(safeAgentSummary(action.id, maxCharacters: 48))｜\(action.actionType.rawValue)｜gate：\(action.gateState.rawValue)"
        } ?? "ActionID \(safeAgentSummary(event.actionId, maxCharacters: 48))"
        let message = safeAgentSummary(redactedAgentLedgerText(event.message), maxCharacters: 120)
        let messageText = message.isEmpty ? "无补充说明。" : message
        return "\(event.stage.rawValue)｜\(event.stageStatus.rawValue)｜\(safeAgentSummary(event.timestamp, maxCharacters: 48))｜\(messageText)｜\(actionText)"
    }

    private var agentEvidenceGapSummaries: [String] {
        var candidates: [ReviewInboxDisplayItem] = []
        if let selectedItem {
            candidates.append(selectedItem)
        }
        for item in sortedItems where item.riskLevel == .critical || item.riskLevel == .high {
            if !candidates.contains(where: { $0.id == item.id }) {
                candidates.append(item)
            }
        }

        return candidates.compactMap { item in
            let itemEvidence = evidence.filter { $0.reviewItemId == item.id }
            if itemEvidence.isEmpty {
                return "证据缺口：\(safeAgentSummary(item.keyResultTitle, maxCharacters: 80)) 暂无摘要证据，需补充平台事实后再判断。"
            }
            if itemEvidence.count < 2 {
                return "证据缺口：\(safeAgentSummary(item.keyResultTitle, maxCharacters: 80)) 仅 \(itemEvidence.count) 条摘要证据，需补充负责人最新口径或更多证据。"
            }
            return nil
        }
    }

    private func agentConfidenceText(_ score: Double) -> String {
        "\(Int((score * 100).rounded()))%"
    }

    private func safeAgentSummary(_ text: String, maxCharacters: Int = 180) -> String {
        let cleaned = text
            .split(whereSeparator: \.isWhitespace)
            .joined(separator: " ")
        guard cleaned.count > maxCharacters else { return cleaned }
        return "\(String(cleaned.prefix(maxCharacters)))..."
    }

    private func redactedAgentLedgerText(_ text: String) -> String {
        for marker in sensitiveLedgerMarkers where text.range(of: marker, options: [.caseInsensitive]) != nil {
            return "已隐藏敏感账本详情。"
        }
        return text
    }

    private var sensitiveLedgerMarkers: [String] {
        [
            "access token",
            "auth code",
            "authorization",
            "bearer",
            "client secret",
            "credential",
            "password",
            "raw payload",
            "raw_payload",
            "secret",
            "sk-",
            "token",
            "unredacted"
        ]
    }
}

private extension ReviewInboxSuggestedAction {
    var isPendingOrDraftForAgent: Bool {
        gateState == .pending || gateState == .draft
    }
}
