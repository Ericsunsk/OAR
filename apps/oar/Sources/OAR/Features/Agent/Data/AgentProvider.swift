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

                    if let error = Self.streamError(forStatusCode: httpResponse.statusCode) {
                        throw error
                    }
                    try await forwardSuccessfulStream(bytes: bytes, to: continuation)
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private static func streamError(forStatusCode statusCode: Int) -> AgentProviderError? {
        switch statusCode {
        case 200..<300:
            return nil
        case 401, 403:
            return .unauthorized
        case 404, 406, 422, 429, 500..<600:
            return .serverUnavailable
        default:
            return .invalidResponse
        }
    }

    private func forwardSuccessfulStream(
        bytes: URLSession.AsyncBytes,
        to continuation: AsyncThrowingStream<AgentStreamEvent, Error>.Continuation
    ) async throws {
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

        throw AgentProviderError.invalidResponse
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
