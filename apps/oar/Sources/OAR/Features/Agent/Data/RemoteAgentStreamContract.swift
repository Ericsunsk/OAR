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

    enum CodingKeys: String, CodingKey {
        case title
        case riskReason = "risk_reason"
        case actionSummary = "action_summary"
        case evidenceSummaries = "evidence_summaries"
        case evidenceRefs = "evidence_refs"
        case workspaceSummary = "workspace_summary"
        case workspaceSignals = "workspace_signals"
        case pendingActionSummaries = "pending_action_summaries"
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
    typealias Element = AgentStreamEvent

    let events: Base
    let decoder: JSONDecoder

    func makeAsyncIterator() -> Iterator {
        Iterator(eventIterator: events.makeAsyncIterator(), decoder: decoder)
    }

    struct Iterator: AsyncIteratorProtocol {
        var eventIterator: Base.AsyncIterator
        let decoder: JSONDecoder

        mutating func next() async throws -> AgentStreamEvent? {
            while let event = try await eventIterator.next() {
                let dto = try decoder.decode(RemoteAgentStreamEventDTO.self, from: Data(event.data.utf8))
                switch dto.event {
                case "delta":
                    guard let delta = dto.delta, !delta.isEmpty else { continue }
                    return .delta(delta)
                case "completed":
                    return .completed
                case "error":
                    throw AgentProviderError.serverUnavailable
                default:
                    continue
                }
            }
            return nil
        }
    }
}

private struct RemoteAgentStreamEventDTO: Decodable {
    let event: String
    let delta: String?
}
