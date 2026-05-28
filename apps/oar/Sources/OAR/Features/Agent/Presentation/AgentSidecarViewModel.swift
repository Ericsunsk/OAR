import Foundation

@Observable
@MainActor
final class AgentSidecarViewModel {
    private static let fallbackConversationID = "__oar_agent_default__"
    private static let streamFlushInterval: TimeInterval = 0.045

    private static func initialMessages() -> [AgentMessage] {
        [
            AgentMessage(
                role: .assistant,
                text: "我只基于当前风险、摘要证据和 dry-run 结果回答。确认前不会写回飞书。"
            )
        ]
    }

    var messages: [AgentMessage] = AgentSidecarViewModel.initialMessages()
    var isSending = false
    var errorMessage: String?
    var settings: AgentSettings

    private let provider: AgentProviding
    private let settingsStore: AgentSettingsStore
    private var activeConversationID = AgentSidecarViewModel.fallbackConversationID
    private var conversationsByID: [String: [AgentMessage]] = [:]
    private var errorsByID: [String: String] = [:]
    private var sendingConversationIDs: Set<String> = []

    init(
        provider: AgentProviding = OpenAICompatibleAgentProvider(),
        settingsStore: AgentSettingsStore = AgentSettingsStore()
    ) {
        self.provider = provider
        self.settingsStore = settingsStore
        self.settings = settingsStore.load()
    }

    var isConfigured: Bool {
        settings.hasAPIKey && !settings.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func activateConversation(itemID: String?) {
        let conversationID = conversationID(for: itemID)
        guard activeConversationID != conversationID else { return }

        conversationsByID[activeConversationID] = messages
        activeConversationID = conversationID
        messages = conversationsByID[conversationID] ?? Self.initialMessages()
        errorMessage = errorsByID[conversationID]
        isSending = sendingConversationIDs.contains(conversationID)
    }

    func reloadSettings() {
        settings = settingsStore.load()
    }

    func send(_ text: String, context: AgentConversationContext) async {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        let conversationID = activeConversationID
        guard !sendingConversationIDs.contains(conversationID) else { return }

        var thread = conversationsByID[conversationID] ?? messages
        thread.append(AgentMessage(role: .user, text: trimmed))
        conversationsByID[conversationID] = thread

        if activeConversationID == conversationID {
            messages = thread
            errorMessage = nil
            isSending = true
        }
        errorsByID[conversationID] = nil
        sendingConversationIDs.insert(conversationID)

        let resolvedSettings: ResolvedAgentSettings
        do {
            resolvedSettings = try settingsStore.resolve()
        } catch {
            let message = (error as? LocalizedError)?.errorDescription ?? "请先配置模型服务。"
            errorsByID[conversationID] = message
            if activeConversationID == conversationID {
                errorMessage = message
                isSending = false
            }
            sendingConversationIDs.remove(conversationID)
            reloadSettings()
            return
        }

        defer {
            sendingConversationIDs.remove(conversationID)
            if activeConversationID == conversationID {
                isSending = false
            }
        }

        let assistantID = UUID()
        var assistantText = ""
        var didStartAssistantReply = false

        do {
            let displayStream = CoalescedAgentTextStream(
                events: provider.stream(
                    messages: thread,
                    context: context,
                    settings: resolvedSettings
                ),
                flushInterval: Self.streamFlushInterval
            )
            for try await displayText in displayStream {
                assistantText = displayText
                flushAssistantReply(
                    id: assistantID,
                    text: displayText,
                    conversationID: conversationID,
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
                    conversationID: conversationID,
                    thread: &thread,
                    didStart: &didStartAssistantReply
                )
            }
            let message = (error as? LocalizedError)?.errorDescription ?? "Agent 暂时不可用。"
            errorsByID[conversationID] = message
            if activeConversationID == conversationID {
                errorMessage = message
            }
        }
    }

    func saveSettings(baseURLString: String, model: String, apiKey: String?) throws {
        settings = try settingsStore.save(
            baseURLString: baseURLString,
            model: model,
            apiKey: apiKey
        )
        errorsByID.removeAll()
        errorMessage = nil
    }

    private func conversationID(for itemID: String?) -> String {
        guard let trimmed = itemID?.trimmingCharacters(in: .whitespacesAndNewlines),
              !trimmed.isEmpty else {
            return Self.fallbackConversationID
        }
        return trimmed
    }

    private func flushAssistantReply(
        id: UUID,
        text: String,
        conversationID: String,
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
        conversationsByID[conversationID] = thread
        if activeConversationID == conversationID {
            messages = thread
        }
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
