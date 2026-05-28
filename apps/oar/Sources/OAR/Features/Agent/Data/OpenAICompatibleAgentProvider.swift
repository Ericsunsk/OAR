import Foundation

protocol AgentProviding {
    func send(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) async throws -> AgentMessage
}

struct OpenAICompatibleAgentProvider: AgentProviding {
    let urlSession: URLSession
    let decoder: JSONDecoder
    let encoder: JSONEncoder

    init(
        urlSession: URLSession = .shared,
        decoder: JSONDecoder = JSONDecoder(),
        encoder: JSONEncoder = JSONEncoder()
    ) {
        self.urlSession = urlSession
        self.decoder = decoder
        self.encoder = encoder
    }

    func send(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) async throws -> AgentMessage {
        var request = URLRequest(url: chatCompletionsURL(baseURL: settings.baseURL))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(settings.apiKey)", forHTTPHeaderField: "Authorization")
        request.httpBody = try encoder.encode(
            OpenAIChatCompletionRequestDTO(
                model: settings.model,
                messages: requestMessages(messages: messages, context: context),
                temperature: 0.2
            )
        )

        let (data, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AgentProviderError.invalidResponse
        }

        switch httpResponse.statusCode {
        case 200..<300:
            let dto = try decoder.decode(OpenAIChatCompletionResponseDTO.self, from: data)
            guard let text = dto.choices.first?.message.content
                .trimmingCharacters(in: .whitespacesAndNewlines),
                !text.isEmpty else {
                throw AgentProviderError.invalidResponse
            }
            return AgentMessage(role: .assistant, text: text)
        case 401, 403:
            throw AgentProviderError.unauthorized
        case 429, 500..<600:
            throw AgentProviderError.serverUnavailable
        default:
            throw AgentProviderError.invalidResponse
        }
    }

    private func chatCompletionsURL(baseURL: URL) -> URL {
        baseURL.appendingPathComponent("chat/completions")
    }

    private func requestMessages(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> [OpenAIChatMessageDTO] {
        var requestMessages = [
            OpenAIChatMessageDTO(role: "system", content: systemPrompt(context: context))
        ]
        requestMessages.append(
            contentsOf: messages.suffix(12).map {
                OpenAIChatMessageDTO(role: $0.role.openAIRole, content: $0.text)
            }
        )
        return requestMessages
    }

    private func systemPrompt(context: AgentConversationContext) -> String {
        let evidence = context.evidenceSummaries.isEmpty
            ? "暂无摘要证据。"
            : context.evidenceSummaries.prefix(4).enumerated().map { index, summary in
                "\(index + 1). \(summary)"
            }.joined(separator: "\n")

        return """
        你是 OAR 的复盘辅助 Agent。只基于客户端提供的上下文回答，不要声称已经读取外部系统。
        你不能执行写操作，不能要求用户跳过人工确认，不能输出敏感 token 或密钥。
        回答要简洁、可操作，优先解释证据、风险、确认理由和下一步审计点。

        当前复盘项：\(context.title)
        风险原因：\(context.riskReason)
        建议动作：\(context.actionSummary)
        摘要证据：
        \(evidence)
        """
    }
}

private struct OpenAIChatCompletionRequestDTO: Encodable {
    let model: String
    let messages: [OpenAIChatMessageDTO]
    let temperature: Double
}

private struct OpenAIChatMessageDTO: Codable {
    let role: String
    let content: String
}

private struct OpenAIChatCompletionResponseDTO: Decodable {
    let choices: [Choice]

    struct Choice: Decodable {
        let message: OpenAIChatMessageDTO
    }
}

struct MockAgentProvider: AgentProviding {
    func send(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) async throws -> AgentMessage {
        let latest = messages.last?.text ?? ""
        if latest.contains("理由") || latest.contains("备注") {
            return AgentMessage(
                role: .assistant,
                text: "可以写：已核对当前摘要证据和 dry-run 影响范围，同意先执行“\(context.actionSummary)”。"
            )
        }
        if latest.contains("证据") {
            return AgentMessage(
                role: .assistant,
                text: "当前证据可以解释风险，但建议补充负责人最新口径。风险点是：\(context.riskReason)"
            )
        }
        return AgentMessage(role: .assistant, text: "建议先围绕“\(context.actionSummary)”确认影响范围，并保留人工确认。")
    }
}
