import SwiftUI

struct AgentSidecarView: View {
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?
    let evidence: [ReviewInboxDisplayEvidence]

    @State private var model: AgentSidecarViewModel
    @State private var draft = ""
    @State private var showsSettings = false

    @MainActor
    init(
        item: ReviewInboxDisplayItem?,
        action: ReviewInboxSuggestedAction?,
        evidence: [ReviewInboxDisplayEvidence],
        model: AgentSidecarViewModel? = nil
    ) {
        self.item = item
        self.action = action
        self.evidence = evidence
        _model = State(initialValue: model ?? AgentSidecarViewModel())
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            AgentContextCard(item: item, action: action)
                .padding(.horizontal, 16)
                .padding(.bottom, 12)

            Divider()
                .overlay(Color.codexBorder.opacity(0.28))

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 10) {
                        ForEach(model.messages) { message in
                            AgentBubble(message: message)
                                .id(message.id)
                        }

                        if model.isSending {
                            AgentThinkingBubble()
                                .id("agent-thinking")
                        }
                    }
                    .padding(16)
                }
                .scrollIndicators(.hidden)
                .onChange(of: model.messages.count) { _, _ in
                    scrollToBottom(proxy)
                }
                .onChange(of: model.isSending) { _, _ in
                    scrollToBottom(proxy)
                }
            }

            if let errorMessage = model.errorMessage {
                Text(errorMessage)
                    .font(.codexBody(11.5, weight: .semibold))
                    .foregroundStyle(Color.oarSignal)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(Color.white.opacity(0.32))
            }

            AgentShortcutStrip(send: send)

            ChatInputBar(draft: $draft, isSending: model.isSending, send: sendDraft)
        }
        .background(.thinMaterial)
        .background(Color.white.opacity(0.16))
        .sheet(isPresented: $showsSettings) {
            AgentSettingsSheet(settings: model.settings) { baseURL, modelName, apiKey in
                try model.saveSettings(
                    baseURLString: baseURL,
                    model: modelName,
                    apiKey: apiKey
                )
            }
        }
        .onAppear(perform: syncConversation)
        .onChange(of: item?.id) { _, _ in
            syncConversation()
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("OAR Agent")
                .font(.codexDisplay(16, weight: .semibold))
            Circle()
                .fill(model.isConfigured ? Color.oarMoss : Color.oarAmber)
                .frame(width: 6, height: 6)
            Spacer()
            Button {
                showsSettings = true
            } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Color.codexMuted)
                    .frame(width: 26, height: 26)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Agent 设置")
        }
        .padding(16)
    }

    private var context: AgentConversationContext {
        guard let item else { return .empty }

        let actionSummary: String
        if let action {
            actionSummary = "\(action.actionType.rawValue)：\(action.rationale) dry-run：\(action.dryRunResultSummary)"
        } else {
            actionSummary = "暂无建议动作。"
        }

        return AgentConversationContext(
            title: item.keyResultTitle,
            riskReason: item.riskReason,
            actionSummary: actionSummary,
            evidenceSummaries: evidence.map { $0.summary }
        )
    }

    private func sendDraft() {
        send(draft)
    }

    private func send(_ text: String) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        draft = ""
        Task {
            await model.send(text, context: context)
        }
    }

    private func syncConversation() {
        model.activateConversation(itemID: item?.id)
        draft = ""
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.18)) {
            if model.isSending {
                proxy.scrollTo("agent-thinking", anchor: .bottom)
            } else if let id = model.messages.last?.id {
                proxy.scrollTo(id, anchor: .bottom)
            }
        }
    }
}

private struct AgentContextCard: View {
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("当前上下文")
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            Text(item?.keyResultTitle ?? "未选择风险")
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(2)
            Text(action?.actionType.rawValue ?? "等待建议动作")
                .font(.codexBody(12, weight: .medium))
                .foregroundStyle(Color.codexMuted)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white.opacity(0.38))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct AgentBubble: View {
    let message: AgentMessage

    private var isUser: Bool {
        message.role == .user
    }

    var body: some View {
        HStack {
            if isUser {
                Spacer(minLength: 34)
            }

            Text(message.text)
                .font(.codexBody(12.5))
                .lineSpacing(3)
                .foregroundStyle(isUser ? Color.white : Color.codexInk)
                .padding(.horizontal, 11)
                .padding(.vertical, 9)
                .background(isUser ? Color.codexInk.opacity(0.88) : Color.white.opacity(0.48))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .textSelection(.enabled)

            if !isUser {
                Spacer(minLength: 34)
            }
        }
    }
}

