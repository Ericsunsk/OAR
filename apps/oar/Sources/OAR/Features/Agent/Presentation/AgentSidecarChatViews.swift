import SwiftUI

struct AgentContextCard: View {
    let context: AgentConversationContext
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?

    private var content: AgentContextCardContent {
        AgentContextCardContent(context: context, item: item, action: action)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("工作区信号")
                    .font(.codexBody(11, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
                Spacer(minLength: 8)
                Text(content.statisticsText)
                    .font(.codexBody(10.5, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
                    .lineLimit(1)
            }

            Text(content.title)
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(2)

            Text(content.focusText)
                .font(.codexBody(12, weight: .medium))
                .foregroundStyle(Color.codexMuted)
                .lineLimit(2)

            Text(content.summaryText)
                .font(.codexBody(11.5))
                .foregroundStyle(Color.codexMuted.opacity(0.88))
                .lineLimit(3)

            if let primarySignal = content.primarySignalText {
                Text(primarySignal)
                    .font(.codexBody(11.5, weight: .medium))
                    .foregroundStyle(Color.codexInk.opacity(0.72))
                    .lineLimit(2)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white.opacity(0.38))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct AgentContextCardContent: Equatable {
    let title: String
    let focusText: String
    let summaryText: String
    let statisticsText: String
    let primarySignalText: String?

    init(
        context: AgentConversationContext,
        item: ReviewInboxDisplayItem? = nil,
        action: ReviewInboxSuggestedAction? = nil
    ) {
        let evidenceCount = max(context.evidenceRefs.count, context.evidenceSummaries.count)
        title = Self.displayText(
            context.title,
            fallback: item?.keyResultTitle ?? AgentConversationContext.empty.title,
            maxCharacters: 96
        )
        focusText = Self.focusText(context: context, action: action)
        summaryText = Self.displayText(
            context.workspaceSummary,
            fallback: context.riskReason,
            maxCharacters: 220
        )
        statisticsText = "证据 \(evidenceCount)｜信号 \(context.workspaceSignals.count)｜待处理 \(context.pendingActionSummaries.count)｜账本 \(context.ledgerEventSummaries.count)"
        primarySignalText = Self.primarySignalText(context: context)
    }

    private static func focusText(
        context: AgentConversationContext,
        action: ReviewInboxSuggestedAction?
    ) -> String {
        let actionSummary = displayText(context.actionSummary, fallback: "", maxCharacters: 150)
        if !actionSummary.isEmpty, actionSummary != AgentConversationContext.empty.actionSummary {
            return "当前焦点：\(actionSummary)"
        }

        let riskReason = displayText(context.riskReason, fallback: "", maxCharacters: 150)
        if !riskReason.isEmpty, riskReason != AgentConversationContext.empty.riskReason {
            return "当前焦点：\(riskReason)"
        }

        if let action {
            return "当前焦点动作：\(action.actionType.rawValue)"
        }

        return "当前焦点：工作区总览"
    }

    private static func primarySignalText(context: AgentConversationContext) -> String? {
        if let workspaceSignal = context.workspaceSignals.first {
            return "信号：\(displayText(workspaceSignal, fallback: "", maxCharacters: 170))"
        }

        if let pendingAction = context.pendingActionSummaries.first {
            return "待处理：\(displayText(pendingAction, fallback: "", maxCharacters: 170))"
        }

        if let evidenceSummary = context.evidenceSummaries.first {
            return "证据：\(displayText(evidenceSummary, fallback: "", maxCharacters: 170))"
        }

        return nil
    }

    private static func displayText(
        _ text: String,
        fallback: String,
        maxCharacters: Int
    ) -> String {
        let cleaned = compact(text)
        let value = cleaned.isEmpty ? compact(fallback) : cleaned
        guard value.count > maxCharacters else { return value }
        return "\(String(value.prefix(maxCharacters)))..."
    }

    private static func compact(_ text: String) -> String {
        text
            .split(whereSeparator: \.isWhitespace)
            .joined(separator: " ")
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
