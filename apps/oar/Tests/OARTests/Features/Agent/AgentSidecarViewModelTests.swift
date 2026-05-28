import XCTest
@testable import OAR

@MainActor
final class AgentSidecarViewModelTests: XCTestCase {
    func testSendAppendsUserAndAssistantMessages() async {
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider)

        await model.send("解释风险", context: .empty)

        XCTAssertEqual(model.messages.suffix(2).map(\.role), [.user, .assistant])
        XCTAssertEqual(model.messages.last?.text, "收到。")
        XCTAssertNil(model.errorMessage)
    }

    func testStreamingReplyUpdatesAssistantMessageIncrementally() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)

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

    func testMissingBackendProviderShowsConfigurationError() async {
        let model = AgentSidecarViewModel(provider: MissingBackendAgentProvider())

        await model.send("解释风险", context: .empty)

        XCTAssertEqual(model.messages.last?.role, .user)
        XCTAssertFalse(model.isConfigured)
        XCTAssertEqual(model.errorMessage, AgentProviderError.missingBackendConfiguration.localizedDescription)
    }

    func testConversationHistoryIsScopedByItemID() async {
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider)

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

    func testLateReplyDoesNotPolluteActiveConversation() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)

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
    var isAvailable: Bool { true }

    private let immediateReply: String?
    private var continuation: AsyncThrowingStream<AgentStreamEvent, Error>.Continuation?

    init(immediateReply: String? = nil) {
        self.immediateReply = immediateReply
    }

    func stream(
        messages: [AgentMessage],
        context: AgentConversationContext
    ) -> AsyncThrowingStream<AgentStreamEvent, Error> {
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
