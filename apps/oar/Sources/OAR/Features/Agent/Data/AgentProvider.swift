import Foundation

protocol AgentProviding {
    var isAvailable: Bool { get }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> AsyncThrowingStream<AgentStreamEvent, Error>
}

struct RemoteAgentProvider: AgentProviding {
    let baseURL: URL
    let appSession: AppSession
    let urlSession: URLSession
    let decoder: JSONDecoder
    let encoder: JSONEncoder

    var isAvailable: Bool { true }

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

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let request = try agentStreamRequest(messages: messages, context: context)
                    let (bytes, response) = try await urlSession.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw AgentProviderError.invalidResponse
                    }

                    switch httpResponse.statusCode {
                    case 200..<300:
                        var didYieldContent = false
                        let streamEvents = RemoteAgentEventSequence(
                            events: ServerSentEventSequence(bytes: bytes),
                            decoder: decoder
                        )
                        for try await event in streamEvents {
                            switch event {
                            case .delta:
                                didYieldContent = true
                                continuation.yield(event)
                            case .completed:
                                guard didYieldContent else {
                                    throw AgentProviderError.invalidResponse
                                }
                                continuation.yield(.completed)
                                continuation.finish()
                                return
                            }

                            if Task.isCancelled { return }
                        }

                        guard didYieldContent else {
                            throw AgentProviderError.invalidResponse
                        }
                        continuation.finish()
                    case 401, 403:
                        throw AgentProviderError.unauthorized
                    case 404, 406, 422, 429, 500..<600:
                        throw AgentProviderError.serverUnavailable
                    default:
                        throw AgentProviderError.invalidResponse
                    }
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private func agentStreamRequest(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) throws -> URLRequest {
        var request = URLRequest(url: baseURL.appendingPathComponent("agent/stream"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(appSession.sessionID)", forHTTPHeaderField: "Authorization")
        request.httpBody = try encoder.encode(
            RemoteAgentStreamRequestDTO(
                messages: messages.map {
                    RemoteAgentMessageDTO(role: $0.role.backendRole, text: $0.text)
                },
                context: RemoteAgentContextDTO(context: context)
            )
        )
        return request
    }
}

struct MissingBackendAgentProvider: AgentProviding {
    var isAvailable: Bool { false }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            continuation.finish(throwing: AgentProviderError.missingBackendConfiguration)
        }
    }
}

struct MockAgentProvider: AgentProviding {
    var isAvailable: Bool { true }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        let latest = messages.last?.text ?? ""
        let reply: String
        if latest.contains("理由") || latest.contains("备注") || latest.contains("起草") || latest.contains("动作") {
            reply = "可以先起草：基于当前焦点、摘要证据和 dry-run 影响范围，建议推进“\(context.actionSummary)”。执行前仍需在 OAR 中确认。"
        } else if latest.contains("证据") {
            reply = "我会先把证据分成已支持和待补充两类。当前可见风险信号是：\(context.riskReason)；如果要下结论，建议补充负责人最新口径。"
        } else {
            reply = "建议先扫描高风险项，再围绕“\(context.actionSummary)”确认影响范围、证据缺口和人工确认路径。"
        }

        return AsyncThrowingStream { continuation in
            continuation.yield(.delta(reply))
            continuation.yield(.completed)
            continuation.finish()
        }
    }
}

enum AgentProviderFactory {
    static func makeProvider(
        appSession: AppSession,
        environment: AppEnvironment = .current()
    ) -> AgentProviding {
        if let baseURL = environment.oarBackendBaseURL {
            return RemoteAgentProvider(baseURL: baseURL, appSession: appSession)
        }

        if environment.allowsMockAgentFallback {
            return MockAgentProvider()
        }

        return MissingBackendAgentProvider()
    }
}

private struct RemoteAgentStreamRequestDTO: Encodable {
    let messages: [RemoteAgentMessageDTO]
    let context: RemoteAgentContextDTO
}

private struct RemoteAgentMessageDTO: Encodable {
    let role: String
    let text: String
}

private struct RemoteAgentContextDTO: Encodable {
    let title: String
    let riskReason: String
    let actionSummary: String
    let evidenceSummaries: [String]

    enum CodingKeys: String, CodingKey {
        case title
        case riskReason = "risk_reason"
        case actionSummary = "action_summary"
        case evidenceSummaries = "evidence_summaries"
    }

    init(context: AgentConversationContext) {
        title = context.title
        riskReason = context.riskReason
        actionSummary = context.actionSummary
        evidenceSummaries = context.evidenceSummaries
    }
}

private struct RemoteAgentEventSequence<Base: AsyncSequence>: AsyncSequence where Base.Element == ServerSentEvent {
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
