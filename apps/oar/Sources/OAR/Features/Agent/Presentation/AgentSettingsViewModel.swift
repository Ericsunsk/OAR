import Foundation

@Observable
@MainActor
final class AgentSettingsViewModel {
    var baseURL = ""
    var apiKey = ""
    var selectedModelID = ""
    var detectedProtocol: String?
    var models: [AgentModelCandidate] = []
    var source: AgentModelSettingsSource = .none
    var apiKeyStatus: AgentAPIKeyStatus = .missing
    var canConfigure = false
    var isLoading = false
    var isDetecting = false
    var isSaving = false
    var statusMessage: String?
    var errorMessage: String?

    private let provider: AgentSettingsProviding
    private var savedUserBaseURL: String?

    init(provider: AgentSettingsProviding) {
        self.provider = provider
        canConfigure = provider.isAvailable
    }

    var canDetect: Bool {
        canConfigure
            && !isDetecting
            && !baseURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && hasUsableAPIKey
    }

    var canSave: Bool {
        canConfigure
            && !isSaving
            && !baseURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && hasUsableAPIKey
            && !selectedModelID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && detectedProtocol != nil
    }

    private var hasUsableAPIKey: Bool {
        !trimmedAPIKey.isEmpty || hasSavedUserKeyForCurrentBaseURL
    }

    private var hasSavedUserKeyForCurrentBaseURL: Bool {
        guard source == .user, apiKeyStatus == .saved else { return false }
        return savedUserBaseURL == baseURL.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var trimmedAPIKey: String {
        apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func load() async {
        guard provider.isAvailable else {
            canConfigure = false
            errorMessage = AgentSettingsProviderError.missingBackendConfiguration.localizedDescription
            return
        }

        isLoading = true
        defer { isLoading = false }

        do {
            apply(snapshot: try await provider.loadSettings())
            errorMessage = nil
        } catch {
            errorMessage = localizedMessage(error)
        }
    }

    func detect() async {
        guard canDetect else { return }
        isDetecting = true
        statusMessage = nil
        errorMessage = nil
        defer { isDetecting = false }

        do {
            let catalog = try await provider.detectModels(
                baseURL: baseURL,
                apiKey: trimmedAPIKey.nilIfEmpty
            )
            detectedProtocol = catalog.detectedProtocol
            models = catalog.models
            selectedModelID = catalog.recommendedModel ?? catalog.models.first?.id ?? ""
            statusMessage = "已检测到 \(catalog.models.count) 个模型"
        } catch {
            models = []
            selectedModelID = ""
            detectedProtocol = nil
            errorMessage = localizedMessage(error)
        }
    }

    func save() async {
        guard canSave else { return }
        isSaving = true
        statusMessage = nil
        errorMessage = nil
        defer { isSaving = false }

        do {
            let snapshot = try await provider.saveSettings(
                baseURL: baseURL,
                apiKey: trimmedAPIKey.nilIfEmpty,
                selectedModel: selectedModelID
            )
            apply(snapshot: snapshot)
            apiKey = ""
            statusMessage = "已保存"
        } catch {
            errorMessage = localizedMessage(error)
        }
    }

    func clear() async {
        guard canConfigure, !isSaving else { return }
        isSaving = true
        statusMessage = nil
        errorMessage = nil
        defer { isSaving = false }

        do {
            let snapshot = try await provider.clearSettings()
            apply(snapshot: snapshot)
            apiKey = ""
            models = []
            statusMessage = "已清除"
        } catch {
            errorMessage = localizedMessage(error)
        }
    }

    private func apply(snapshot: AgentModelSettingsSnapshot) {
        source = snapshot.source
        detectedProtocol = snapshot.detectedProtocol
        baseURL = snapshot.baseURL ?? ""
        savedUserBaseURL = snapshot.source == .user ? snapshot.baseURL : nil
        selectedModelID = snapshot.selectedModel ?? ""
        apiKeyStatus = snapshot.apiKeyStatus
        canConfigure = snapshot.canConfigure
        if selectedModelID.isEmpty {
            models = []
        } else if !models.contains(where: { $0.id == selectedModelID }) {
            models = [
                AgentModelCandidate(
                    id: selectedModelID,
                    displayName: selectedModelID
                )
            ]
        }
    }

    private func localizedMessage(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? "Agent 设置暂时不可用。"
    }
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
    }
}
