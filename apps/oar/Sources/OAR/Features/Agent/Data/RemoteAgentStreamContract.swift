import Foundation

struct RemoteAgentStreamRequestDTO: Encodable {
    let messages: [RemoteAgentMessageDTO]
    let context: RemoteAgentContextDTO
}

struct RemoteAgentMessageDTO: Encodable {
    let role: String
    let text: String
}

struct RemoteAgentContextDTO: Encodable {
    let title: String
    let riskReason: String
    let actionSummary: String
    let evidenceSummaries: [String]
    private let evidenceRefs: [RemoteAgentEvidenceRefDTO]
    let workspaceSummary: String
    let workspaceSignals: [String]
    let pendingActionSummaries: [String]
    let ledgerEventSummaries: [String]

    enum CodingKeys: String, CodingKey {
        case title
        case riskReason = "risk_reason"
        case actionSummary = "action_summary"
        case evidenceSummaries = "evidence_summaries"
        case evidenceRefs = "evidence_refs"
        case workspaceSummary = "workspace_summary"
        case workspaceSignals = "workspace_signals"
        case pendingActionSummaries = "pending_action_summaries"
        case ledgerEventSummaries = "ledger_event_summaries"
    }

    init(context: AgentConversationContext) {
        title = context.title
        riskReason = context.riskReason
        actionSummary = context.actionSummary
        evidenceSummaries = context.canonicalEvidenceSummaries
        evidenceRefs = context.evidenceRefs.map(RemoteAgentEvidenceRefDTO.init(ref:))
        workspaceSummary = context.workspaceSummary
        workspaceSignals = context.workspaceSignals
        pendingActionSummaries = context.pendingActionSummaries
        ledgerEventSummaries = context.ledgerEventSummaries
    }
}

private struct RemoteAgentEvidenceRefDTO: Encodable {
    let sourceType: String
    let sourceRef: String
    let summary: String

    enum CodingKeys: String, CodingKey {
        case sourceType = "source_type"
        case sourceRef = "source_ref"
        case summary
    }

    init(ref: AgentEvidenceRef) {
        sourceType = ref.sourceType
        sourceRef = ref.sourceRef
        summary = ref.summary
    }
}

struct RemoteAgentEventSequence<Base: AsyncSequence>: AsyncSequence where Base.Element == ServerSentEvent {
    typealias Element = RemoteAgentStreamEvent

    let events: Base
    let decoder: JSONDecoder

    func makeAsyncIterator() -> Iterator {
        Iterator(eventIterator: events.makeAsyncIterator(), decoder: decoder)
    }

    struct Iterator: AsyncIteratorProtocol {
        var eventIterator: Base.AsyncIterator
        let decoder: JSONDecoder

        mutating func next() async throws -> RemoteAgentStreamEvent? {
            while let event = try await eventIterator.next() {
                let dto: RemoteAgentStreamEventDTO
                do {
                    dto = try decoder.decode(
                        RemoteAgentStreamEventDTO.self,
                        from: Data(event.data.utf8)
                    )
                } catch {
                    throw AgentProviderError.invalidResponse
                }
                switch dto.event {
                case .contextStatus:
                    guard let status = dto.status else {
                        throw AgentProviderError.invalidResponse
                    }
                    return .contextStatus(status.domain)
                case .delta:
                    guard let delta = dto.delta,
                          !delta.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    else { continue }
                    return .delta(delta)
                case .completed:
                    return .completed
                case .error:
                    return .error(dto.streamError)
                case .unknown:
                    continue
                }
            }
            return nil
        }
    }
}

enum RemoteAgentStreamEvent: Equatable {
    case contextStatus(AgentContextStatus)
    case delta(String)
    case completed
    case error(RemoteAgentStreamErrorDTO)
}

struct RemoteAgentStreamErrorDTO: Decodable, Equatable {
    let code: String?
}

private enum RemoteAgentStreamEventKind: Decodable, Equatable {
    case contextStatus
    case delta
    case completed
    case error
    case unknown(String)

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        switch value {
        case "context_status":
            self = .contextStatus
        case "delta":
            self = .delta
        case "completed":
            self = .completed
        case "error":
            self = .error
        default:
            self = .unknown(value)
        }
    }
}

private struct RemoteAgentStreamEventDTO: Decodable {
    let event: RemoteAgentStreamEventKind
    let status: RemoteAgentContextStatusDTO?
    let delta: String?
    let streamError: RemoteAgentStreamErrorDTO

    enum CodingKeys: String, CodingKey {
        case event
        case status
        case delta
        case error
        case code
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        event = try container.decode(RemoteAgentStreamEventKind.self, forKey: .event)
        status = try container.decodeIfPresent(RemoteAgentContextStatusDTO.self, forKey: .status)
        delta = try container.decodeIfPresent(String.self, forKey: .delta)
        let code = try container.decodeIfPresent(String.self, forKey: .code)
            ?? container.decodeIfPresent(String.self, forKey: .error)
        streamError = RemoteAgentStreamErrorDTO(code: code)
    }
}

private struct RemoteAgentContextStatusDTO: Decodable {
    private static let maxSummaryCount = 4
    private static let maxSummaryCharacters = 240

    let activatedSkillSummaries: [String]
    let liveReadSummaries: [String]

    enum CodingKeys: String, CodingKey {
        case activatedSkillSummaries = "activated_skill_summaries"
        case liveReadSummaries = "live_read_summaries"
    }

    var domain: AgentContextStatus {
        AgentContextStatus(
            activatedSkillSummaries: Self.visibleSummaries(activatedSkillSummaries),
            liveReadSummaries: Self.visibleSummaries(liveReadSummaries)
        )
    }

    private static func visibleSummaries(_ summaries: [String]) -> [String] {
        summaries
            .map { $0.split(whereSeparator: \.isWhitespace).joined(separator: " ") }
            .filter { !$0.isEmpty }
            .prefix(maxSummaryCount)
            .map { summary in
                guard summary.count > maxSummaryCharacters else { return summary }
                return "\(String(summary.prefix(maxSummaryCharacters)))..."
            }
    }
}
