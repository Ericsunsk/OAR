import XCTest
@testable import OAR

@MainActor
final class AgentSettingsViewModelTests: XCTestCase {
    func testLoadIfNeededLoadsOnceAndMarksMissingConfiguration() async {
        let provider = RecordingAgentSettingsProvider()
        let model = AgentSettingsViewModel(provider: provider)

        await model.loadIfNeeded()
        await model.loadIfNeeded()

        XCTAssertEqual(provider.loadCount, 1)
        XCTAssertEqual(model.configurationState, .missingModel)
        XCTAssertFalse(model.isReadyForChat)
    }

    func testUnavailableProviderMarksChatUnavailable() async {
        let provider = RecordingAgentSettingsProvider()
        provider.isAvailable = false
        let model = AgentSettingsViewModel(provider: provider)

        await model.loadIfNeeded()

        XCTAssertEqual(model.configurationState, .unavailable)
        XCTAssertFalse(model.isReadyForChat)
        XCTAssertFalse(model.canConfigure)
    }

    func testEnvSnapshotIsReadyButDoesNotOfferBlankAPIKeyReuse() async {
        let provider = RecordingAgentSettingsProvider()
        provider.nextSnapshot = Fixture.snapshot(
            source: .env,
            detectedProtocol: Fixture.anthropicProtocol,
            baseURL: Fixture.anthropicBaseURL,
            selectedModel: Fixture.claudeSonnet.id,
            apiKeyStatus: .saved,
            canConfigure: false
        )
        let model = AgentSettingsViewModel(provider: provider)

        await model.load()

        XCTAssertEqual(model.configurationState, .ready)
        XCTAssertTrue(model.isReadyForChat)
        XCTAssertFalse(model.canConfigure)
        XCTAssertFalse(model.canReuseSavedAPIKey)
        XCTAssertEqual(model.apiKeyPlaceholder, "sk-...")
        XCTAssertFalse(model.canDetect)
    }

    func testUserSnapshotAllowsBlankAPIKeyReuseForSameBaseURL() async {
        let provider = RecordingAgentSettingsProvider()
        provider.nextSnapshot = Fixture.snapshot(
            source: .user,
            detectedProtocol: Fixture.openAIProtocol,
            baseURL: Fixture.openAIBaseURL,
            selectedModel: Fixture.gpt41.id,
            apiKeyStatus: .saved,
            canConfigure: true
        )
        let model = AgentSettingsViewModel(provider: provider)

        await model.load()

        XCTAssertEqual(model.configurationState, .ready)
        XCTAssertTrue(model.canReuseSavedAPIKey)
        XCTAssertEqual(model.apiKeyPlaceholder, "已保存，留空复用")
        XCTAssertTrue(model.canDetect)
    }

    func testLoadingSnapshotCollapsesStaleDetectedCatalog() async {
        let provider = RecordingAgentSettingsProvider()
        provider.nextCatalog = Fixture.catalog(
            models: Fixture.openAIModels,
            recommendedModel: Fixture.gpt41.id
        )
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = Fixture.openAIBaseURL
        model.apiKey = "sk-one"

        await model.detect()
        XCTAssertEqual(model.models.count, 2)

        provider.nextSnapshot = Fixture.snapshot(
            source: .env,
            detectedProtocol: Fixture.openAIProtocol,
            baseURL: Fixture.otherOpenAIBaseURL,
            selectedModel: Fixture.gpt41.id,
            apiKeyStatus: .saved,
            canConfigure: true
        )

        await model.load()

        XCTAssertEqual(model.models, [Fixture.gpt41])
        XCTAssertEqual(model.baseURL, Fixture.otherOpenAIBaseURL)
        XCTAssertFalse(model.canReuseSavedAPIKey)
    }

    func testEditingBaseURLAfterDetectInvalidatesCatalogAndPreventsSave() async {
        let provider = RecordingAgentSettingsProvider()
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = Fixture.openAIBaseURL
        model.apiKey = "sk-one"

        await model.detect()

        XCTAssertTrue(model.canSave)
        XCTAssertEqual(model.detectedProtocol, Fixture.openAIProtocol)
        XCTAssertEqual(model.selectedModelID, Fixture.gpt41.id)

        model.baseURL = Fixture.alternateOpenAIBaseURL

        XCTAssertFalse(model.canSave)
        XCTAssertNil(model.detectedProtocol)
        XCTAssertTrue(model.models.isEmpty)
        XCTAssertEqual(model.selectedModelID, "")

        await model.save()

        XCTAssertTrue(provider.saveRequests.isEmpty)
    }

    func testEditingAPIKeyAfterDetectInvalidatesCatalogAndPreventsSave() async {
        let provider = RecordingAgentSettingsProvider()
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = Fixture.openAIBaseURL
        model.apiKey = "sk-one"

        await model.detect()

        XCTAssertTrue(model.canSave)

        model.apiKey = "sk-two"

        XCTAssertFalse(model.canSave)
        XCTAssertNil(model.detectedProtocol)
        XCTAssertTrue(model.models.isEmpty)

        await model.save()

        XCTAssertTrue(provider.saveRequests.isEmpty)
    }

