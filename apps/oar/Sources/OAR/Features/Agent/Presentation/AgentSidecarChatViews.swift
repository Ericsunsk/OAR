import SwiftUI

struct AgentContextCard: View {
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

struct AgentBubble: View {
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

struct AgentThinkingBubble: View {
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

struct AgentShortcutStrip: View {
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
