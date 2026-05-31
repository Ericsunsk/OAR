import SwiftUI

struct AgentContextStatusStrip: View {
    let status: AgentContextStatus

    private var content: AgentContextStatusContent {
        AgentContextStatusContent(status: status)
    }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: content.symbolName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(content.tint)
                .frame(width: 18, height: 18)

            VStack(alignment: .leading, spacing: 3) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(content.title)
                        .font(.codexBody(11.5, weight: .semibold))
                        .foregroundStyle(Color.codexInk.opacity(0.78))
                    Text(content.statisticsText)
                        .font(.codexBody(10.5, weight: .semibold))
                        .foregroundStyle(Color.codexMuted)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                }

                if let detailText = content.detailText {
                    Text(detailText)
                        .font(.codexBody(11))
                        .foregroundStyle(Color.codexMuted.opacity(0.88))
                        .fixedSize(horizontal: false, vertical: true)
                        .lineLimit(2)
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white.opacity(0.28))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct AgentContextStatusContent {
    let title: String
    let statisticsText: String
    let detailText: String?
    let symbolName: String
    let tint: Color

    init(status: AgentContextStatus) {
        let hasDegradedRead = status.liveReads.contains { $0.state.isRestricted }
        if hasDegradedRead {
            title = "实时读取受限"
        } else if !status.liveReads.isEmpty {
            title = "实时读取已接入"
        } else {
            title = "已激活内置 skill"
        }
        statisticsText = "读取 \(status.liveReads.count)｜技能 \(status.activatedSkills.count)"
        detailText = Self.detailText(for: status)
        symbolName = hasDegradedRead ? "exclamationmark.triangle" : "antenna.radiowaves.left.and.right"
        tint = hasDegradedRead ? Color.oarAmber : Color.oarMoss
    }

    private static func detailText(for status: AgentContextStatus) -> String? {
        let summaries = [status.liveReads.first?.summary, status.activatedSkills.first?.summary]
            .compactMap { $0 }
            .map { summary in
                summary
                    .split(whereSeparator: \.isWhitespace)
                    .joined(separator: " ")
            }
            .filter { !$0.isEmpty }
        guard !summaries.isEmpty else { return nil }
        let compacted = summaries.joined(separator: "\n")
        let maxCharacters = 170
        guard compacted.count > maxCharacters else { return compacted }
        return "\(String(compacted.prefix(maxCharacters)))..."
    }
}
