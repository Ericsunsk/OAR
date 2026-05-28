import Foundation

enum AgentRole: Equatable, Codable {
    case assistant
    case user

    var openAIRole: String {
        switch self {
        case .assistant:
            return "assistant"
        case .user:
            return "user"
        }
    }
}

struct AgentMessage: Identifiable, Equatable {
    let id: UUID
    let role: AgentRole
    let text: String

    init(id: UUID = UUID(), role: AgentRole, text: String) {
        self.id = id
        self.role = role
        self.text = text
    }
}

enum AgentStreamEvent: Equatable {
    case delta(String)
    case completed
}

struct AgentConversationContext: Equatable {
    var title: String
    var riskReason: String
    var actionSummary: String
    var evidenceSummaries: [String]

    static let empty = AgentConversationContext(
        title: "未选择风险",
        riskReason: "暂无风险说明。",
        actionSummary: "暂无建议动作。",
        evidenceSummaries: []
    )
}

struct AgentSettings: Equatable {
    static let defaultBaseURL = URL(string: "https://api.openai.com/v1")!

    var baseURL: URL
    var model: String
    var hasAPIKey: Bool

    static let empty = AgentSettings(
        baseURL: defaultBaseURL,
        model: "",
        hasAPIKey: false
    )
}

struct ResolvedAgentSettings: Equatable {
    let baseURL: URL
    let model: String
    let apiKey: String
}

enum AgentSettingsError: LocalizedError, Equatable {
    case invalidBaseURL
    case missingModel
    case missingAPIKey
    case secretStoreUnavailable

    var errorDescription: String? {
        switch self {
        case .invalidBaseURL:
            return "Base URL 无效。"
        case .missingModel:
            return "请填写模型名称。"
        case .missingAPIKey:
            return "请填写 API Key。"
        case .secretStoreUnavailable:
            return "密钥存储暂时不可用。"
        }
    }
}

enum AgentProviderError: LocalizedError, Equatable {
    case missingConfiguration
    case unauthorized
    case invalidResponse
    case serverUnavailable

    var errorDescription: String? {
        switch self {
        case .missingConfiguration:
            return "请先在 Agent 设置中配置模型服务。"
        case .unauthorized:
            return "模型服务认证失败，请检查 API Key。"
        case .invalidResponse:
            return "模型服务返回了无法识别的响应。"
        case .serverUnavailable:
            return "模型服务暂时不可用。"
        }
    }
}
