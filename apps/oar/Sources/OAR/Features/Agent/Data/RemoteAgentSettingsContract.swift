struct AgentModelCatalogRequestDTO: Encodable {
    let baseURL: String
    let apiKey: String?

    enum CodingKeys: String, CodingKey {
        case baseURL = "base_url"
        case apiKey = "api_key"
    }
}

struct AgentSettingsUpdateRequestDTO: Encodable {
    let baseURL: String
    let apiKey: String?
    let selectedModel: String

    enum CodingKeys: String, CodingKey {
        case baseURL = "base_url"
        case apiKey = "api_key"
        case selectedModel = "selected_model"
    }
}

struct AgentSettingsErrorDTO: Decodable {
    let error: String
}

struct AgentModelSettingsSnapshotDTO: Decodable {
    let source: AgentModelSettingsSource
    let detectedProtocol: String?
    let baseURL: String?
    let selectedModel: String?
    let apiKeyStatus: AgentAPIKeyStatus
    let canConfigure: Bool

    enum CodingKeys: String, CodingKey {
        case source
        case detectedProtocol = "detected_protocol"
        case baseURL = "base_url"
        case selectedModel = "selected_model"
        case apiKeyStatus = "api_key_status"
        case canConfigure = "can_configure"
    }

    func toDomain() -> AgentModelSettingsSnapshot {
        AgentModelSettingsSnapshot(
            source: source,
            detectedProtocol: detectedProtocol,
            baseURL: baseURL,
            selectedModel: selectedModel,
            apiKeyStatus: apiKeyStatus,
            canConfigure: canConfigure
        )
    }
}

struct AgentModelCatalogDTO: Decodable {
    let detectedProtocol: String
    private let models: [AgentModelCandidateDTO]
    let recommendedModel: String?

    enum CodingKeys: String, CodingKey {
        case detectedProtocol = "detected_protocol"
        case models
        case recommendedModel = "recommended_model"
    }

    func toDomain() -> AgentModelCatalog {
        AgentModelCatalog(
            detectedProtocol: detectedProtocol,
            models: models.map { $0.toDomain() },
            recommendedModel: recommendedModel
        )
    }
}

private struct AgentModelCandidateDTO: Decodable {
    let id: String
    let displayName: String

    enum CodingKeys: String, CodingKey {
        case id
        case displayName = "display_name"
    }

    func toDomain() -> AgentModelCandidate {
        AgentModelCandidate(id: id, displayName: displayName)
    }
}
