import Foundation

@Observable
@MainActor
final class AgentSidecarViewModel {
    private static let fallbackConversationID = "__oar_agent_default__"

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

        do {
            let reply = try await provider.send(
                messages: thread,
                context: context,
                settings: resolvedSettings
            )
            thread.append(reply)
            conversationsByID[conversationID] = thread
            if activeConversationID == conversationID {
                messages = thread
            }
        } catch {
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
}
