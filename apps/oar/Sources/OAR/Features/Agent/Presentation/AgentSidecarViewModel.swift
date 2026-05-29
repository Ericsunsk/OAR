import Foundation

@Observable
@MainActor
final class AgentSidecarViewModel {
    private static let streamFlushInterval: TimeInterval = 0.045

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

    init(
        provider: AgentProviding
    ) {
        self.provider = provider
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
        var assistantText = ""
        var didStartAssistantReply = false

        do {
            let displayStream = CoalescedAgentTextStream(
                events: provider.stream(
                    messages: thread,
                    context: context
                ),
                flushInterval: Self.streamFlushInterval
            )
            for try await displayText in displayStream {
                assistantText = displayText
                flushAssistantReply(
                    id: assistantID,
                    text: displayText,
                    thread: &thread,
                    didStart: &didStartAssistantReply
                )
            }

            guard didStartAssistantReply else {
                throw AgentProviderError.invalidResponse
            }
        } catch {
            if !assistantText.isEmpty {
                flushAssistantReply(
                    id: assistantID,
                    text: assistantText,
                    thread: &thread,
                    didStart: &didStartAssistantReply
                )
            }
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

private struct CoalescedAgentTextStream<Base: AsyncSequence>: AsyncSequence where Base.Element == AgentStreamEvent {
    typealias Element = String

    let events: Base
    let flushInterval: TimeInterval

    func makeAsyncIterator() -> Iterator {
        Iterator(eventIterator: events.makeAsyncIterator(), flushInterval: flushInterval)
    }

    struct Iterator: AsyncIteratorProtocol {
        var eventIterator: Base.AsyncIterator
        let flushInterval: TimeInterval
        var accumulatedText = ""
        var lastEmittedText = ""
        var lastEmitDate = Date.distantPast
        var didFinish = false

        mutating func next() async throws -> String? {
            guard !didFinish else { return nil }

            while let event = try await eventIterator.next() {
                switch event {
                case .delta(let chunk):
                    guard !chunk.isEmpty else { continue }
                    accumulatedText += chunk

                    let now = Date()
                    if now.timeIntervalSince(lastEmitDate) >= flushInterval {
                        lastEmitDate = now
                        lastEmittedText = accumulatedText
                        return accumulatedText
                    }
                case .completed:
                    return finish()
                }
            }

            return finish()
        }

        private mutating func finish() -> String? {
            didFinish = true
            let finalText = accumulatedText.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !finalText.isEmpty, finalText != lastEmittedText else { return nil }
            lastEmittedText = finalText
            return finalText
        }
    }
}
