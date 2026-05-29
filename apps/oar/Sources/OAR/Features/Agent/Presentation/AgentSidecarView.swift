import AppKit
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

            ChatInputBar(draft: $draft, isSending: model.isSending, send: sendDraft)
        }
        .background(.thinMaterial)
        .background(Color.white.opacity(0.16))
        .onAppear(perform: syncFocus)
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
            OARSymbolDot(color: model.isConfigured ? Color.oarMoss : Color.oarAmber, size: 6)
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
        guard !text.isEmpty, !model.isSending else { return }
        draft = ""
        Task {
            await model.send(text, context: context)
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

private struct AgentContextCard: View {
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("工作区信号")
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            Text(item?.keyResultTitle ?? "未选择焦点")
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(2)
            Text(action.map { "当前焦点动作：\($0.actionType.rawValue)" } ?? "当前焦点：工作区总览")
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
                    OARSymbolDot(color: Color.codexMuted.opacity(0.52), size: 5)
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
            shortcut("规划下一步")
            shortcut("扫描风险")
            shortcut("起草动作")
            shortcut("检查证据")
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
            ZStack(alignment: .topLeading) {
                AgentComposerTextView(text: $draft, submit: send)
                    .frame(maxWidth: .infinity, minHeight: 32, maxHeight: 64)

                if draft.isEmpty {
                    Text("问计划、风险、证据或动作")
                        .font(.codexBody(13))
                        .foregroundStyle(Color.codexMuted.opacity(0.72))
                        .padding(.top, 7)
                        .allowsHitTesting(false)
                }
            }

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
            .accessibilityLabel("发送消息")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .frame(minHeight: 46)
        .background(Color.white.opacity(0.42))
    }

    private var sendDisabled: Bool {
        isSending || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

private struct AgentComposerTextView: NSViewRepresentable {
    @Binding var text: String
    let submit: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text, submit: submit)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        textStorage.addLayoutManager(layoutManager)
        let textContainer = NSTextContainer(containerSize: NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        textContainer.widthTracksTextView = true
        textContainer.lineFragmentPadding = 0
        layoutManager.addTextContainer(textContainer)

        let textView = EditableTextView(frame: NSRect(x: 0, y: 0, width: 240, height: 32), textContainer: textContainer)
        textView.delegate = context.coordinator
        textView.drawsBackground = false
        textView.isEditable = true
        textView.isSelectable = true
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.font = .systemFont(ofSize: 13)
        textView.textColor = .labelColor
        textView.insertionPointColor = .labelColor
        textView.textContainerInset = NSSize(width: 0, height: 6)
        textView.minSize = NSSize(width: 0, height: 32)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]

        let scrollView = NSScrollView()
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = false
        scrollView.hasHorizontalScroller = false
        scrollView.autoresizesSubviews = true
        scrollView.documentView = textView
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.text = $text
        context.coordinator.submit = submit

        guard let textView = scrollView.documentView as? NSTextView else { return }

        // Keep the text view width in sync with the clip view so the
        // full area is clickable / editable.
        let clipWidth = scrollView.contentSize.width
        if clipWidth > 0, abs(textView.frame.width - clipWidth) > 0.5 {
            textView.setFrameSize(NSSize(width: clipWidth, height: textView.frame.height))
        }

        // Only sync text when it was changed externally (e.g. cleared after
        // send). Preserve the insertion point so the cursor doesn't jump.
        if textView.string != text {
            let selectedRanges = textView.selectedRanges
            textView.string = text
            let textLength = (textView.string as NSString).length
            let clampedRanges = selectedRanges.compactMap { value -> NSValue? in
                let range = value.rangeValue
                guard range.location != NSNotFound else { return nil }
                let location = min(range.location, textLength)
                let upperBound = min(NSMaxRange(range), textLength)
                return NSValue(range: NSRange(location: location, length: max(0, upperBound - location)))
            }
            textView.selectedRanges = clampedRanges.isEmpty
                ? [NSValue(range: NSRange(location: textLength, length: 0))]
                : clampedRanges
        }
    }

    /// Subclass that guarantees first-responder acceptance for keyboard input.
    private final class EditableTextView: NSTextView {
        override var acceptsFirstResponder: Bool { true }

        override func becomeFirstResponder() -> Bool {
            let result = super.becomeFirstResponder()
            insertionPointColor = .labelColor
            return result
        }
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var text: Binding<String>
        var submit: () -> Void

        init(text: Binding<String>, submit: @escaping () -> Void) {
            self.text = text
            self.submit = submit
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            text.wrappedValue = textView.string
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                submit()
                return true
            }
            return false
        }
    }
}
