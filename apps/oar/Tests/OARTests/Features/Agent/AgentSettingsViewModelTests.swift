import XCTest
@testable import OAR

@MainActor
final class AgentSettingsViewModelTests: XCTestCase {
    func testEditingBaseURLAfterDetectInvalidatesCatalogAndPreventsSave() async {
        let provider = RecordingAgentSettingsProvider()
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = "https://api.example.test/v1"
        model.apiKey = "sk-one"

        await model.detect()

        XCTAssertTrue(model.canSave)
        XCTAssertEqual(model.detectedProtocol, "openai-compatible")
        XCTAssertEqual(model.selectedModelID, "gpt-4.1")

        model.baseURL = "https://api.other.test/v1"

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
        model.baseURL = "https://api.example.test/v1"
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
        model.baseURL = "https://api.example.test/v1"
        model.apiKey = "sk-one"

        await model.detect()
        model.baseURL = "https://api.other.test/v1"
        provider.nextCatalog = AgentModelCatalog(
            detectedProtocol: "anthropic",
            models: [
                AgentModelCandidate(id: "claude-sonnet-4-5", displayName: "Claude Sonnet 4.5")
            ],
            recommendedModel: "claude-sonnet-4-5"
        )

        await model.detect()

        XCTAssertTrue(model.canSave)
        XCTAssertEqual(provider.detectRequests.map(\.baseURL), [
            "https://api.example.test/v1",
            "https://api.other.test/v1"
        ])

        await model.save()

        XCTAssertEqual(provider.saveRequests.count, 1)
        XCTAssertEqual(provider.saveRequests[0].baseURL, "https://api.other.test/v1")
        XCTAssertEqual(provider.saveRequests[0].selectedModel, "claude-sonnet-4-5")
    }

    func testSaveUsesCurrentSelectedModelFromDetectedCatalog() async {
        let provider = RecordingAgentSettingsProvider()
        provider.nextCatalog = AgentModelCatalog(
            detectedProtocol: "openai-compatible",
            models: [
                AgentModelCandidate(id: "gpt-4.1", displayName: "gpt-4.1"),
                AgentModelCandidate(id: "gpt-4o", displayName: "gpt-4o")
            ],
            recommendedModel: "gpt-4.1"
        )
        let model = AgentSettingsViewModel(provider: provider)
        model.baseURL = "https://api.example.test/v1"
        model.apiKey = "sk-one"

        await model.detect()
        model.selectedModelID = "gpt-4o"
        await model.save()

        XCTAssertEqual(provider.saveRequests.count, 1)
        XCTAssertEqual(provider.saveRequests[0].selectedModel, "gpt-4o")
        XCTAssertEqual(model.apiKey, "")
        XCTAssertEqual(model.detectedProtocol, "openai-compatible")
        XCTAssertEqual(model.selectedModelID, "gpt-4o")
        XCTAssertTrue(model.canSave)
    }
}

private final class RecordingAgentSettingsProvider: AgentSettingsProviding {
    var isAvailable: Bool = true
    var nextCatalog = AgentModelCatalog(
        detectedProtocol: "openai-compatible",
        models: [
            AgentModelCandidate(id: "gpt-4.1", displayName: "gpt-4.1")
        ],
        recommendedModel: "gpt-4.1"
    )
    private(set) var detectRequests: [DetectRequest] = []
    private(set) var saveRequests: [SaveRequest] = []

    func loadSettings() async throws -> AgentModelSettingsSnapshot {
        AgentModelSettingsSnapshot(
            source: .none,
            detectedProtocol: nil,
            baseURL: nil,
            selectedModel: nil,
            apiKeyStatus: .missing,
            canConfigure: true
        )
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
        return AgentModelSettingsSnapshot(
            source: .user,
            detectedProtocol: nextCatalog.detectedProtocol,
            baseURL: baseURL,
            selectedModel: selectedModel,
            apiKeyStatus: .saved,
            canConfigure: true
        )
    }

    func clearSettings() async throws -> AgentModelSettingsSnapshot {
        AgentModelSettingsSnapshot(
            source: .none,
            detectedProtocol: nil,
            baseURL: nil,
            selectedModel: nil,
            apiKeyStatus: .missing,
            canConfigure: true
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
