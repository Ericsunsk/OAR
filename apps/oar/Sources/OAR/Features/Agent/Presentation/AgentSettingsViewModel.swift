import CryptoKit
import Foundation

@Observable
@MainActor
final class AgentSettingsViewModel {
    var baseURL = "" {
        didSet { invalidateDetectedCatalogIfInputChanged() }
    }
    var apiKey = "" {
        didSet { invalidateDetectedCatalogIfInputChanged() }
    }
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
    private var lastDetectedInput: DetectionInput?
    private var isApplyingSnapshot = false
    private var didLoadSettings = false

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
            && !trimmedBaseURL.isEmpty
            && hasUsableAPIKey
            && currentDetectionInput == lastDetectedInput
            && selectedModelIsInDetectedCatalog
            && detectedProtocol != nil
    }

    var configurationState: AgentSettingsConfigurationState {
        if isLoading { return .loading }
        if source != .none, !selectedModelID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return .ready
        }
        return canConfigure ? .missingModel : .unavailable
    }

    var isReadyForChat: Bool {
        configurationState == .ready
    }

    var canReuseSavedAPIKey: Bool {
        hasSavedUserKeyForCurrentBaseURL
    }

    var apiKeyPlaceholder: String {
        canReuseSavedAPIKey ? "已保存，留空复用" : "sk-..."
    }

    private var hasUsableAPIKey: Bool {
        !trimmedAPIKey.isEmpty || hasSavedUserKeyForCurrentBaseURL
    }

    private var hasSavedUserKeyForCurrentBaseURL: Bool {
        guard source == .user, apiKeyStatus == .saved else { return false }
        return savedUserBaseURL == trimmedBaseURL
    }

    private var trimmedBaseURL: String {
        baseURL.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var trimmedAPIKey: String {
        apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var currentDetectionInput: DetectionInput? {
        guard !trimmedBaseURL.isEmpty else { return nil }
        if !trimmedAPIKey.isEmpty {
            return DetectionInput(
                baseURL: trimmedBaseURL,
                apiKey: .explicit(fingerprint: Self.apiKeyFingerprint(trimmedAPIKey))
            )
        }
        if hasSavedUserKeyForCurrentBaseURL {
            return DetectionInput(baseURL: trimmedBaseURL, apiKey: .savedUserKey)
        }
        return nil
    }

    private var selectedModelIsInDetectedCatalog: Bool {
        let selectedModelID = selectedModelID.trimmingCharacters(in: .whitespacesAndNewlines)
        return models.contains(where: { $0.id == selectedModelID })
    }

    func load() async {
        guard provider.isAvailable else {
            canConfigure = false
            errorMessage = AgentSettingsProviderError.missingBackendConfiguration.localizedDescription
            didLoadSettings = true
            return
        }

        isLoading = true
        defer { isLoading = false }

        do {
            apply(snapshot: try await provider.loadSettings())
            errorMessage = nil
            didLoadSettings = true
        } catch {
            errorMessage = localizedMessage(error)
            didLoadSettings = true
        }
    }

    func loadIfNeeded() async {
        guard !didLoadSettings, !isLoading else { return }
        await load()
    }

    func detect() async {
        guard canDetect, let detectionInput = currentDetectionInput else { return }
        let requestBaseURL = trimmedBaseURL
        let requestAPIKey = trimmedAPIKey.nilIfEmpty
        isDetecting = true
        statusMessage = nil
        errorMessage = nil
        defer { isDetecting = false }

        do {
            let catalog = try await provider.detectModels(
                baseURL: requestBaseURL,
                apiKey: requestAPIKey
            )
            guard currentDetectionInput == detectionInput else { return }
            lastDetectedInput = detectionInput
            detectedProtocol = catalog.detectedProtocol
            models = catalog.models
            selectedModelID = catalog.recommendedModel ?? catalog.models.first?.id ?? ""
            statusMessage = "已检测到 \(catalog.models.count) 个模型"
        } catch {
            lastDetectedInput = nil
            models = []
            selectedModelID = ""
            detectedProtocol = nil
            errorMessage = localizedMessage(error)
        }
    }

    func save() async {
        guard canSave else { return }
        let requestBaseURL = trimmedBaseURL
        let requestAPIKey = trimmedAPIKey.nilIfEmpty
        let requestModel = selectedModelID.trimmingCharacters(in: .whitespacesAndNewlines)
        isSaving = true
        statusMessage = nil
        errorMessage = nil
        defer { isSaving = false }

        do {
            let snapshot = try await provider.saveSettings(
                baseURL: requestBaseURL,
                apiKey: requestAPIKey,
                selectedModel: requestModel
            )
            apply(snapshot: snapshot)
            clearAPIKeyAfterSaving()
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
            lastDetectedInput = nil
            statusMessage = "已清除"
        } catch {
            errorMessage = localizedMessage(error)
        }
    }

    private func apply(snapshot: AgentModelSettingsSnapshot) {
        isApplyingSnapshot = true
        defer { isApplyingSnapshot = false }

        source = snapshot.source
        baseURL = snapshot.baseURL ?? ""
        savedUserBaseURL = snapshot.source == .user ? snapshot.baseURL : nil
        detectedProtocol = snapshot.detectedProtocol
        selectedModelID = snapshot.selectedModel ?? ""
        apiKeyStatus = snapshot.apiKeyStatus
        canConfigure = snapshot.canConfigure
        if selectedModelID.isEmpty {
            models = []
        } else {
            models = [
                AgentModelCandidate(
                    id: selectedModelID,
                    displayName: selectedModelID
                )
            ]
        }
        lastDetectedInput = currentDetectionInput
    }

    private func localizedMessage(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? "Agent 设置暂时不可用。"
    }

    private func invalidateDetectedCatalogIfInputChanged() {
        guard !isApplyingSnapshot, lastDetectedInput != nil else { return }
        guard currentDetectionInput != lastDetectedInput else { return }
        lastDetectedInput = nil
        detectedProtocol = nil
        models = []
        selectedModelID = ""
        statusMessage = nil
    }

    private func clearAPIKeyAfterSaving() {
        isApplyingSnapshot = true
        apiKey = ""
        isApplyingSnapshot = false
        lastDetectedInput = currentDetectionInput
    }

    private static func apiKeyFingerprint(_ value: String) -> String {
        let digest = SHA256.hash(data: Data(value.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

enum AgentSettingsConfigurationState: Equatable {
    case unavailable
    case loading
    case missingModel
    case ready
}

private struct DetectionInput: Equatable {
    let baseURL: String
    let apiKey: DetectionAPIKeyInput
}

private enum DetectionAPIKeyInput: Equatable {
    case explicit(fingerprint: String)
    case savedUserKey
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
    }
}
