import XCTest
@testable import OAR

@MainActor
final class AgentSidecarViewModelTests: XCTestCase {
    private var suiteName: String!
    private var userDefaults: UserDefaults!
    private var secretStore: ViewModelTestSecretStore!

    override func setUp() {
        super.setUp()
        suiteName = "AgentSidecarViewModelTests-\(UUID().uuidString)"
        userDefaults = UserDefaults(suiteName: suiteName)!
        secretStore = ViewModelTestSecretStore()
    }

    override func tearDown() {
        userDefaults.removePersistentDomain(forName: suiteName)
        suiteName = nil
        userDefaults = nil
        secretStore = nil
        super.tearDown()
    }

    func testSendAppendsUserAndAssistantMessages() async throws {
        let settingsStore = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)
        _ = try settingsStore.save(
            baseURLString: "https://llm.example.test/v1",
            model: "agent-model",
            apiKey: "sk-test"
        )
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider, settingsStore: settingsStore)

        await model.send("解释风险", context: .empty)

        XCTAssertEqual(model.messages.suffix(2).map(\.role), [.user, .assistant])
        XCTAssertEqual(model.messages.last?.text, "收到。")
        XCTAssertEqual(provider.lastSettings?.model, "agent-model")
        XCTAssertNil(model.errorMessage)
    }

    func testStreamingReplyUpdatesAssistantMessageIncrementally() async throws {
        let settingsStore = try configuredSettingsStore()
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider, settingsStore: settingsStore)

        let sendTask = Task {
            await model.send("解释风险", context: .empty)
        }
        await provider.waitForStream()
        XCTAssertTrue(model.isSending)

        provider.yield("流式")
        await waitForLastMessage("流式", in: model)
        XCTAssertEqual(model.messages.suffix(2).map(\.role), [.user, .assistant])

        provider.yield("回复")
        provider.finish()
        await sendTask.value

        XCTAssertFalse(model.isSending)
        XCTAssertEqual(model.messages.last?.text, "流式回复")
        XCTAssertNil(model.errorMessage)
    }

    func testMissingSettingsDoesNotCallProvider() async {
        let settingsStore = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider, settingsStore: settingsStore)

        await model.send("解释风险", context: .empty)

        XCTAssertEqual(model.messages.last?.role, .user)
        XCTAssertNil(provider.lastSettings)
        XCTAssertEqual(model.errorMessage, AgentSettingsError.missingModel.localizedDescription)
    }

    func testConversationHistoryIsScopedByItemID() async throws {
        let settingsStore = try configuredSettingsStore()
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider, settingsStore: settingsStore)

        model.activateConversation(itemID: "review-a")
        await model.send("解释 A", context: .empty)
        let reviewAThread = model.messages

        model.activateConversation(itemID: "review-b")
        XCTAssertEqual(model.messages.count, 1)

        await model.send("解释 B", context: .empty)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 B", "收到。"])

        model.activateConversation(itemID: "review-a")
        XCTAssertEqual(model.messages, reviewAThread)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A", "收到。"])
    }

    func testLateReplyDoesNotPolluteActiveConversation() async throws {
        let settingsStore = try configuredSettingsStore()
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider, settingsStore: settingsStore)

        model.activateConversation(itemID: "review-a")
        let sendTask = Task {
            await model.send("解释 A", context: .empty)
        }
        await provider.waitForStream()
        XCTAssertTrue(model.isSending)

        model.activateConversation(itemID: "review-b")
        XCTAssertFalse(model.isSending)
        XCTAssertEqual(model.messages.count, 1)

        provider.finish(with: "A 的回复")
        await sendTask.value

        XCTAssertEqual(model.messages.count, 1)
        model.activateConversation(itemID: "review-a")
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A", "A 的回复"])
        XCTAssertFalse(model.isSending)
    }

    private func configuredSettingsStore() throws -> AgentSettingsStore {
        let settingsStore = AgentSettingsStore(userDefaults: userDefaults, secretStore: secretStore)
        _ = try settingsStore.save(
            baseURLString: "https://llm.example.test/v1",
            model: "agent-model",
            apiKey: "sk-test"
        )
        return settingsStore
    }

    private func waitForLastMessage(_ expectedText: String, in model: AgentSidecarViewModel) async {
        for _ in 0..<100 {
            if model.messages.last?.text == expectedText {
                return
            }
            await Task.yield()
        }
        XCTFail("Expected last message to become \(expectedText), got \(model.messages.last?.text ?? "nil")")
    }
}

private final class ManualStreamingAgentProvider: AgentProviding {
    var lastSettings: ResolvedAgentSettings?
    private let immediateReply: String?
    private var continuation: AsyncThrowingStream<AgentStreamEvent, Error>.Continuation?

    init(immediateReply: String? = nil) {
        self.immediateReply = immediateReply
    }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext,
        settings: ResolvedAgentSettings
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
        lastSettings = settings
        if let immediateReply {
            return AsyncThrowingStream { continuation in
                continuation.yield(.delta(immediateReply))
                continuation.yield(.completed)
                continuation.finish()
            }
        }

        return AsyncThrowingStream { continuation in
            self.continuation = continuation
        }
    }

    func waitForStream() async {
        while continuation == nil {
            await Task.yield()
        }
    }

    func yield(_ text: String) {
        continuation?.yield(.delta(text))
    }

    func finish() {
        continuation?.yield(.completed)
        continuation?.finish()
        continuation = nil
    }

    func finish(with text: String) {
        yield(text)
        finish()
    }
}

private final class ViewModelTestSecretStore: AgentSecretStoring {
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
