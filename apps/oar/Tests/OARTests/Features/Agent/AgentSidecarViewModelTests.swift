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

    func testBufferedStreamingFlushesPausedChunksBeforeCompletion() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider, streamFlushInterval: 0.05)

        let sendTask = Task {
            await model.send("解释风险", context: .empty)
        }
        await provider.waitForStream()

        provider.yield("流式")
        await waitForLastMessage("流式", in: model)

        provider.yield("回复")
        await waitForLastMessage("流式回复", in: model)
        XCTAssertTrue(model.isSending)

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

    func testConversationHistoryStaysInWorkspaceThreadWhenFocusChanges() async {
        let provider = ManualStreamingAgentProvider(immediateReply: "收到。")
        let model = AgentSidecarViewModel(provider: provider)

        model.activateFocus(itemID: "review-a")
        await model.send("解释 A", context: .empty)
        let threadAfterReviewA = model.messages

        model.activateFocus(itemID: "review-b")
        XCTAssertEqual(model.activeFocusItemID, "review-b")
        XCTAssertEqual(model.messages, threadAfterReviewA)

        await model.send("解释 B", context: .empty)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A", "收到。", "解释 B", "收到。"])

        model.activateFocus(itemID: "review-a")
        XCTAssertEqual(model.activeFocusItemID, "review-a")
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A", "收到。", "解释 B", "收到。"])
    }

    func testLateReplyContinuesIntoWorkspaceThreadAfterFocusChanges() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)

        model.activateFocus(itemID: "review-a")
        let sendTask = Task {
            await model.send("解释 A", context: .empty)
        }
        await provider.waitForStream()
        XCTAssertTrue(model.isSending)

        model.activateFocus(itemID: "review-b")
        XCTAssertEqual(model.activeFocusItemID, "review-b")
        XCTAssertTrue(model.isSending)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A"])

        provider.finish(with: "A 的回复")
        await sendTask.value

        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释 A", "A 的回复"])
        XCTAssertFalse(model.isSending)
    }

    func testPartialReplyIsPreservedWhenStreamFails() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)

        let sendTask = Task {
            await model.send("解释风险", context: .empty)
        }
        await provider.waitForStream()

        provider.yield("部分回复")
        await waitForLastMessage("部分回复", in: model)
        provider.fail(AgentProviderError.invalidResponse)
        await sendTask.value

        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释风险", "部分回复"])
        XCTAssertEqual(model.errorMessage, AgentProviderError.invalidResponse.localizedDescription)
        XCTAssertFalse(model.isSending)
    }

    func testContextStatusUpdatesTransientStateWithoutAddingMessage() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)
        let status = AgentContextStatus(
            activatedSkillSummaries: ["feishu.okr｜Feishu OKR｜用途：读取 OKR"],
            liveReadSummaries: ["工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。"]
        )

        let sendTask = Task {
            await model.send("看我的 OKR", context: .empty)
        }
        await provider.waitForStream()

        provider.yield(status)
        await waitForContextStatus(status, in: model)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["看我的 OKR"])

        provider.finish(with: "读取完成。")
        await sendTask.value

        XCTAssertEqual(model.contextStatus, status)
        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["看我的 OKR", "读取完成。"])
        XCTAssertNil(model.errorMessage)
    }

    func testContextStatusClearsBeforeNextSend() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider)
        let status = AgentContextStatus(
            activatedSkillSummaries: ["feishu.calendar｜Feishu Calendar"],
            liveReadSummaries: ["实时读取完成"]
        )

        let firstSendTask = Task {
            await model.send("看日历", context: .empty)
        }
        await provider.waitForStream()
        provider.yield(status)
        provider.finish(with: "有 1 个会议。")
        await firstSendTask.value
        XCTAssertEqual(model.contextStatus, status)

        let secondSendTask = Task {
            await model.send("继续", context: .empty)
        }
        await provider.waitForStream()

        XCTAssertNil(model.contextStatus)

        provider.finish(with: "收到。")
        await secondSendTask.value
    }

    func testUnflushedPartialReplyIsPreservedWhenStreamFails() async {
        let provider = ManualStreamingAgentProvider()
        let model = AgentSidecarViewModel(provider: provider, streamFlushInterval: 60)

        let sendTask = Task {
            await model.send("解释风险", context: .empty)
        }
        await provider.waitForStream()

        provider.yield("部分")
        await waitForLastMessage("部分", in: model)
        provider.yield("回复")
        provider.fail(AgentProviderError.invalidResponse)
        await sendTask.value

        XCTAssertEqual(model.messages.dropFirst().map(\.text), ["解释风险", "部分回复"])
        XCTAssertEqual(model.errorMessage, AgentProviderError.invalidResponse.localizedDescription)
        XCTAssertFalse(model.isSending)
    }

    private func waitForLastMessage(_ expectedText: String, in model: AgentSidecarViewModel) async {
        for _ in 0..<100 {
            if model.messages.last?.text == expectedText {
                return
            }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Expected last message to become \(expectedText), got \(model.messages.last?.text ?? "nil")")
    }

    private func waitForContextStatus(
        _ expectedStatus: AgentContextStatus,
        in model: AgentSidecarViewModel
    ) async {
        for _ in 0..<100 {
            if model.contextStatus == expectedStatus {
                return
            }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Expected context status to become \(expectedStatus), got \(String(describing: model.contextStatus))")
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

    func yield(_ status: AgentContextStatus) {
        continuation?.yield(.contextStatus(status))
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

    func fail(_ error: Error) {
        continuation?.finish(throwing: error)
        continuation = nil
    }
}
