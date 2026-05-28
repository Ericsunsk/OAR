import Foundation

protocol AgentProviding {
    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) -> AsyncThrowingStream<AgentStreamEvent, Error>
}

struct OpenAICompatibleAgentProvider: AgentProviding {
    let urlSession: URLSession
    let decoder: JSONDecoder
    let encoder: JSONEncoder
    let promptBuilder: AgentSystemPromptBuilder

    init(
        urlSession: URLSession = .shared,
        decoder: JSONDecoder = JSONDecoder(),
        encoder: JSONEncoder = JSONEncoder(),
        promptBuilder: AgentSystemPromptBuilder = AgentSystemPromptBuilder()
    ) {
        self.urlSession = urlSession
        self.decoder = decoder
        self.encoder = encoder
        self.promptBuilder = promptBuilder
    }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let request = try chatCompletionsRequest(
                        messages: messages,
                        context: context,
                        settings: settings
                    )
                    let (bytes, response) = try await urlSession.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw AgentProviderError.invalidResponse
                    }

                    switch httpResponse.statusCode {
                    case 200..<300:
                        var didYieldContent = false
                        let streamEvents = OpenAIChatCompletionEventSequence(
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
                    case 429, 500..<600:
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

    private func chatCompletionsRequest(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) throws -> URLRequest {
        var request = URLRequest(url: chatCompletionsURL(baseURL: settings.baseURL))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(settings.apiKey)", forHTTPHeaderField: "Authorization")
        request.httpBody = try encoder.encode(
            OpenAIChatCompletionRequestDTO(
                model: settings.model,
                messages: requestMessages(messages: messages, context: context),
                temperature: 0.2,
                stream: true
            )
        )
        return request
    }

    private func chatCompletionsURL(baseURL: URL) -> URL {
        baseURL.appendingPathComponent("chat/completions")
    }

    private func requestMessages(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> [OpenAIChatMessageDTO] {
        var requestMessages = [
            OpenAIChatMessageDTO(role: "system", content: promptBuilder.makePrompt(context: context))
        ]
        requestMessages.append(
            contentsOf: messages.suffix(12).map {
                OpenAIChatMessageDTO(role: $0.role.openAIRole, content: $0.text)
            }
        )
        return requestMessages
    }
}

private struct OpenAIChatCompletionRequestDTO: Encodable {
    let model: String
    let messages: [OpenAIChatMessageDTO]
    let temperature: Double
    let stream: Bool
}

private struct OpenAIChatMessageDTO: Encodable {
    let role: String
    let content: String
}

struct MockAgentProvider: AgentProviding {
    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        let latest = messages.last?.text ?? ""
        let reply: String
        if latest.contains("理由") || latest.contains("备注") {
            reply = "可以写：已核对当前摘要证据和 dry-run 影响范围，同意先执行“\(context.actionSummary)”。"
        } else if latest.contains("证据") {
            reply = "当前证据可以解释风险，但建议补充负责人最新口径。风险点是：\(context.riskReason)"
        } else {
            reply = "建议先围绕“\(context.actionSummary)”确认影响范围，并保留人工确认。"
        }

        return AsyncThrowingStream { continuation in
            continuation.yield(.delta(reply))
            continuation.yield(.completed)
            continuation.finish()
        }
    }
}
