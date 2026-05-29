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
    case decisionPathNotWired
    case remoteRejected(String)
    case missingBackendConfiguration
    case unauthorized
    case serverUnavailable
    case remoteProviderNotConfigured
}

private struct ReviewInboxErrorResponseDTO: Decodable {
    let error: String?
    let reason: String?
    let safeMessage: String?

    enum CodingKeys: String, CodingKey {
        case error
        case reason
        case safeMessage = "safe_message"
    }
}

struct RemoteReviewInboxDataProvider: ReviewInboxDataProviding {
    let baseURL: URL
    let appSession: AppSession
    let urlSession: URLSession
    let decoder: JSONDecoder
    let encoder: JSONEncoder

    init(
        baseURL: URL,
        appSession: AppSession,
        urlSession: URLSession = .shared,
        decoder: JSONDecoder = JSONDecoder(),
        encoder: JSONEncoder = JSONEncoder()
    ) {
        self.baseURL = baseURL
        self.appSession = appSession
        self.urlSession = urlSession
        self.decoder = decoder
        self.encoder = encoder
    }

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        let endpoint = baseURL.appendingPathComponent("review-inbox/snapshot")
        let data = try await performRequest(URLRequest(url: endpoint))
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

    private func performRequest(_ request: URLRequest) async throws -> Data {
        var request = request
        request.setValue("Bearer \(appSession.sessionID)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let (data, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw ReviewInboxDataProviderError.remoteProviderNotConfigured
        }

        switch httpResponse.statusCode {
        case 200..<300:
            return data
        case 401, 403:
            throw ReviewInboxDataProviderError.unauthorized
        case 409:
            throw ReviewInboxDataProviderError.staleSyncCursor
        case 422:
            if let response = try? decoder.decode(ReviewInboxErrorResponseDTO.self, from: data) {
                let code = response.error ?? response.reason
                if code == "review_decision_not_wired" {
                    throw ReviewInboxDataProviderError.decisionPathNotWired
                }
                if let safeMessage = response.safeMessage?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !safeMessage.isEmpty {
                    throw ReviewInboxDataProviderError.remoteRejected(safeMessage)
                }
            }
            throw ReviewInboxDataProviderError.unsupportedAction
        case 500..<600:
            throw ReviewInboxDataProviderError.serverUnavailable
        default:
            throw ReviewInboxDataProviderError.remoteProviderNotConfigured
        }
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
        case .decisionPathNotWired:
            return "当前后端尚未接通复盘决策写入链路。"
        case let .remoteRejected(message):
            return message
        case .missingBackendConfiguration:
            return "请配置 OAR 后端地址后再同步真实复盘数据。"
        case .unauthorized:
            return "登录会话已失效，请重新扫码登录。"
        case .serverUnavailable:
            return "OAR 后端暂时不可用，请稍后重试。"
        case .remoteProviderNotConfigured:
            return "远端复盘收件箱服务尚未配置或返回异常。"
        }
    }
}
