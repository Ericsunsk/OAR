import XCTest
@testable import OAR

final class AgentSettingsStoreTests: XCTestCase {
    private var suiteName: String!
    private var userDefaults: UserDefaults!
    private var secretStore: InMemoryAgentSecretStore!

    override func setUp() {
        super.setUp()
        suiteName = "AgentSettingsStoreTests-\(UUID().uuidString)"
        userDefaults = UserDefaults(suiteName: suiteName)!
        secretStore = InMemoryAgentSecretStore()
    }

    override func tearDown() {
        userDefaults.removePersistentDomain(forName: suiteName)
        suiteName = nil
        userDefaults = nil
        secretStore = nil
        super.tearDown()
    }

    func testSavePersistsModelBaseURLAndAPIKeySeparately() throws {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        let settings = try store.save(
            baseURLString: "https://llm.example.test/v1",
            model: "oar-model",
            apiKey: "sk-test"
        )
        let resolved = try store.resolve()

        XCTAssertEqual(settings.baseURL.absoluteString, "https://llm.example.test/v1")
        XCTAssertEqual(settings.model, "oar-model")
        XCTAssertTrue(settings.hasAPIKey)
        XCTAssertEqual(resolved.apiKey, "sk-test")
        XCTAssertNil(userDefaults.string(forKey: "openai-compatible-api-key"))
    }

    func testSaveWithoutAPIKeyKeepsExistingSecret() throws {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        _ = try store.save(
            baseURLString: "https://llm.example.test/v1",
            model: "model-a",
            apiKey: "sk-original"
        )
        _ = try store.save(
            baseURLString: "https://llm.example.test/v1",
            model: "model-b",
            apiKey: nil
        )

        let resolved = try store.resolve()
        XCTAssertEqual(resolved.model, "model-b")
        XCTAssertEqual(resolved.apiKey, "sk-original")
    }

    func testInvalidBaseURLIsRejected() {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        XCTAssertThrowsError(
            try store.save(baseURLString: "file:///tmp/model", model: "model", apiKey: "sk")
        ) { error in
            XCTAssertEqual(error as? AgentSettingsError, .invalidBaseURL)
        }
    }

    func testSaveRejectsFileSchemeEvenWithLocalhostHost() {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        XCTAssertThrowsError(
            try store.save(baseURLString: "file://localhost/tmp/model", model: "model", apiKey: "sk")
        ) { error in
            XCTAssertEqual(error as? AgentSettingsError, .invalidBaseURL)
        }
    }

    func testSaveAllowsHTTPForLocalhost() throws {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        let settings = try store.save(
            baseURLString: "http://localhost:8080/v1",
            model: "oar-model",
            apiKey: "sk-test"
        )

        XCTAssertEqual(settings.baseURL.absoluteString, "http://localhost:8080/v1")
    }

    func testSaveAllowsHTTPSForRemoteHost() throws {
        let store = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)

        let settings = try store.save(
            baseURLString: "https://api.example.com/v1",
            model: "oar-model",
            apiKey: "sk-test"
        )

        XCTAssertEqual(settings.baseURL.absoluteString, "https://api.example.com/v1")
    }
}

private final class InMemoryAgentSecretStore: AgentSecretStoring {
    private var apiKey: String?

    func readAPIKey() throws -> String? {
        apiKey
    }

    func saveAPIKey(_ apiKey: String) throws {
        self.apiKey = apiKey
    }

    func deleteAPIKey() throws {
        apiKey = nil
    }
}