    func testRedetectingCurrentInputsAllowsSave() async {
        let provider = RecordingAgentSettingsProvider()
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = Fixture.openAIBaseURL
        model.apiKey = "sk-one"

        await model.detect()
        model.baseURL = Fixture.alternateOpenAIBaseURL
        provider.nextCatalog = Fixture.catalog(
            detectedProtocol: Fixture.anthropicProtocol,
            models: [Fixture.claudeSonnet],
            recommendedModel: Fixture.claudeSonnet.id
        )

        await model.detect()

        XCTAssertTrue(model.canSave)
        XCTAssertEqual(provider.detectRequests.map(\.baseURL), [
            Fixture.openAIBaseURL,
            Fixture.alternateOpenAIBaseURL
        ])

        await model.save()

        XCTAssertEqual(provider.saveRequests.count, 1)
        XCTAssertEqual(provider.saveRequests[0].baseURL, Fixture.alternateOpenAIBaseURL)
        XCTAssertEqual(provider.saveRequests[0].selectedModel, Fixture.claudeSonnet.id)
    }

    func testSaveUsesCurrentSelectedModelFromDetectedCatalog() async {
        let provider = RecordingAgentSettingsProvider()
        provider.nextCatalog = Fixture.catalog(
            models: Fixture.openAIModels,
            recommendedModel: Fixture.gpt41.id
        )
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = Fixture.openAIBaseURL
        model.apiKey = "sk-one"

        await model.detect()
        model.selectedModelID = Fixture.gpt4o.id
        await model.save()

        XCTAssertEqual(provider.saveRequests.count, 1)
        XCTAssertEqual(provider.saveRequests[0].selectedModel, Fixture.gpt4o.id)
        XCTAssertEqual(model.apiKey, "")
        XCTAssertEqual(model.detectedProtocol, Fixture.openAIProtocol)
        XCTAssertEqual(model.selectedModelID, Fixture.gpt4o.id)
        XCTAssertTrue(model.canSave)
    }
}

private final class RecordingAgentSettingsProvider: AgentSettingsProviding {
    var isAvailable: Bool = true
    var nextSnapshot = Fixture.snapshot()
    var nextCatalog = Fixture.catalog()
    private(set) var loadCount = 0
    private(set) var detectRequests: [DetectRequest] = []
    private(set) var saveRequests: [SaveRequest] = []

    func loadSettings() async throws -> AgentModelSettingsSnapshot {
        loadCount += 1
        return nextSnapshot
    }

    func detectModels(baseURL: String, apiKey: String?) async throws -> AgentModelCatalog {
        detectRequests.append(DetectRequest(baseURL: baseURL, apiKey: apiKey))
        return nextCatalog
    }

    func saveSettings(
        baseURL: String,
        apiKey: String?,
        selectedModel: String
    ) async throws -> AgentModelSettingsSnapshot {
        saveRequests.append(
            SaveRequest(baseURL: baseURL, apiKey: apiKey, selectedModel: selectedModel)
        )
        return Fixture.snapshot(
            source: .user,
            detectedProtocol: nextCatalog.detectedProtocol,
            baseURL: baseURL,
            selectedModel: selectedModel,
            apiKeyStatus: .saved,
            canConfigure: true
        )
    }

    func clearSettings() async throws -> AgentModelSettingsSnapshot {
        Fixture.snapshot()
    }
}

private enum Fixture {
    static let openAIProtocol = "openai-compatible"
    static let anthropicProtocol = "anthropic"
    static let openAIBaseURL = "https://api.example.test/v1"
    static let alternateOpenAIBaseURL = "https://api.other.test/v1"
    static let otherOpenAIBaseURL = "https://other.example.test/v1"
    static let anthropicBaseURL = "https://api.anthropic.com/v1"
    static let gpt41 = AgentModelCandidate(id: "gpt-4.1", displayName: "gpt-4.1")
    static let gpt4o = AgentModelCandidate(id: "gpt-4o", displayName: "gpt-4o")
    static let claudeSonnet = AgentModelCandidate(
        id: "claude-sonnet-4-5",
        displayName: "Claude Sonnet 4.5"
    )
    static let openAIModels = [gpt41, gpt4o]

    static func snapshot(
        source: AgentModelSettingsSource = .none,
        detectedProtocol: String? = nil,
        baseURL: String? = nil,
        selectedModel: String? = nil,
        apiKeyStatus: AgentAPIKeyStatus = .missing,
        canConfigure: Bool = true
    ) -> AgentModelSettingsSnapshot {
        AgentModelSettingsSnapshot(
            source: source,
            detectedProtocol: detectedProtocol,
            baseURL: baseURL,
            selectedModel: selectedModel,
            apiKeyStatus: apiKeyStatus,
            canConfigure: canConfigure
        )
    }

    static func catalog(
        detectedProtocol: String = openAIProtocol,
        models: [AgentModelCandidate] = [gpt41],
        recommendedModel: String? = gpt41.id
    ) -> AgentModelCatalog {
        AgentModelCatalog(
            detectedProtocol: detectedProtocol,
            models: models,
            recommendedModel: recommendedModel
        )
    }
}

private struct DetectRequest: Equatable {
    let baseURL: String
    let apiKey: String?
}

private struct SaveRequest: Equatable {
    let baseURL: String
    let apiKey: String?
    let selectedModel: String
}
