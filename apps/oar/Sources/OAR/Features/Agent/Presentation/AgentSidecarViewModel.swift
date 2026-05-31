import Foundation

private let defaultAgentStreamFlushInterval: TimeInterval = 0.045

@Observable
@MainActor
final class AgentSidecarViewModel {
    private static func initialMessages() -> [AgentMessage] {
        [
            AgentMessage(
                role: .assistant,
                text: "我是工作区级 OAR Agent。可以结合这条线程、当前焦点和后端提供的证据摘要来规划下一步；确认前不会写回飞书。"
            )
        ]
    }

    var messages: [AgentMessage] = AgentSidecarViewModel.initialMessages()
    var isSending = false
    var errorMessage: String?
    private(set) var activeFocusItemID: String?

    private let provider: AgentProviding
    private let streamFlushInterval: TimeInterval

    init(
        provider: AgentProviding,
        streamFlushInterval: TimeInterval = defaultAgentStreamFlushInterval
    ) {
        self.provider = provider
        self.streamFlushInterval = streamFlushInterval
    }

    var isConfigured: Bool {
        provider.isAvailable
    }

    func activateFocus(itemID: String?) {
        activeFocusItemID = normalizedFocusID(for: itemID)
    }

    func send(_ text: String, context: AgentConversationContext) async {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard !isSending else { return }

        var thread = messages
        thread.append(AgentMessage(role: .user, text: trimmed))
        messages = thread
        errorMessage = nil
        isSending = true

        defer {
            isSending = false
        }

        let assistantID = UUID()
        var didStartAssistantReply = false
        let displayBuffer = AgentReplyDisplayBuffer(
            flushInterval: streamFlushInterval
        ) { [weak self] displayText in
            guard let self else { return }
            flushAssistantReply(
                id: assistantID,
                text: displayText,
                thread: &thread,
                didStart: &didStartAssistantReply
            )
        }

        do {
            for try await event in provider.stream(messages: thread, context: context) {
                switch event {
                case .delta(let chunk):
                    displayBuffer.append(chunk)
                case .completed:
                    displayBuffer.finish()
                }
            }

            displayBuffer.finish()
            guard displayBuffer.hasDisplayedContent else {
                throw AgentProviderError.invalidResponse
            }
        } catch {
            displayBuffer.flushLatest()
            let message = (error as? LocalizedError)?.errorDescription ?? "Agent 暂时不可用。"
            errorMessage = message
        }
    }

    private func normalizedFocusID(for itemID: String?) -> String? {
        guard let trimmed = itemID?.trimmingCharacters(in: .whitespacesAndNewlines),
              !trimmed.isEmpty else {
            return nil
        }
        return trimmed
    }

    private func flushAssistantReply(
        id: UUID,
        text: String,
        thread: inout [AgentMessage],
        didStart: inout Bool
    ) {
        if didStart {
            updateAssistantReply(
                id: id,
                text: text,
                in: &thread
            )
        } else {
            thread.append(AgentMessage(id: id, role: .assistant, text: text))
            didStart = true
        }
        messages = thread
    }

    private func updateAssistantReply(id: UUID, text: String, in thread: inout [AgentMessage]) {
        guard let index = thread.firstIndex(where: { $0.id == id }) else { return }
        thread[index] = AgentMessage(id: id, role: .assistant, text: text)
    }
}

@MainActor
private final class AgentReplyDisplayBuffer {
    private let flushInterval: TimeInterval
    private let emit: (String) -> Void
    private var accumulatedText = ""
    private var lastFlushedText = ""
    private var lastFlushDate = Date.distantPast
    private var scheduledFlush: Task<Void, Never>?

    init(flushInterval: TimeInterval, flush: @escaping (String) -> Void) {
        self.flushInterval = flushInterval
        self.emit = flush
    }

    var hasDisplayedContent: Bool {
        !lastFlushedText.isEmpty
    }

    func append(_ chunk: String) {
        guard !chunk.isEmpty else { return }
        accumulatedText += chunk

        let elapsed = Date().timeIntervalSince(lastFlushDate)
        guard elapsed < flushInterval else {
            flushLatest()
            return
        }
        scheduleFlush(after: flushInterval - elapsed)
    }

    func finish() {
        let finalText = accumulatedText.trimmingCharacters(in: .whitespacesAndNewlines)
        flush(finalText)
        cancelScheduledFlush()
    }

    func flushLatest() {
        flush(accumulatedText)
        cancelScheduledFlush()
    }

    private func scheduleFlush(after delay: TimeInterval) {
        guard scheduledFlush == nil else { return }
        scheduledFlush = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.nanoseconds(from: delay))
            guard !Task.isCancelled else { return }
            self?.flushScheduledText()
        }
    }

    private func flushScheduledText() {
        scheduledFlush = nil
        flush(accumulatedText)
    }

    private func flush(_ text: String) {
        guard !text.isEmpty, text != lastFlushedText else { return }
        lastFlushedText = text
        lastFlushDate = Date()
        emit(text)
    }

    private func cancelScheduledFlush() {
        scheduledFlush?.cancel()
        scheduledFlush = nil
    }

    private static func nanoseconds(from interval: TimeInterval) -> UInt64 {
        UInt64((max(0, interval) * 1_000_000_000).rounded(.up))
    }
}
