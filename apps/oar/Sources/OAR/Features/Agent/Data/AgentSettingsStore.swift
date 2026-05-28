import Foundation
import Security

protocol AgentSecretStoring {
    func readAPIKey() throws -> String?
    func saveAPIKey(_ apiKey: String) throws
    func deleteAPIKey() throws
}

final class KeychainAgentSecretStore: AgentSecretStoring {
    private let service: String
    private let account: String

    init(
        service: String = "com.oar.agent.model",
        account: String = "openai-compatible-api-key"
    ) {
        self.service = service
        self.account = account
    }

    func readAPIKey() throws -> String? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess,
              let data = result as? Data,
              let value = String(data: data, encoding: .utf8) else {
            throw AgentSettingsError.secretStoreUnavailable
        }
        return value
    }

    func saveAPIKey(_ apiKey: String) throws {
        try deleteAPIKey()
        var query = baseQuery
        query[kSecValueData as String] = Data(apiKey.utf8)

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw AgentSettingsError.secretStoreUnavailable
        }
    }

    func deleteAPIKey() throws {
        let status = SecItemDelete(baseQuery as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw AgentSettingsError.secretStoreUnavailable
        }
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
    }
}

struct AgentSettingsStore {
    private enum Key {
        static let baseURL = "agent.openai_compatible.base_url"
        static let model = "agent.openai_compatible.model"
    }

    let userDefaults: UserDefaults
    let secretStore: AgentSecretStoring

    init(
        userDefaults: UserDefaults = .standard,
        secretStore: AgentSecretStoring = KeychainAgentSecretStore()
    ) {
        self.userDefaults = userDefaults
        self.secretStore = secretStore
    }

    func load() -> AgentSettings {
        let baseURLString = userDefaults.string(forKey: Key.baseURL)
        let baseURL = baseURLString.flatMap(URL.init(string:)) ?? AgentSettings.defaultBaseURL
        let model = userDefaults.string(forKey: Key.model) ?? ""
        let apiKey: String? = (try? secretStore.readAPIKey()) ?? nil
        let hasAPIKey = apiKey?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false

        return AgentSettings(
            baseURL: baseURL,
            model: model,
            hasAPIKey: hasAPIKey
        )
    }

    func resolve() throws -> ResolvedAgentSettings {
        let settings = load()
        let model = settings.model.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !model.isEmpty else {
            throw AgentSettingsError.missingModel
        }
        guard let apiKey = try secretStore.readAPIKey()?.trimmingCharacters(in: .whitespacesAndNewlines),
              !apiKey.isEmpty else {
            throw AgentSettingsError.missingAPIKey
        }

        return ResolvedAgentSettings(
            baseURL: settings.baseURL,
            model: model,
            apiKey: apiKey
        )
    }

    @discardableResult
    func save(baseURLString: String, model: String, apiKey: String?) throws -> AgentSettings {
        let trimmedBaseURL = baseURLString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let baseURL = URL(string: trimmedBaseURL),
              baseURL.isAllowedAgentBaseURL else {
            throw AgentSettingsError.invalidBaseURL
        }

        let trimmedModel = model.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedModel.isEmpty else {
            throw AgentSettingsError.missingModel
        }

        userDefaults.set(baseURL.absoluteString, forKey: Key.baseURL)
        userDefaults.set(trimmedModel, forKey: Key.model)

        if let apiKey {
            let trimmedAPIKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmedAPIKey.isEmpty {
                try secretStore.saveAPIKey(trimmedAPIKey)
            }
        }

        return load()
    }

    func clearAPIKey() throws {
        try secretStore.deleteAPIKey()
    }
}

private extension URL {
    var isAllowedAgentBaseURL: Bool {
        guard let scheme = scheme?.lowercased(),
              let host = host?.lowercased() else {
            return false
        }

        switch scheme {
        case "https":
            return true
        case "http":
            return host == "localhost" || host == "127.0.0.1" || host == "::1"
        default:
            return false
        }
    }
}
