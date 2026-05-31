import Foundation

protocol AgentSettingsProviding {
    var isAvailable: Bool { get }

    func loadSettings() async throws -> AgentModelSettingsSnapshot
    func detectModels(baseURL: String, apiKey: String?) async throws -> AgentModelCatalog
    func saveSettings(baseURL: String, apiKey: String?, selectedModel: String) async throws -> AgentModelSettingsSnapshot
    func clearSettings() async throws -> AgentModelSettingsSnapshot
}

struct AgentModelSettingsSnapshot: Equatable {
    let source: AgentModelSettingsSource
    let detectedProtocol: String?
    let baseURL: String?
    let selectedModel: String?
    let apiKeyStatus: AgentAPIKeyStatus
    let canConfigure: Bool
}

enum AgentModelSettingsSource: String, Decodable, Equatable {
    case user
    case env
    case none
}

enum AgentAPIKeyStatus: String, Decodable, Equatable {
    case saved
    case missing
}

struct AgentModelCatalog: Equatable {
    let detectedProtocol: String
    let models: [AgentModelCandidate]
    let recommendedModel: String?
}

struct AgentModelCandidate: Identifiable, Equatable {
    let id: String
    let displayName: String
}

enum AgentSettingsProviderError: LocalizedError {
    case missingBackendConfiguration
    case unauthorized
    case invalidResponse
    case detectionFailed
    case invalidAPIKey
    case serverUnavailable

    var errorDescription: String? {
        switch self {
        case .missingBackendConfiguration:
            return "Agent 设置需要连接 OAR 后端。"
        case .unauthorized:
            return "当前 OAR 会话已失效，请重新登录。"
        case .invalidResponse:
            return "Agent 设置服务返回了无法识别的响应。"
        case .detectionFailed:
            return "无法根据 Base URL 和 API Key 检测模型。"
        case .invalidAPIKey:
            return "模型服务拒绝了这个 API Key，请检查是否复制了完整密钥。"
        case .serverUnavailable:
            return "Agent 设置服务暂时不可用。"
        }
    }
}

struct MissingBackendAgentSettingsProvider: AgentSettingsProviding {
    var isAvailable: Bool { false }

    func loadSettings() async throws -> AgentModelSettingsSnapshot {
        throw AgentSettingsProviderError.missingBackendConfiguration
    }

    func detectModels(baseURL: String, apiKey: String?) async throws -> AgentModelCatalog {
        throw AgentSettingsProviderError.missingBackendConfiguration
    }

    func saveSettings(baseURL: String, apiKey: String?, selectedModel: String) async throws -> AgentModelSettingsSnapshot {
        throw AgentSettingsProviderError.missingBackendConfiguration
    }

    func clearSettings() async throws -> AgentModelSettingsSnapshot {
        throw AgentSettingsProviderError.missingBackendConfiguration
    }
}

struct RemoteAgentSettingsProvider: AgentSettingsProviding {
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

    func loadSettings() async throws -> AgentModelSettingsSnapshot {
        let endpoint = baseURL.appendingPathComponent("agent/settings")
        let data = try await performRequest(URLRequest(url: endpoint))
        return try decodeResponse(AgentModelSettingsSnapshotDTO.self, from: data).toDomain()
    }

    func detectModels(baseURL: String, apiKey: String?) async throws -> AgentModelCatalog {
        let endpoint = self.baseURL.appendingPathComponent("agent/model-catalog/preview")
        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try encoder.encode(AgentModelCatalogRequestDTO(baseURL: baseURL, apiKey: apiKey))

        let data = try await performRequest(request)
        return try decodeResponse(AgentModelCatalogDTO.self, from: data).toDomain()
    }

    func saveSettings(baseURL: String, apiKey: String?, selectedModel: String) async throws -> AgentModelSettingsSnapshot {
        let endpoint = self.baseURL.appendingPathComponent("agent/settings")
        var request = URLRequest(url: endpoint)
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try encoder.encode(
            AgentSettingsUpdateRequestDTO(
                baseURL: baseURL,
                apiKey: apiKey,
                selectedModel: selectedModel
            )
        )

        let data = try await performRequest(request)
        return try decodeResponse(AgentModelSettingsSnapshotDTO.self, from: data).toDomain()
    }

    func clearSettings() async throws -> AgentModelSettingsSnapshot {
        let endpoint = baseURL.appendingPathComponent("agent/settings")
        var request = URLRequest(url: endpoint)
        request.httpMethod = "DELETE"

        let data = try await performRequest(request)
        return try decodeResponse(AgentModelSettingsSnapshotDTO.self, from: data).toDomain()
    }

    private func decodeResponse<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try decoder.decode(type, from: data)
        } catch {
            throw AgentSettingsProviderError.invalidResponse
        }
    }

    private func performRequest(_ request: URLRequest) async throws -> Data {
        var request = request
        request.setValue("Bearer \(appSession.sessionID)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let (data, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AgentSettingsProviderError.invalidResponse
        }

        switch httpResponse.statusCode {
        case 200..<300:
            return data
        case 401, 403:
            throw AgentSettingsProviderError.unauthorized
        case 400, 422:
            if Self.backendErrorCode(from: data) == "agent_settings_api_key_rejected" {
                throw AgentSettingsProviderError.invalidAPIKey
            }
            throw AgentSettingsProviderError.detectionFailed
        case 500..<600:
            throw AgentSettingsProviderError.serverUnavailable
        default:
            throw AgentSettingsProviderError.invalidResponse
        }
    }

    private static func backendErrorCode(from data: Data) -> String? {
        try? JSONDecoder().decode(AgentSettingsErrorDTO.self, from: data).error
    }
}

enum AgentSettingsProviderFactory {
    static func makeProvider(
        appSession: AppSession,
        environment: AppEnvironment = .current()
    ) -> AgentSettingsProviding {
        if let baseURL = environment.oarBackendBaseURL {
            return RemoteAgentSettingsProvider(baseURL: baseURL, appSession: appSession)
        }

        return MissingBackendAgentSettingsProvider()
    }
}
