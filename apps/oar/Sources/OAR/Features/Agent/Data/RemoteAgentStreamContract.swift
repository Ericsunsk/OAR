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
        evidenceSummaries = context.evidenceSummaries
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
    case delta(String)
    case completed
    case error(RemoteAgentStreamErrorDTO)
}

struct RemoteAgentStreamErrorDTO: Decodable, Equatable {
    let code: String?
}

private enum RemoteAgentStreamEventKind: Decodable, Equatable {
    case delta
    case completed
    case error
    case unknown(String)

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        switch value {
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
    let delta: String?
    let streamError: RemoteAgentStreamErrorDTO

    enum CodingKeys: String, CodingKey {
        case event
        case delta
        case error
        case code
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        event = try container.decode(RemoteAgentStreamEventKind.self, forKey: .event)
        delta = try container.decodeIfPresent(String.self, forKey: .delta)
        let code = try container.decodeIfPresent(String.self, forKey: .code)
            ?? container.decodeIfPresent(String.self, forKey: .error)
        streamError = RemoteAgentStreamErrorDTO(code: code)
    }
}
