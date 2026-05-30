import SwiftUI

struct RiskStrip: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 10) {
                ForEach(model.sortedItems) { item in
                    Button {
                        model.select(item)
                    } label: {
                        RiskPillCard(
                            item: item,
                            selected: model.selectedItem?.id == item.id
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(item.riskLevel.rawValue)风险：\(item.keyResultTitle)")
                }
            }
        }
        .scrollIndicators(.hidden)
    }
}

private struct RiskPillCard: View {
    let item: ReviewInboxDisplayItem
    let selected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                HStack(spacing: 7) {
                    OARSymbolDot(color: item.riskLevel.color)
                    Text(item.riskLevel.rawValue)
                        .font(.codexBody(11, weight: .semibold))
                        .foregroundStyle(item.riskLevel.color)
                }
                Spacer()
                Text(item.status.rawValue)
                    .font(.codexBody(11, weight: .semibold))
                    .foregroundStyle(selected ? Color.white.opacity(0.78) : Color.codexMuted)
            }

            Text(item.keyResultTitle)
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(1)

            HStack {
                Text(item.ownerName)
                Spacer()
                Text("可信 \(Int(item.confidenceScore * 100))%")
            }
            .font(.codexBody(11, weight: .medium))
            .foregroundStyle(selected ? Color.white.opacity(0.68) : Color.codexMuted)
        }
        .padding(11)
        .frame(width: 214, height: 92, alignment: .leading)
        .background(selected ? Color.codexInk.opacity(0.88) : Color.white.opacity(0.42))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(selected ? 0.14 : 0.44), lineWidth: 1)
        )
        .foregroundStyle(selected ? Color.white : Color.codexInk)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
