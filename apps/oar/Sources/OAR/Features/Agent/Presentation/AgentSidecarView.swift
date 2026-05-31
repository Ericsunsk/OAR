import SwiftUI

struct AgentSidecarView: View {
    @Bindable var model: AgentSidecarViewModel
    @Bindable var settingsModel: AgentSettingsViewModel
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?
    let context: AgentConversationContext

    @State private var draft = ""
    @State private var showsSettings = false

    init(
        model: AgentSidecarViewModel,
        settingsModel: AgentSettingsViewModel,
        item: ReviewInboxDisplayItem?,
        action: ReviewInboxSuggestedAction?,
        context: AgentConversationContext
    ) {
        self.model = model
        self.settingsModel = settingsModel
        self.item = item
        self.action = action
        self.context = context
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            AgentContextCard(context: context, item: item, action: action)
                .padding(.horizontal, 16)
                .padding(.bottom, 12)

            if let contextStatus = model.contextStatus {
                AgentContextStatusStrip(status: contextStatus)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 12)
            }

            Divider()
                .overlay(Color.codexBorder.opacity(0.28))

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 10) {
                        ForEach(model.messages) { message in
                            AgentBubble(message: message)
                                .id(message.id)
                        }

                        if model.isSending, model.messages.last?.role != .assistant {
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
                .onChange(of: model.messages.last?.text) { _, _ in
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
                .disabled(!agentInputEnabled)

            ChatInputBar(
                draft: $draft,
                isSending: model.isSending,
                isEnabled: agentInputEnabled,
                send: sendDraft
            )
        }
        .background(.thinMaterial)
        .background(Color.white.opacity(0.16))
        .onAppear(perform: syncFocus)
        .task {
            await settingsModel.loadIfNeeded()
        }
        .onChange(of: item?.id) { _, _ in
            syncFocus()
        }
        .sheet(isPresented: $showsSettings) {
            AgentSettingsSheet(model: settingsModel)
                .frame(width: 430)
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("OAR Agent")
                .font(.codexDisplay(16, weight: .semibold))
            OARSymbolDot(color: readinessColor, size: 6)
            Spacer()
            Button {
                showsSettings = true
            } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Color.codexMuted.opacity(0.72))
                    .frame(width: 24, height: 24)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Agent 设置")
        }
        .padding(16)
    }

    private func sendDraft() {
        send(draft)
    }

    private func send(_ text: String) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, !model.isSending, agentInputEnabled else { return }
        draft = ""
        Task {
            await model.send(text, context: context)
        }
    }

    private var agentInputEnabled: Bool {
        model.isConfigured && settingsModel.isReadyForChat
    }

    private var readinessColor: Color {
        switch settingsModel.configurationState {
        case .ready:
            return Color.oarMoss
        case .loading:
            return Color.codexMuted
        case .missingModel:
            return Color.oarAmber
        case .unavailable:
            return Color.codexMuted
        }
    }

    private func syncFocus() {
        model.activateFocus(itemID: item?.id)
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
