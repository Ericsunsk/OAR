import SwiftUI

struct DetailHeader: View {
    let item: ReviewInboxDisplayItem

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Text(item.weekLabel)
                Text("·")
                Text(item.ownerName)
                Text("·")
                Text("更新 \(item.lastUpdatedAt)")
            }
            .font(.codexBody(12, weight: .semibold))
            .foregroundStyle(Color.codexMuted)

            Text(item.keyResultTitle)
                .font(.codexDisplay(30, weight: .semibold))
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)

            Text(item.objectiveTitle)
                .font(.codexBody(14, weight: .medium))
                .foregroundStyle(Color.codexMuted)

            Text(item.riskReason)
                .font(.codexBody(15))
                .lineSpacing(4)
                .foregroundStyle(Color.codexInk.opacity(0.86))
        }
    }
}

struct DetailSection<Content: View>: View {
    let title: String
    let content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 11) {
            Text(title)
                .font(.codexBody(12, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct EvidenceRow: View {
    let evidence: ReviewInboxDisplayEvidence

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: sourceIcon)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color.codexMuted)
                .frame(width: 24, height: 24)
                .background(Color.white.opacity(0.42))
                .clipShape(RoundedRectangle(cornerRadius: 6))

            VStack(alignment: .leading, spacing: 4) {
                Text(evidence.summary)
                    .font(.codexBody(13))
                    .lineSpacing(3)

                Text("\(evidence.sourceType.rawValue) · \(evidence.signalType.rawValue) · \(evidence.capturedAt)")
                    .font(.codexBody(11, weight: .medium))
                    .foregroundStyle(Color.codexMuted)
            }

            Spacer()

            Text("\(Int(evidence.trustScore * 100))")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(Color.codexMuted)
        }
    }

    private var sourceIcon: String {
        switch evidence.sourceType {
        case .okr: "scope"
        case .task: "checklist"
        case .calendar: "calendar"
        case .meeting: "person.2"
        case .doc: "doc.text"
        case .im: "bubble.left.and.bubble.right"
        }
    }
}
