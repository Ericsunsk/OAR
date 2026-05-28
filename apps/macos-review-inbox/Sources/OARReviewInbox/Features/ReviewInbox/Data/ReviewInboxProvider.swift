import Foundation

struct ReviewInboxDisplaySnapshot {
    var items: [ReviewInboxDisplayItem]
    var evidence: [ReviewInboxDisplayEvidence]
    var actions: [ReviewInboxSuggestedAction]
    var ledgerEvents: [ReviewInboxTimelineEvent]
}

enum ReviewInboxLoadState: Equatable {
    case idle
    case loading
    case ready
    case failed(String)
}

enum ReviewInboxDecisionCommand {
    case approve(actionID: ReviewInboxSuggestedAction.ID, version: UInt64, expectedSyncCursor: UInt64?, note: String)
    case reject(actionID: ReviewInboxSuggestedAction.ID, version: UInt64, expectedSyncCursor: UInt64?, note: String)
}

protocol ReviewInboxDataProviding {
    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot
    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot
}

enum ReviewInboxDataProviderError: Error {
    case actionNotFound
    case actionVersionMismatch
    case staleSyncCursor
    case unsupportedAction
    case remoteProviderNotConfigured
}

final class MockReviewInboxDataProvider: ReviewInboxDataProviding {
    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        ReviewInboxDisplaySnapshot(
            items: ReviewInboxMockData.reviewItems,
            evidence: ReviewInboxMockData.evidence,
            actions: ReviewInboxMockData.actions,
            ledgerEvents: ReviewInboxMockData.ledgerEvents
        )
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        var updated = snapshot

        switch decision {
        case let .approve(actionID, version, expectedSyncCursor, note):
            let action = try validateDecision(actionID: actionID, version: version, expectedSyncCursor: expectedSyncCursor, snapshot: snapshot)
            guard action.canEnterProductionExecution else {
                throw ReviewInboxDataProviderError.unsupportedAction
            }
            try applyDecision(actionID: actionID, gateState: .approved, itemStatus: .confirmed, note: note, snapshot: &updated)
        case let .reject(actionID, version, expectedSyncCursor, note):
            _ = try validateDecision(actionID: actionID, version: version, expectedSyncCursor: expectedSyncCursor, snapshot: snapshot)
            try applyDecision(
                actionID: actionID,
                gateState: .rejected,
                itemStatus: .rejected,
                note: note.isEmpty ? "人工拒绝。" : note,
                snapshot: &updated
            )
        }

        return updated
    }

    private func validateDecision(
        actionID: ReviewInboxSuggestedAction.ID,
        version: UInt64,
        expectedSyncCursor: UInt64?,
        snapshot: ReviewInboxDisplaySnapshot
    ) throws -> ReviewInboxSuggestedAction {
        guard let action = snapshot.actions.first(where: { $0.id == actionID }) else {
            throw ReviewInboxDataProviderError.actionNotFound
        }
        guard action.version == version else {
            throw ReviewInboxDataProviderError.actionVersionMismatch
        }
        guard action.gateState == .pending else {
            throw ReviewInboxDataProviderError.unsupportedAction
        }
        if let expectedSyncCursor,
           let item = snapshot.items.first(where: { $0.id == action.reviewItemId }),
           item.syncCursor != expectedSyncCursor {
            throw ReviewInboxDataProviderError.staleSyncCursor
        }
        return action
    }

    private func applyDecision(
        actionID: ReviewInboxSuggestedAction.ID,
        gateState: ReviewInboxGateState,
        itemStatus: ReviewInboxDisplayStatus,
        note: String,
        snapshot: inout ReviewInboxDisplaySnapshot
    ) throws {
        guard let actionIndex = snapshot.actions.firstIndex(where: { $0.id == actionID }) else {
            throw ReviewInboxDataProviderError.actionNotFound
        }

        let action = snapshot.actions[actionIndex]
        snapshot.actions[actionIndex].gateState = gateState

        if let itemIndex = snapshot.items.firstIndex(where: { $0.id == action.reviewItemId }) {
            snapshot.items[itemIndex].status = itemStatus
        }

        snapshot.ledgerEvents.removeAll { $0.actionId == action.id }
        snapshot.ledgerEvents.append(contentsOf: mockLedgerEvents(for: action, gateState: gateState, note: note))
    }

    private func mockLedgerEvents(for action: ReviewInboxSuggestedAction, gateState: ReviewInboxGateState, note: String) -> [ReviewInboxTimelineEvent] {
        let status: ReviewInboxTimelineStatus = gateState == .approved ? .ok : .error
        let key = "tenant:t_demo:pa:\(action.reviewItemId):v1:confirm"
        let messages: [(ReviewInboxTimelineStage, String)] = [
            (.confirmedAction, gateState == .approved ? "人工确认。\(note)" : "人工拒绝。\(note)"),
            (.operationLedger, gateState == .approved ? "模拟写入执行账本。" : "拒绝后不创建执行记录。"),
            (.larkAdapter, gateState == .approved ? "原型不真实写回，适配器为模拟状态。" : "未调用适配器。"),
            (.auditEvent, "本地审计链路已更新。")
        ]

        return messages.enumerated().map { index, entry in
            ReviewInboxTimelineEvent(
                id: "led-\(action.id)-\(index)-\(UUID().uuidString.prefix(6))",
                actionId: action.id,
                stage: entry.0,
                stageStatus: index == 0 || gateState == .approved ? status : .pending,
                timestamp: "Now",
                message: entry.1.trimmingCharacters(in: .whitespaces),
                idempotencyKey: key
            )
        }
    }
}