private struct AgentThinkingBubble: View {
    var body: some View {
        HStack {
            HStack(spacing: 5) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Color.codexMuted.opacity(0.52))
                        .frame(width: 5, height: 5)
                        .opacity(index == 1 ? 0.72 : 0.42)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color.white.opacity(0.48))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            Spacer(minLength: 34)
        }
    }
}

private struct AgentShortcutStrip: View {
    let send: (String) -> Void

    var body: some View {
        HStack(spacing: 7) {
            shortcut("解释风险")
            shortcut("生成确认理由")
            shortcut("检查证据缺口")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color.white.opacity(0.25))
    }

    private func shortcut(_ title: String) -> some View {
        Button(title) {
            send(title)
        }
        .font(.codexBody(11.5, weight: .semibold))
        .buttonStyle(.plain)
        .padding(.horizontal, 9)
        .frame(height: 26)
        .background(Color.white.opacity(0.44))
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }
}

private struct ChatInputBar: View {
    @Binding var draft: String
    let isSending: Bool
    let send: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            TextField("问证据、理由或风险", text: $draft)
                .font(.codexBody(13))
                .textFieldStyle(.plain)
                .onSubmit(send)

            Button(action: send) {
                Image(systemName: isSending ? "hourglass" : "arrow.up")
                    .font(.system(size: 11, weight: .bold))
                    .frame(width: 25, height: 25)
                    .background(sendDisabled ? Color.codexMuted.opacity(0.14) : Color.codexInk)
                    .foregroundStyle(sendDisabled ? Color.codexMuted : Color.white)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .disabled(sendDisabled)
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(Color.white.opacity(0.42))
    }

    private var sendDisabled: Bool {
        isSending || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

private struct AgentSettingsSheet: View {
    let settings: AgentSettings
    let save: (String, String, String?) throws -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var baseURLString: String
    @State private var modelName: String
    @State private var apiKey = ""
    @State private var errorMessage: String?

    init(
        settings: AgentSettings,
        save: @escaping (String, String, String?) throws -> Void
    ) {
        self.settings = settings
        self.save = save
        _baseURLString = State(initialValue: settings.baseURL.absoluteString)
        _modelName = State(initialValue: settings.model)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Agent 设置")
                    .font(.codexDisplay(18, weight: .semibold))
                Spacer()
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(.plain)
            }

            VStack(alignment: .leading, spacing: 10) {
                settingsField("Base URL", text: $baseURLString)
                settingsField("Model", text: $modelName)
                SecureField(settings.hasAPIKey ? "API Key（留空保持不变）" : "API Key", text: $apiKey)
                    .font(.codexBody(13))
                    .textFieldStyle(.plain)
                    .padding(10)
                    .background(Color.white.opacity(0.58))
                    .clipShape(RoundedRectangle(cornerRadius: 7))
            }

            Text("当前上下文会发送到你配置的模型服务。API Key 只保存在本机 Keychain。")
                .font(.codexBody(11.5))
                .foregroundStyle(Color.codexMuted)

            if let errorMessage {
                Text(errorMessage)
                    .font(.codexBody(12, weight: .semibold))
                    .foregroundStyle(Color.oarSignal)
            }

            HStack {
                Spacer()
                Button("保存") {
                    do {
                        try save(baseURLString, modelName, apiKey.isEmpty ? nil : apiKey)
                        dismiss()
                    } catch {
                        errorMessage = (error as? LocalizedError)?.errorDescription ?? "保存失败。"
                    }
                }
                .buttonStyle(OARButtonStyle(prominent: true))
            }
        }
        .padding(22)
        .frame(width: 420)
        .background(.thinMaterial)
    }

    private func settingsField(_ title: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            TextField(title, text: text)
                .font(.codexBody(13))
                .textFieldStyle(.plain)
                .padding(10)
                .background(Color.white.opacity(0.58))
                .clipShape(RoundedRectangle(cornerRadius: 7))
        }
    }
}
