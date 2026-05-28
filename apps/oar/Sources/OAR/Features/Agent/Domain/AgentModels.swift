import Foundation

enum AgentRole: Equatable, Codable {
    case assistant
    case user

    var backendRole: String {
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

enum AgentProviderError: LocalizedError, Equatable {
    case missingBackendConfiguration
    case unauthorized
    case invalidResponse
    case serverUnavailable

    var errorDescription: String? {
        switch self {
        case .missingBackendConfiguration:
            return "请配置 OAR 后端地址后再使用 Agent。"
        case .unauthorized:
            return "登录会话已失效，请重新扫码登录。"
        case .invalidResponse:
            return "OAR 后端返回了无法识别的 Agent 响应。"
        case .serverUnavailable:
            return "OAR 后端 Agent 服务暂时不可用。"
        }
    }
}