struct RemoteReviewInboxDataProvider: ReviewInboxDataProviding {
    let baseURL: URL
    let urlSession: URLSession
    let decoder: JSONDecoder
    let encoder: JSONEncoder

    init(
        baseURL: URL,
        urlSession: URLSession = .shared,
        decoder: JSONDecoder = JSONDecoder(),
        encoder: JSONEncoder = JSONEncoder()
    ) {
        self.baseURL = baseURL
        self.urlSession = urlSession
        self.decoder = decoder
        self.encoder = encoder
    }

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        let endpoint = baseURL.appendingPathComponent("review-inbox/snapshot")
        let data = try await performRequest(endpoint)
        return try decoder.decode(ReviewInboxAPISnapshot.self, from: data).toDisplaySnapshot()
    }

    func submitDecision(_ decision: ReviewInboxDecisionCommand, snapshot: ReviewInboxDisplaySnapshot) async throws -> ReviewInboxDisplaySnapshot {
        let actionID: String
        let actionVersion: UInt64
        let decisionKind: ProposedActionDecisionDTO
        let note: String
        let expectedSyncCursor: UInt64?

        switch decision {
        case let .approve(id, version, syncCursor, submittedNote):
            actionID = id
            actionVersion = version
            decisionKind = .confirm
            note = submittedNote
            expectedSyncCursor = syncCursor
        case let .reject(id, version, syncCursor, submittedNote):
            actionID = id
            actionVersion = version
            decisionKind = .reject
            note = submittedNote
            expectedSyncCursor = syncCursor
        }

        let endpoint = baseURL.appendingPathComponent("review-inbox/decisions")
        let payload = ReviewDecisionDTO(
            actionID: actionID,
            actionVersion: actionVersion,
            decision: decisionKind,
            note: note,
            expectedSyncCursor: expectedSyncCursor
        )
        var urlRequest = URLRequest(url: endpoint)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.httpBody = try encoder.encode(payload)

        let data = try await performRequest(urlRequest)
        return try decoder.decode(ReviewInboxAPISnapshot.self, from: data).toDisplaySnapshot()
    }

    private func performRequest(_ url: URL) async throws -> Data {
        try await performRequest(URLRequest(url: url))
    }

    private func performRequest(_ request: URLRequest) async throws -> Data {
        let (data, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              200..<300 ~= httpResponse.statusCode else {
            throw ReviewInboxDataProviderError.remoteProviderNotConfigured
        }
        return data
    }
}

extension ReviewInboxDataProviderError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case .actionNotFound:
            return "找不到对应建议动作。"
        case .actionVersionMismatch:
            return "建议动作版本已变化，请重新同步。"
        case .staleSyncCursor:
            return "复盘项已被其他端更新，请重新同步。"
        case .unsupportedAction:
            return "当前动作不在生产执行白名单内。"
        case .remoteProviderNotConfigured:
            return "远端复盘收件箱服务尚未配置或返回异常。"
        }
    }
}
